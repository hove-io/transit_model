// Copyright (C) 2017 Kisio Digital and/or its affiliates.
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by the
// Free Software Foundation, version 3.

// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.

// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>

use super::{utils, utils::FareFrameType};
use crate::{
    model::Collections,
    netex_utils,
    netex_utils::{FrameType, Frames},
    objects::*,
    AddPrefix, Result,
};
use failure::{bail, format_err};
use log::{info, warn, Level as LogLevel};
use minidom::Element;
use minidom_ext::{AttributeElementExt, OnlyChildElementExt};
use rust_decimal::Decimal;
use skip_error::skip_error_and_log;
use std::{
    collections::BTreeSet,
    convert::{From, TryFrom},
    fs,
    io::Read,
    path::Path,
};
use zip::read::ZipArchive;

impl TryFrom<&Element> for Ticket {
    type Error = failure::Error;
    fn try_from(distance_matrix_element: &Element) -> Result<Self> {
        if distance_matrix_element.name() != "DistanceMatrixElement" {
            bail!(
                "Failed to create a ticket from a '{}', it should be a 'DistanceMatrixElement'",
                distance_matrix_element.name()
            );
        }
        let id = distance_matrix_element
            .try_attribute("id")
            .map_err(|e| format_err!("{}", e))?;
        let ticket = Ticket {
            id,
            name: "Ticket Origin-Destination".to_string(),
            comment: None,
        };
        Ok(ticket)
    }
}

impl TryFrom<(String, Decimal, String, (Date, Date))> for TicketPrice {
    type Error = failure::Error;
    fn try_from(
        (ticket_id, price, currency, validity): (String, Decimal, String, (Date, Date)),
    ) -> Result<Self> {
        iso4217::alpha3(&currency)
            .ok_or_else(|| format_err!("Failed to convert '{}' as a currency", currency))?;
        let ticket_price = Self {
            ticket_id,
            price,
            currency,
            ticket_validity_start: validity.0,
            ticket_validity_end: validity.1,
        };
        Ok(ticket_price)
    }
}

impl From<String> for TicketUse {
    fn from(ticket_id: String) -> Self {
        let ticket_use_id = "TU:".to_string() + &ticket_id;
        Self {
            id: ticket_use_id,
            ticket_id,
            max_transfers: Some(0),
            boarding_time_limit: None,
            alighting_time_limit: None,
        }
    }
}

impl From<(String, String)> for TicketUsePerimeter {
    fn from((ticket_use_id, line_id): (String, String)) -> Self {
        Self {
            ticket_use_id,
            object_type: ObjectType::Line,
            object_id: line_id,
            perimeter_action: PerimeterAction::Included,
        }
    }
}

impl From<(String, (String, String))> for TicketUseRestriction {
    fn from((ticket_use_id, (use_origin, use_destination)): (String, (String, String))) -> Self {
        Self {
            ticket_use_id,
            restriction_type: RestrictionType::OriginDestination,
            use_origin,
            use_destination,
        }
    }
}

/// For HelloGo fares connector, we need the prefix of the input NTFS.
/// The prefix will be extracted from the 'contributor_id'
fn get_prefix(collections: &Collections) -> Option<String> {
    collections
        .contributors
        .values()
        .next()
        .map(|contributor| &contributor.id)
        .and_then(|contributor_id| {
            contributor_id
                .find(':')
                .map(|index| contributor_id[..index].to_string())
        })
}

fn get_unit_price_frame<'a>(frames: &'a Frames<'a>) -> Result<&'a Element> {
    if let Some(fare_frames) = frames.get(&FrameType::Fare) {
        let mut iterator = fare_frames.iter().filter(|fare_frame| {
            utils::get_fare_frame_type(fare_frame)
                .map(|fare_frame_type| fare_frame_type == FareFrameType::UnitPrice)
                .unwrap_or(false)
        });
        if let Some(ref unit_price_frame) = iterator.next() {
            if iterator.next().is_none() {
                Ok(unit_price_frame)
            } else {
                bail!("Failed to find a unique 'UnitPrice' fare frame in the Netex file")
            }
        } else {
            bail!("Failed to find a 'UnitPrice' fare frame in the Netex file")
        }
    } else {
        bail!("Failed to find a fare frame")
    }
}

fn calculate_direct_price(distance_matrix_element: &Element) -> Result<Decimal> {
    let distance_matrix_element_price = distance_matrix_element
        .try_only_child("prices")
        .map_err(|e| format_err!("{}", e))?
        .try_only_child("DistanceMatrixElementPrice")
        .map_err(|e| format_err!("{}", e))?;
    Ok(utils::get_amount_units_factor(
        distance_matrix_element_price,
    )?)
}

fn get_distance(distance_matrix_element: &Element) -> Result<u32> {
    let distance_str = distance_matrix_element
        .try_only_child("Distance")
        .map_err(|e| format_err!("{}", e))?
        .text();
    distance_str
        .parse()
        .map_err(|_| format_err!("Failed to parse '{}' into a 'u32'", distance_str))
}

fn get_line_id(fare_frame: &Element, service_frame: &Element) -> Result<String> {
    fn get_line_ref<'a>(fare_frame: &'a Element) -> Result<&'a str> {
        let references: Vec<_> = fare_frame
            .try_only_child("contentValidityConditions")
            .map_err(|e| format_err!("{}", e))?
            .children()
            .filter(|element| element.name() == "ValidityTrigger")
            .flat_map(|validity_trigger| validity_trigger.children())
            .filter(|element| element.name() == "TriggerObjectRef")
            .filter(|trigger_object_ref| {
                trigger_object_ref
                    .try_attribute::<String>("nameOfRefClass")
                    .map_err(|e| format_err!("{}", e))
                    .map(|ref_class| ref_class == "Line")
                    .unwrap_or(false)
            })
            .flat_map(|trigger_object_ref| trigger_object_ref.attr("ref"))
            .collect();
        if references.len() == 1 {
            Ok(references[0])
        } else {
            bail!("Failed to find a Line reference")
        }
    }

    fn get_line_id_from_line_ref(service_frame: &Element, line_ref: &str) -> Result<String> {
        let values: Vec<String> = service_frame
            .try_only_child("lines")
            .map_err(|e| format_err!("{}", e))?
            .children()
            .filter(|element| element.name() == "Line")
            .filter(|line| {
                line.try_attribute::<String>("id")
                    .map_err(|e| format_err!("{}", e))
                    .map(|id| id == line_ref)
                    .unwrap_or(false)
            })
            .map(|line| netex_utils::get_value_in_keylist(line, "KV1PlanningLijnNummer"))
            .collect::<Result<_>>()?;
        if values.len() == 1 {
            Ok(values[0].clone())
        } else {
            bail!("Failed to find the Line with identifier '{}'", line_ref)
        }
    }

    let line_ref = get_line_ref(fare_frame)?;
    let line_id = get_line_id_from_line_ref(service_frame, line_ref)?;
    Ok(line_id)
}

fn get_origin_destinations(
    collections: &Collections,
    service_frame: &Element,
    distance_matrix_element: &Element,
    prefix_with_colon: &str,
) -> Result<Vec<(String, String)>> {
    fn get_ref(distance_matrix_element: &Element, element_name: &str) -> Result<String> {
        distance_matrix_element
            .try_only_child(element_name)
            .map_err(|e| format_err!("{}", e))?
            .try_attribute("ref")
            .map_err(|e| format_err!("{}", e))
    }
    let start_stop_point_ref = get_ref(distance_matrix_element, "StartStopPointRef")?;
    let end_stop_point_ref = get_ref(distance_matrix_element, "EndStopPointRef")?;
    let scheduled_stop_points = service_frame
        .try_only_child("scheduledStopPoints")
        .map_err(|e| format_err!("{}", e))?;
    fn get_stop_point_ids<'a>(
        scheduled_stop_points: &'a Element,
        stop_point_ref: &str,
    ) -> Result<Vec<&'a str>> {
        let selected_scheduled_stop_points: Vec<_> = scheduled_stop_points
            .children()
            .filter(|element| element.name() == "ScheduledStopPoint")
            .filter(|scheduled_stop_point| {
                scheduled_stop_point
                    .try_attribute::<String>("id")
                    .map_err(|e| format_err!("{}", e))
                    .map(|id| id == stop_point_ref)
                    .unwrap_or(false)
            })
            .collect();
        if selected_scheduled_stop_points.len() != 1 {
            bail!(
                "Failed to find a unique 'ScheduledStopPoint' with reference '{}'",
                stop_point_ref
            )
        }
        let scheduled_stop_point = selected_scheduled_stop_points[0];
        fn remove_netex_prefix<'a>(reference: &'a str) -> Result<&'a str> {
            if let Some(index) = reference.find(':') {
                if reference.len() > index + 1 {
                    Ok(&reference[index + 1..])
                } else {
                    bail!("Failed to remove prefix from '{}'", reference)
                }
            } else {
                bail!("Failed to find ':' to remove a prefix in '{}'", reference)
            }
        }
        let stop_point_ids = scheduled_stop_point
            .try_only_child("projections")
            .map_err(|e| format_err!("{}", e))?
            .children()
            .filter(|element| element.name() == "PointProjection")
            .flat_map(|point_projection| point_projection.children())
            .filter(|element| element.name() == "ProjectedPointRef")
            .flat_map(|projected_point_ref| projected_point_ref.attr("ref"))
            .map(|reference| remove_netex_prefix(reference))
            .collect::<Result<_>>()?;
        Ok(stop_point_ids)
    }
    let start_stop_point_ids = get_stop_point_ids(scheduled_stop_points, &start_stop_point_ref)?;
    let end_stop_point_ids = get_stop_point_ids(scheduled_stop_points, &end_stop_point_ref)?;
    fn get_stop_point_from_collections<'a>(
        collections: &'a Collections,
        stop_point_id: &str,
        prefix_with_colon: &str,
    ) -> Option<&'a StopPoint> {
        collections
            .stop_points
            .get(&format!("{}{}", prefix_with_colon, stop_point_id))
    }
    let start_stop_area_ids: BTreeSet<_> = start_stop_point_ids
        .iter()
        .flat_map(|stop_point_id| {
            get_stop_point_from_collections(collections, stop_point_id, prefix_with_colon)
        })
        .map(|stop_point| stop_point.stop_area_id.clone())
        .collect();
    let end_stop_area_ids: BTreeSet<_> = end_stop_point_ids
        .iter()
        .flat_map(|stop_point_id| {
            get_stop_point_from_collections(collections, stop_point_id, prefix_with_colon)
        })
        .map(|stop_point| stop_point.stop_area_id.clone())
        .collect();
    let origin_destinations = start_stop_area_ids
        .iter()
        .flat_map(|origin| {
            end_stop_area_ids
                .iter()
                .map(move |destination| (origin.clone(), destination.clone()))
        })
        .collect();
    Ok(origin_destinations)
}

fn load_netex_fares(collections: &mut Collections, root: &Element) -> Result<()> {
    let prefix_with_colon = get_prefix(&collections)
        .map(|prefix| prefix + ":")
        .unwrap_or_else(String::new);
    let frames = netex_utils::parse_frames_by_type(
        root.try_only_child("dataObjects")
            .map_err(|e| format_err!("{}", e))?
            .try_only_child("CompositeFrame")
            .map_err(|e| format_err!("{}", e))?
            .try_only_child("frames")
            .map_err(|e| format_err!("{}", e))?,
    )?;
    let unit_price_frame = get_unit_price_frame(&frames)?;
    let service_frame = netex_utils::get_only_frame(&frames, FrameType::Service)?;
    let resource_frame = netex_utils::get_only_frame(&frames, FrameType::Resource)?;
    let unit_price = utils::get_unit_price(unit_price_frame)?;
    let validity = utils::get_validity(resource_frame)?;
    for fare_frame in frames.get(&FrameType::Fare).unwrap_or(&vec![]) {
        let fare_frame_type = utils::get_fare_frame_type(fare_frame)?;
        if fare_frame_type != FareFrameType::DirectPriceMatrix
            && fare_frame_type != FareFrameType::DistanceMatrix
        {
            continue;
        }
        let line_id = get_line_id(fare_frame, service_frame)?;
        let line = if let Some(line) = collections
            .lines
            .get(&format!("{}{}", &prefix_with_colon, line_id))
        {
            line
        } else {
            warn!("Failed to find line ID '{}' in the existing NTFS", line_id);
            continue;
        };
        let boarding_fee: Decimal =
            netex_utils::get_value_in_keylist(fare_frame, "EntranceRateWrtCurrency")?;
        let rounding_rule: Decimal =
            netex_utils::get_value_in_keylist(fare_frame, "RoundingWrtCurrencyRule")?;
        let rounding_rule = rounding_rule.normalize().scale();
        let currency = utils::get_currency(fare_frame)?;
        let distance_matrix_elements = utils::get_distance_matrix_elements(fare_frame)?;
        for distance_matrix_element in distance_matrix_elements {
            let mut ticket = Ticket::try_from(distance_matrix_element)?;
            let price = match fare_frame_type {
                FareFrameType::DirectPriceMatrix => {
                    boarding_fee + calculate_direct_price(distance_matrix_element)?
                }
                FareFrameType::DistanceMatrix => {
                    let distance: Decimal = get_distance(distance_matrix_element)?.into();
                    boarding_fee + unit_price * distance
                }
                _ => continue,
            };
            let price = price
                .round_dp_with_strategy(rounding_rule, rust_decimal::RoundingStrategy::RoundHalfUp);
            let mut ticket_price =
                TicketPrice::try_from((ticket.id.clone(), price, currency.clone(), validity))?;
            let mut ticket_use = TicketUse::from(ticket.id.clone());
            let mut ticket_use_perimeter =
                TicketUsePerimeter::from((ticket_use.id.clone(), line.id.clone()));
            let origin_destinations = get_origin_destinations(
                &*collections,
                service_frame,
                distance_matrix_element,
                &prefix_with_colon,
            )?;
            if !origin_destinations.is_empty() {
                for origin_destination in origin_destinations {
                    let mut ticket_use_restriction =
                        TicketUseRestriction::from((ticket_use.id.clone(), origin_destination));
                    // `use_origin` and `use_destination` are already
                    // prefixed so we can't use the AddPrefix trait here
                    ticket_use_restriction.ticket_use_id =
                        prefix_with_colon.clone() + &ticket_use_restriction.ticket_use_id;
                    collections
                        .ticket_use_restrictions
                        .push(ticket_use_restriction);
                }
                ticket.add_prefix(&prefix_with_colon);
                collections.tickets.push(ticket)?;
                ticket_use.add_prefix(&prefix_with_colon);
                collections.ticket_uses.push(ticket_use)?;
                ticket_price.add_prefix(&prefix_with_colon);
                collections.ticket_prices.push(ticket_price);
                // `object_id` is already prefixed so we can't use the
                // AddPrefix trait here
                ticket_use_perimeter.ticket_use_id =
                    prefix_with_colon.clone() + &ticket_use_perimeter.ticket_use_id;
                collections.ticket_use_perimeters.push(ticket_use_perimeter);
            }
        }
    }
    Ok(())
}

/// Enrich the model with HelloGo fares.
///
/// HelloGo fares is provided as Netex files, compressed into ZIP archives.
/// `fares_path` is the path to a folder that may contain one or more ZIP
/// archive, all relative to the same model.
///
/// `collections` will be enrich with all the fares in the form of NTFS fares
/// model (see
/// https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fare_extension.md)
pub fn enrich_with_hellogo_fares<P: AsRef<Path>>(
    collections: &mut Collections,
    fares_path: P,
) -> Result<()> {
    let file_paths = fs::read_dir(&fares_path)?
        .map(|f| Ok(f?.path()))
        .collect::<Result<BTreeSet<_>>>()?;
    if file_paths.is_empty() {
        bail!("no files found into HelloGo fares directory");
    }
    for file_path in file_paths {
        let zip_file = fs::File::open(file_path)?;
        let mut zip_archive = skip_error_and_log!(ZipArchive::new(zip_file), LogLevel::Warn);
        for i in 0..zip_archive.len() {
            let mut zip_file = zip_archive.by_index(i)?;
            match zip_file.sanitized_name().extension() {
                Some(ext) if ext == "xml" => {
                    info!("reading fares file {:?}", zip_file.sanitized_name());
                    let mut file_content = String::new();
                    zip_file.read_to_string(&mut file_content)?;
                    let root: Element = file_content.parse().map_err(|e| {
                        format_err!(
                            "failed to parse file '{:?}': {}",
                            zip_file.sanitized_name(),
                            e
                        )
                    })?;
                    load_netex_fares(collections, &root)?;
                }
                _ => {
                    info!("skipping file in zip: {:?}", zip_file.sanitized_name());
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    mod prefix {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn extract_prefix() {
            let mut collections = Collections::default();
            let contributor = Contributor {
                id: String::from("PRE:contributor:id"),
                ..Default::default()
            };
            collections.contributors.push(contributor).unwrap();
            let prefix = get_prefix(&collections).unwrap();
            assert_eq!("PRE", prefix);
        }

        #[test]
        fn no_prefix() {
            let collections = Collections::default();
            let prefix = get_prefix(&collections);
            assert_eq!(None, prefix);
        }
    }

    mod unit_price_frame {
        use super::*;
        use std::collections::HashMap;

        #[test]
        fn has_unit_price_frame() {
            let xml = r#"<root>
                    <fareStructures>
                        <FareStructure>
                            <KeyList>
                                <KeyValue>
                                    <Key>FareStructureType</Key>
                                    <Value>UnitPrice</Value>
                                </KeyValue>
                            </KeyList>
                        </FareStructure>
                    </fareStructures>
                </root>"#;
            let unit_price_frame: Element = xml.parse().unwrap();
            let mut frames = HashMap::new();
            frames.insert(FrameType::Fare, vec![&unit_price_frame]);
            let unit_price_frame = get_unit_price_frame(&frames);
            assert!(unit_price_frame.is_ok())
        }

        #[test]
        #[should_panic = "Failed to find a fare frame"]
        fn no_fare_frame() {
            get_unit_price_frame(&HashMap::new()).unwrap();
        }

        #[test]
        #[should_panic = "Failed to find a \\'UnitPrice\\' fare frame in the Netex file"]
        fn no_unit_price_fare_frame() {
            let xml = r#"<root>
                    <fareStructures>
                        <FareStructure>
                            <KeyList>
                                <KeyValue>
                                    <Key>FareStructureType</Key>
                                    <Value>DistanceMatrix</Value>
                                </KeyValue>
                            </KeyList>
                        </FareStructure>
                    </fareStructures>
                </root>"#;
            let unit_price_frame: Element = xml.parse().unwrap();
            let mut frames = HashMap::new();
            frames.insert(FrameType::Fare, vec![&unit_price_frame]);
            get_unit_price_frame(&frames).unwrap();
        }

        #[test]
        #[should_panic = "Failed to find a unique \\'UnitPrice\\' fare frame in the Netex file"]
        fn multiple_unit_price_fare_frame() {
            let xml = r#"<root>
                    <fareStructures>
                        <FareStructure>
                            <KeyList>
                                <KeyValue>
                                    <Key>FareStructureType</Key>
                                    <Value>UnitPrice</Value>
                                </KeyValue>
                            </KeyList>
                        </FareStructure>
                    </fareStructures>
                </root>"#;
            let unit_price_frame: Element = xml.parse().unwrap();
            let mut frames = HashMap::new();
            frames.insert(FrameType::Fare, vec![&unit_price_frame, &unit_price_frame]);
            get_unit_price_frame(&frames).unwrap();
        }
    }

    mod ticket {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn extract_ticket() {
            let xml = r#"<DistanceMatrixElement id="ticket:1" />"#;
            let distance_matrix_element: Element = xml.parse().unwrap();
            let ticket = Ticket::try_from(&distance_matrix_element).unwrap();
            assert_eq!("ticket:1", ticket.id);
            assert_eq!("Ticket Origin-Destination", ticket.name);
            assert_eq!(None, ticket.comment);
        }

        #[test]
        #[should_panic(
            expected = "Failed to find attribute \\'id\\' in element \\'DistanceMatrixElement\\'"
        )]
        fn no_id() {
            let xml = r#"<DistanceMatrixElement />"#;
            let distance_matrix_element: Element = xml.parse().unwrap();
            Ticket::try_from(&distance_matrix_element).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to create a ticket from a \\'ticket\\', it should be a \\'DistanceMatrixElement\\'"
        )]
        fn incorrect_element_type() {
            let xml = r#"<ticket />"#;
            let ticket: Element = xml.parse().unwrap();
            Ticket::try_from(&ticket).unwrap();
        }
    }

    mod ticket_price {
        use super::*;
        use chrono::NaiveDate;
        use pretty_assertions::assert_eq;
        use rust_decimal_macros::dec;

        #[test]
        fn valid_ticket_price() {
            let ticket_id = String::from("ticket:1");
            let price = dec!(4.2);
            let currency = String::from("EUR");
            let validity_start = NaiveDate::from_ymd(2019, 2, 7);
            let validity_end = NaiveDate::from_ymd(2019, 3, 14);
            let ticket_price =
                TicketPrice::try_from((ticket_id, price, currency, (validity_start, validity_end)))
                    .unwrap();
            assert_eq!(String::from("ticket:1"), ticket_price.ticket_id);
            assert_eq!(dec!(4.2), ticket_price.price);
            assert_eq!(String::from("EUR"), ticket_price.currency);
            assert_eq!(validity_start, ticket_price.ticket_validity_start);
            assert_eq!(validity_end, ticket_price.ticket_validity_end);
        }

        #[test]
        #[should_panic(expected = "Failed to convert \\'XXX\\' as a currency")]
        fn invalid_currency() {
            let ticket_id = String::from("ticket:1");
            let price = dec!(4.2);
            let currency = String::from("XXX");
            let validity_start = NaiveDate::from_ymd(2019, 2, 7);
            let validity_end = NaiveDate::from_ymd(2019, 3, 14);
            TicketPrice::try_from((ticket_id, price, currency, (validity_start, validity_end)))
                .unwrap();
        }
    }

    mod ticket_use {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn valid_ticket_use() {
            let ticket_id = String::from("ticket:1");
            let ticket_use = TicketUse::from(ticket_id);
            assert_eq!(String::from("TU:ticket:1"), ticket_use.id);
            assert_eq!(String::from("ticket:1"), ticket_use.ticket_id);
            assert_eq!(0, ticket_use.max_transfers.unwrap());
            assert_eq!(None, ticket_use.boarding_time_limit);
            assert_eq!(None, ticket_use.alighting_time_limit);
        }
    }

    mod ticket_use_perimeter {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn valid_ticket_use() {
            let ticket_use_id = String::from("ticket_use:1");
            let line_id = String::from("line:1");
            let ticket_use_perimeter = TicketUsePerimeter::from((ticket_use_id, line_id));
            assert_eq!(
                String::from("ticket_use:1"),
                ticket_use_perimeter.ticket_use_id
            );
            assert_eq!(String::from("line:1"), ticket_use_perimeter.object_id);
            assert_eq!(ObjectType::Line, ticket_use_perimeter.object_type);
            assert_eq!(
                PerimeterAction::Included,
                ticket_use_perimeter.perimeter_action
            );
        }
    }

    mod ticket_use_restriction {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn valid_ticket_use() {
            let ticket_use_id = String::from("ticket_use:1");
            let origin = String::from("stop_area:1");
            let destination = String::from("stop_area:2");
            let ticket_use_restriction =
                TicketUseRestriction::from((ticket_use_id, (origin, destination)));
            assert_eq!(
                String::from("ticket_use:1"),
                ticket_use_restriction.ticket_use_id
            );
            assert_eq!(
                RestrictionType::OriginDestination,
                ticket_use_restriction.restriction_type
            );
            assert_eq!(
                String::from("stop_area:1"),
                ticket_use_restriction.use_origin
            );
            assert_eq!(
                String::from("stop_area:2"),
                ticket_use_restriction.use_destination
            );
        }
    }

    mod direct_price {
        use super::*;
        use pretty_assertions::assert_eq;
        use rust_decimal_macros::dec;

        #[test]
        fn get_direct_price() {
            let xml = r#"<DistanceMatrixElement>
                    <prices>
                        <DistanceMatrixElementPrice>
                            <Amount>42</Amount>
                            <Units>0.5</Units>
                        </DistanceMatrixElementPrice>
                    </prices>
                </DistanceMatrixElement>"#;
            let distance_element_matrix: Element = xml.parse().unwrap();
            let price = calculate_direct_price(&distance_element_matrix).unwrap();
            assert_eq!(dec!(21.0), price);
        }

        #[test]
        #[should_panic(
            expected = "No children with name \\'prices\\' in Element \\'DistanceMatrixElement\\'"
        )]
        fn no_prices() {
            let xml = r#"<DistanceMatrixElement />"#;
            let distance_element_matrix: Element = xml.parse().unwrap();
            calculate_direct_price(&distance_element_matrix).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "No children with name \\'DistanceMatrixElementPrice\\' in Element \\'prices\\'"
        )]
        fn no_distance_matrix_element_price() {
            let xml = r#"<DistanceMatrixElement>
                    <prices />
                </DistanceMatrixElement>"#;
            let distance_element_matrix: Element = xml.parse().unwrap();
            calculate_direct_price(&distance_element_matrix).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Multiple children with name \\'DistanceMatrixElementPrice\\' in Element \\'prices\\'"
        )]
        fn multiple_distance_matrix_element_price() {
            let xml = r#"<DistanceMatrixElement>
                    <prices>
                        <DistanceMatrixElementPrice />
                        <DistanceMatrixElementPrice />
                    </prices>
                </DistanceMatrixElement>"#;
            let distance_element_matrix: Element = xml.parse().unwrap();
            calculate_direct_price(&distance_element_matrix).unwrap();
        }
    }

    mod distance {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn get_direct_price() {
            let xml = r#"<DistanceMatrixElement>
                    <Distance>50</Distance>
                </DistanceMatrixElement>"#;
            let distance_element_matrix: Element = xml.parse().unwrap();
            let distance = get_distance(&distance_element_matrix).unwrap();
            assert_eq!(50, distance);
        }

        #[test]
        #[should_panic(
            expected = "No children with name \\'Distance\\' in Element \\'DistanceMatrixElement\\'"
        )]
        fn no_prices() {
            let xml = r#"<DistanceMatrixElement />"#;
            let distance_element_matrix: Element = xml.parse().unwrap();
            get_distance(&distance_element_matrix).unwrap();
        }
    }

    mod line_id {
        use super::*;
        use pretty_assertions::assert_eq;

        const SERVICE_XML: &str = r#"<ServiceFrame>
                    <lines>
                        <Line id="syn:Line-B42">
                            <KeyList>
                                <KeyValue>
                                    <Key>KV1PlanningLijnNummer</Key>
                                    <Value>B42</Value>
                                </KeyValue>
                            </KeyList>
                        </Line>
                    </lines>
                </ServiceFrame>"#;
        const FARE_FRAME_XML: &str = r#"<FareFrame>
                    <contentValidityConditions>
                        <ValidityTrigger id="vt:1">
                            <TriggerObjectRef ref="syn:Line-B42" nameOfRefClass="Line" />
                        </ValidityTrigger>
                    </contentValidityConditions>
                </FareFrame>"#;

        #[test]
        fn extract_line_id() {
            let service_frame: Element = SERVICE_XML.parse().unwrap();
            let fare_frame: Element = FARE_FRAME_XML.parse().unwrap();
            let line_id = get_line_id(&fare_frame, &service_frame).unwrap();
            assert_eq!("B42", line_id);
        }

        #[test]
        #[should_panic(
            expected = "No children with name \\'contentValidityConditions\\' in Element \\'FareFrame\\'"
        )]
        fn no_content_validations() {
            let fare_frame_xml = r#"<FareFrame />"#;
            let service_frame: Element = SERVICE_XML.parse().unwrap();
            let fare_frame: Element = fare_frame_xml.parse().unwrap();
            get_line_id(&fare_frame, &service_frame).unwrap();
        }

        #[test]
        #[should_panic(expected = "Failed to find a Line reference")]
        fn no_validity_trigger_for_line() {
            let fare_frame_xml = r#"<FareFrame>
                    <contentValidityConditions>
                        <ValidityTrigger>
                            <TriggerObjectRef ref="nw:1" nameOfRefClass="Network" />
                        </ValidityTrigger>
                    </contentValidityConditions>
                </FareFrame>"#;
            let service_frame: Element = SERVICE_XML.parse().unwrap();
            let fare_frame: Element = fare_frame_xml.parse().unwrap();
            get_line_id(&fare_frame, &service_frame).unwrap();
        }

        #[test]
        #[should_panic(expected = "Failed to find a Line reference")]
        fn no_line_ref() {
            let fare_frame_xml = r#"<FareFrame>
                    <contentValidityConditions>
                        <ValidityTrigger>
                            <TriggerObjectRef nameOfRefClass="Line" />
                        </ValidityTrigger>
                    </contentValidityConditions>
                </FareFrame>"#;
            let service_frame: Element = SERVICE_XML.parse().unwrap();
            let fare_frame: Element = fare_frame_xml.parse().unwrap();
            get_line_id(&fare_frame, &service_frame).unwrap();
        }

        #[test]
        #[should_panic(expected = "Failed to find a Line reference")]
        fn multiple_line_ref() {
            let fare_frame_xml = r#"<FareFrame>
                    <contentValidityConditions>
                        <ValidityTrigger>
                            <TriggerObjectRef ref="syn:Line-B42" nameOfRefClass="Line" />
                            <TriggerObjectRef ref="syn:Line-Other" nameOfRefClass="Line" />
                        </ValidityTrigger>
                    </contentValidityConditions>
                </FareFrame>"#;
            let service_frame: Element = SERVICE_XML.parse().unwrap();
            let fare_frame: Element = fare_frame_xml.parse().unwrap();
            let line_id = get_line_id(&fare_frame, &service_frame).unwrap();
            assert_eq!("Bla", line_id);
        }

        #[test]
        #[should_panic(
            expected = "No children with name \\'lines\\' in Element \\'ServiceFrame\\'"
        )]
        fn no_lines() {
            let service_xml = r#"<ServiceFrame />"#;
            let service_frame: Element = service_xml.parse().unwrap();
            let fare_frame: Element = FARE_FRAME_XML.parse().unwrap();
            get_line_id(&fare_frame, &service_frame).unwrap();
        }

        #[test]
        #[should_panic(expected = "Failed to find the Line with identifier \\'syn:Line-B42\\'")]
        fn no_line() {
            let service_xml = r#"<ServiceFrame>
                    <lines>
                        <Line id="OtherID">
                            <KeyList>
                                <KeyValue>
                                    <Key>KV1PlanningLijnNummer</Key>
                                    <Value>B42</Value>
                                </KeyValue>
                            </KeyList>
                        </Line>
                    </lines>
                </ServiceFrame>"#;
            let service_frame: Element = service_xml.parse().unwrap();
            let fare_frame: Element = FARE_FRAME_XML.parse().unwrap();
            get_line_id(&fare_frame, &service_frame).unwrap();
        }

        #[test]
        #[should_panic(expected = "Failed to find the Line with identifier \\'syn:Line-B42\\'")]
        fn no_unique_line() {
            let service_xml = r#"<ServiceFrame>
                    <lines>
                        <Line id="syn:Line-B42">
                            <KeyList>
                                <KeyValue>
                                    <Key>KV1PlanningLijnNummer</Key>
                                    <Value>B42</Value>
                                </KeyValue>
                            </KeyList>
                        </Line>
                        <Line id="syn:Line-B42">
                            <KeyList>
                                <KeyValue>
                                    <Key>KV1PlanningLijnNummer</Key>
                                    <Value>B42</Value>
                                </KeyValue>
                            </KeyList>
                        </Line>
                    </lines>
                </ServiceFrame>"#;
            let service_frame: Element = service_xml.parse().unwrap();
            let fare_frame: Element = FARE_FRAME_XML.parse().unwrap();
            get_line_id(&fare_frame, &service_frame).unwrap();
        }
    }

    mod origin_destination {
        use super::*;
        use pretty_assertions::assert_eq;

        const PREFIX_WITH_COLON: &str = "NTM:";

        const SERVICE_XML: &str = r#"<ServiceFrame>
                <scheduledStopPoints>
                    <ScheduledStopPoint id="syn:ssp:1">
                        <projections>
                            <PointProjection>
                                <ProjectedPointRef ref="syn:sp:1" />
                            </PointProjection>
                        </projections>
                    </ScheduledStopPoint>
                    <ScheduledStopPoint id="syn:ssp:2">
                        <projections>
                            <PointProjection>
                                <ProjectedPointRef ref="syn:sp:2" />
                            </PointProjection>
                            <PointProjection>
                                <ProjectedPointRef ref="syn:sp:3" />
                            </PointProjection>
                        </projections>
                    </ScheduledStopPoint>
                </scheduledStopPoints>
            </ServiceFrame>"#;
        const DISTANCE_MATRIX_ELEMENT_XML: &str = r#"<DistanceMatrixElement>
                <Distance>50</Distance>
                <StartStopPointRef ref="syn:ssp:1" />
                <EndStopPointRef ref="syn:ssp:2" />
            </DistanceMatrixElement>"#;

        fn init_collections() -> Collections {
            let mut collections = Collections::default();
            let sa1 = StopArea {
                id: format!("{}sa:1", PREFIX_WITH_COLON),
                ..Default::default()
            };
            let sa2 = StopArea {
                id: format!("{}sa:2", PREFIX_WITH_COLON),
                ..Default::default()
            };
            let sa3 = StopArea {
                id: format!("{}sa:3", PREFIX_WITH_COLON),
                ..Default::default()
            };
            let sp1 = StopPoint {
                id: format!("{}sp:1", PREFIX_WITH_COLON),
                stop_area_id: format!("{}sa:1", PREFIX_WITH_COLON),
                ..Default::default()
            };
            let sp2 = StopPoint {
                id: format!("{}sp:2", PREFIX_WITH_COLON),
                stop_area_id: format!("{}sa:2", PREFIX_WITH_COLON),
                ..Default::default()
            };
            let sp3 = StopPoint {
                id: format!("{}sp:3", PREFIX_WITH_COLON),
                stop_area_id: format!("{}sa:3", PREFIX_WITH_COLON),
                ..Default::default()
            };
            collections.stop_areas.push(sa1).unwrap();
            collections.stop_areas.push(sa2).unwrap();
            collections.stop_areas.push(sa3).unwrap();
            collections.stop_points.push(sp1).unwrap();
            collections.stop_points.push(sp2).unwrap();
            collections.stop_points.push(sp3).unwrap();
            collections
        }

        #[test]
        fn extract_ticket_use_restriction_od() {
            let collections = init_collections();
            let service_frame: Element = SERVICE_XML.parse().unwrap();
            let distance_matrix_element: Element = DISTANCE_MATRIX_ELEMENT_XML.parse().unwrap();
            let ticket_use_restrictions = get_origin_destinations(
                &collections,
                &service_frame,
                &distance_matrix_element,
                PREFIX_WITH_COLON,
            )
            .unwrap();
            assert_eq!(2, ticket_use_restrictions.len());
            let ticket_use_restriction = &ticket_use_restrictions[0];
            assert_eq!(
                format!("{}sa:1", PREFIX_WITH_COLON),
                ticket_use_restriction.0
            );
            assert_eq!(
                format!("{}sa:2", PREFIX_WITH_COLON),
                ticket_use_restriction.1
            );
            let ticket_use_restriction = &ticket_use_restrictions[1];
            assert_eq!(
                format!("{}sa:1", PREFIX_WITH_COLON),
                ticket_use_restriction.0
            );
            assert_eq!(
                format!("{}sa:3", PREFIX_WITH_COLON),
                ticket_use_restriction.1
            );
        }

        #[test]
        #[should_panic(
            expected = "No children with name \\'StartStopPointRef\\' in Element \\'DistanceMatrixElement\\'"
        )]
        fn no_start_stop_point_ref() {
            let collections = init_collections();
            let service_frame: Element = SERVICE_XML.parse().unwrap();
            let distance_matrix_element_xml = r#"<DistanceMatrixElement>
                <Distance>50</Distance>
                <EndStopPointRef ref="syn:ssp:2" />
            </DistanceMatrixElement>"#;
            let distance_matrix_element: Element = distance_matrix_element_xml.parse().unwrap();
            get_origin_destinations(
                &collections,
                &service_frame,
                &distance_matrix_element,
                PREFIX_WITH_COLON,
            )
            .unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find attribute \\'ref\\' in element \\'StartStopPointRef\\'"
        )]
        fn no_start_stop_point_ref_reference() {
            let collections = init_collections();
            let service_frame: Element = SERVICE_XML.parse().unwrap();
            let distance_matrix_element_xml = r#"<DistanceMatrixElement>
                <Distance>50</Distance>
                <StartStopPointRef />
                <EndStopPointRef ref="syn:ssp:2" />
            </DistanceMatrixElement>"#;
            let distance_matrix_element: Element = distance_matrix_element_xml.parse().unwrap();
            get_origin_destinations(
                &collections,
                &service_frame,
                &distance_matrix_element,
                PREFIX_WITH_COLON,
            )
            .unwrap();
        }

        #[test]
        #[should_panic(
            expected = "No children with name \\'scheduledStopPoints\\' in Element \\'ServiceFrame\\'"
        )]
        fn no_scheduled_stop_points() {
            let collections = init_collections();
            let service_xml = r#"<ServiceFrame />"#;
            let service_frame: Element = service_xml.parse().unwrap();
            let distance_matrix_element: Element = DISTANCE_MATRIX_ELEMENT_XML.parse().unwrap();
            get_origin_destinations(
                &collections,
                &service_frame,
                &distance_matrix_element,
                PREFIX_WITH_COLON,
            )
            .unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a unique \\'ScheduledStopPoint\\' with reference \\'syn:ssp:2\\'"
        )]
        fn scheduled_stop_point_not_found() {
            let collections = init_collections();
            let service_xml = r#"<ServiceFrame>
                <scheduledStopPoints>
                    <ScheduledStopPoint id="syn:ssp:1">
                        <projections>
                            <PointProjection>
                                <ProjectedPointRef ref="syn:sp:1" />
                            </PointProjection>
                        </projections>
                    </ScheduledStopPoint>
                    <ScheduledStopPoint id="syn:ssp:42" />
                </scheduledStopPoints>
            </ServiceFrame>"#;
            let service_frame: Element = service_xml.parse().unwrap();
            let distance_matrix_element: Element = DISTANCE_MATRIX_ELEMENT_XML.parse().unwrap();
            get_origin_destinations(
                &collections,
                &service_frame,
                &distance_matrix_element,
                PREFIX_WITH_COLON,
            )
            .unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a unique \\'ScheduledStopPoint\\' with reference \\'syn:ssp:1\\'"
        )]
        fn multiple_scheduled_stop_points_found() {
            let collections = init_collections();
            let service_xml = r#"<ServiceFrame>
                <scheduledStopPoints>
                    <ScheduledStopPoint id="syn:ssp:1" />
                    <ScheduledStopPoint id="syn:ssp:1" />
                </scheduledStopPoints>
            </ServiceFrame>"#;
            let service_frame: Element = service_xml.parse().unwrap();
            let distance_matrix_element: Element = DISTANCE_MATRIX_ELEMENT_XML.parse().unwrap();
            get_origin_destinations(
                &collections,
                &service_frame,
                &distance_matrix_element,
                PREFIX_WITH_COLON,
            )
            .unwrap();
        }

        #[test]
        #[should_panic(
            expected = "No children with name \\'projections\\' in Element \\'ScheduledStopPoint\\'"
        )]
        fn no_projections() {
            let collections = init_collections();
            let service_xml = r#"<ServiceFrame>
                <scheduledStopPoints>
                    <ScheduledStopPoint id="syn:ssp:1">
                        <projections>
                            <PointProjection>
                                <ProjectedPointRef ref="syn:sp:1" />
                            </PointProjection>
                        </projections>
                    </ScheduledStopPoint>
                    <ScheduledStopPoint id="syn:ssp:2" />
                </scheduledStopPoints>
            </ServiceFrame>"#;
            let service_frame: Element = service_xml.parse().unwrap();
            let distance_matrix_element: Element = DISTANCE_MATRIX_ELEMENT_XML.parse().unwrap();
            get_origin_destinations(
                &collections,
                &service_frame,
                &distance_matrix_element,
                PREFIX_WITH_COLON,
            )
            .unwrap();
        }

        #[test]
        fn no_point_projection() {
            let collections = init_collections();
            let service_xml = r#"<ServiceFrame>
                <scheduledStopPoints>
                    <ScheduledStopPoint id="syn:ssp:1">
                        <projections>
                            <PointProjection>
                                <ProjectedPointRef ref="syn:sp:1" />
                            </PointProjection>
                        </projections>
                    </ScheduledStopPoint>
                    <ScheduledStopPoint id="syn:ssp:2">
                        <projections />
                    </ScheduledStopPoint>
                </scheduledStopPoints>
            </ServiceFrame>"#;
            let service_frame: Element = service_xml.parse().unwrap();
            let distance_matrix_element: Element = DISTANCE_MATRIX_ELEMENT_XML.parse().unwrap();
            let origin_destinations = get_origin_destinations(
                &collections,
                &service_frame,
                &distance_matrix_element,
                PREFIX_WITH_COLON,
            )
            .unwrap();
            assert_eq!(0, origin_destinations.len());
        }

        #[test]
        fn no_stop_point() {
            let collections = init_collections();
            let service_xml = r#"<ServiceFrame>
                <scheduledStopPoints>
                    <ScheduledStopPoint id="syn:ssp:1">
                        <projections>
                            <PointProjection>
                                <ProjectedPointRef ref="syn:sp:42" />
                            </PointProjection>
                        </projections>
                    </ScheduledStopPoint>
                    <ScheduledStopPoint id="syn:ssp:2">
                        <projections>
                            <PointProjection>
                                <ProjectedPointRef ref="syn:sp:2" />
                            </PointProjection>
                            <PointProjection>
                                <ProjectedPointRef ref="syn:sp:3" />
                            </PointProjection>
                        </projections>
                    </ScheduledStopPoint>
                </scheduledStopPoints>
            </ServiceFrame>"#;
            let service_frame: Element = service_xml.parse().unwrap();
            let distance_matrix_element: Element = DISTANCE_MATRIX_ELEMENT_XML.parse().unwrap();
            let origin_destinations = get_origin_destinations(
                &collections,
                &service_frame,
                &distance_matrix_element,
                PREFIX_WITH_COLON,
            )
            .unwrap();
            assert_eq!(0, origin_destinations.len());
        }
    }
}
