// Copyright 2017 Kisio Digital and/or its affiliates.
//
// This program is free software: you can redistribute it and/or
// modify it under the terms of the GNU General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see
// <http://www.gnu.org/licenses/>.

use super::utils;
use super::utils::FrameType;
use super::{TryAttribute, TryOnlyChild};
use crate::model::Collections;
use crate::objects::*;
use crate::Result;
use failure::{bail, format_err};
use log::{info, warn};
use minidom::Element;
use std::collections::BTreeSet;
use std::convert::{From, TryFrom};
use std::fs;
use std::io::Read;
use std::path::Path;
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
        let id = distance_matrix_element.try_attribute("id")?;
        let ticket = Ticket {
            id,
            name: "Ticket Origin-Destination".to_string(),
            comment: None,
        };
        Ok(ticket)
    }
}

impl TryFrom<(String, f64, String, (Date, Date))> for TicketPrice {
    type Error = failure::Error;
    fn try_from(
        (ticket_id, price, currency, validity): (String, f64, String, (Date, Date)),
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

fn calculate_direct_price(distance_matrix_element: &Element) -> Result<f64> {
    let distance_matrix_element_price = distance_matrix_element
        .try_only_child("prices")?
        .try_only_child("DistanceMatrixElementPrice")?;
    Ok(utils::get_amount_units_factor(
        distance_matrix_element_price,
    )?)
}

fn get_distance(distance_matrix_element: &Element) -> Result<f64> {
    let distance_str = distance_matrix_element.try_only_child("Distance")?.text();
    distance_str
        .parse()
        .map_err(|_| format_err!("Failed to parse '{}' into a 'f64'", distance_str))
}

fn get_line_id(fare_frame: &Element, service_frame: &Element) -> Result<String> {
    fn get_line_ref<'a>(fare_frame: &'a Element) -> Result<&'a str> {
        let references: Vec<_> = fare_frame
            .try_only_child("contentValidityConditions")?
            .children()
            .filter(|element| element.name() == "ValidityTrigger")
            .flat_map(|validity_trigger| validity_trigger.children())
            .filter(|element| element.name() == "TriggerObjectRef")
            .filter(|trigger_object_ref| {
                trigger_object_ref
                    .try_attribute::<String>("nameOfRefClass")
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
            .try_only_child("lines")?
            .children()
            .filter(|element| element.name() == "Line")
            .filter(|line| {
                line.try_attribute::<String>("id")
                    .map(|id| id == line_ref)
                    .unwrap_or(false)
            })
            .map(|line| utils::get_value_in_keylist(line, "KV1PlanningLijnNummer"))
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
) -> Result<Vec<(String, String)>> {
    fn get_ref(distance_matrix_element: &Element, element_name: &str) -> Result<String> {
        distance_matrix_element
            .try_only_child(element_name)?
            .try_attribute("ref")
    }
    let start_stop_point_ref = get_ref(distance_matrix_element, "StartStopPointRef")?;
    let end_stop_point_ref = get_ref(distance_matrix_element, "EndStopPointRef")?;
    let scheduled_stop_points = service_frame.try_only_child("scheduledStopPoints")?;
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
        let stop_point_ids = scheduled_stop_point
            .try_only_child("projections")?
            .children()
            .filter(|element| element.name() == "PointProjection")
            .flat_map(|point_projection| point_projection.children())
            .filter(|element| element.name() == "ProjectedPointRef")
            .flat_map(|projected_point_ref| projected_point_ref.attr("ref"))
            .collect();
        Ok(stop_point_ids)
    }
    let start_stop_point_ids = get_stop_point_ids(scheduled_stop_points, &start_stop_point_ref)?;
    let end_stop_point_ids = get_stop_point_ids(scheduled_stop_points, &end_stop_point_ref)?;
    let start_stop_area_ids: BTreeSet<_> = start_stop_point_ids
        .iter()
        .flat_map(|stop_point_id| collections.stop_points.get(*stop_point_id))
        .map(|stop_point| stop_point.stop_area_id.clone())
        .collect();
    let end_stop_area_ids: BTreeSet<_> = end_stop_point_ids
        .iter()
        .flat_map(|stop_point_id| collections.stop_points.get(*stop_point_id))
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

fn round_price(price: f64, rounding_rule: f64) -> f64 {
    (price / rounding_rule).round() * rounding_rule
}

fn load_netex_fares(collections: &mut Collections, root: &Element) -> Result<()> {
    let frames = utils::get_fare_frames(root)?;
    let unit_price_frame = utils::get_only_frame(&frames, FrameType::UnitPrice)?;
    let service_frame = utils::get_only_frame(&frames, FrameType::Service)?;
    let resource_frame = utils::get_only_frame(&frames, FrameType::Resource)?;
    let unit_price = utils::get_unit_price(unit_price_frame)?;
    let validity = utils::get_validity(resource_frame)?;
    for frame_type in &[FrameType::DistanceMatrix, FrameType::DirectPriceMatrix] {
        if let Some(fare_frames) = frames.get(frame_type) {
            for fare_frame in fare_frames {
                let boarding_fee: f64 =
                    utils::get_value_in_keylist(fare_frame, "EntranceRateWrtCurrency")?;
                let rounding_rule: f64 =
                    utils::get_value_in_keylist(fare_frame, "RoundingWrtCurrencyRule")?;
                let currency = utils::get_currency(fare_frame)?;
                let distance_matrix_elements = utils::get_distance_matrix_elements(fare_frame)?;
                for distance_matrix_element in distance_matrix_elements {
                    let ticket = Ticket::try_from(distance_matrix_element)?;
                    let price = match frame_type {
                        FrameType::DirectPriceMatrix => {
                            boarding_fee + calculate_direct_price(distance_matrix_element)?
                        }
                        FrameType::DistanceMatrix => {
                            boarding_fee + unit_price * get_distance(distance_matrix_element)?
                        }
                        _ => continue,
                    };
                    let price = round_price(price, rounding_rule);
                    let ticket_price = TicketPrice::try_from((
                        ticket.id.clone(),
                        price,
                        currency.clone(),
                        validity,
                    ))?;
                    let ticket_use = TicketUse::from(ticket.id.clone());
                    let line_id = get_line_id(fare_frame, service_frame)?;
                    let ticket_use_perimeter =
                        TicketUsePerimeter::from((ticket_use.id.clone(), line_id));
                    let origin_destinations = get_origin_destinations(
                        &*collections,
                        service_frame,
                        distance_matrix_element,
                    )?;
                    if !origin_destinations.is_empty() {
                        for origin_destination in origin_destinations {
                            let ticket_use_restriction = TicketUseRestriction::from((
                                ticket_use.id.clone(),
                                origin_destination,
                            ));
                            collections
                                .ticket_use_restrictions
                                .push(ticket_use_restriction);
                        }
                        collections.tickets.push(ticket)?;
                        collections.ticket_uses.push(ticket_use)?;
                        collections.ticket_prices.push(ticket_price);
                        collections.ticket_use_perimeters.push(ticket_use_perimeter);
                    }
                }
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
/// https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/ntfs_fare_extension.md)
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
        let mut zip_archive = skip_fail!(ZipArchive::new(zip_file));
        for i in 0..zip_archive.len() {
            let mut zip_file = zip_archive.by_index(i)?;
            match zip_file.sanitized_name().extension() {
                Some(ext) if ext == "xml" => {
                    info!("reading fares file {:?}", zip_file.sanitized_name());
                    let mut file_content = String::new();
                    zip_file.read_to_string(&mut file_content)?;
                    let root: Element = file_content.parse()?;
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
    mod ticket {
        use crate::objects::Ticket;
        use minidom::Element;
        use pretty_assertions::assert_eq;
        use std::convert::TryFrom;

        #[test]
        fn extract_ticket() {
            let xml = r#"<DistanceMatrixElement id="ticket:1" />"#;
            let distance_matrix_element: Element = xml.parse().unwrap();
            let ticket = Ticket::try_from(&distance_matrix_element).unwrap();
            assert_eq!(ticket.id, "ticket:1");
            assert_eq!(ticket.name, "Ticket Origin-Destination");
            assert_eq!(ticket.comment, None);
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
        use crate::objects::TicketPrice;
        use approx::assert_relative_eq;
        use chrono::NaiveDate;
        use pretty_assertions::assert_eq;
        use std::convert::TryFrom;

        #[test]
        fn valid_ticket_price() {
            let ticket_id = String::from("ticket:1");
            let price = 4.2;
            let currency = String::from("EUR");
            let validity_start = NaiveDate::from_ymd(2019, 2, 7);
            let validity_end = NaiveDate::from_ymd(2019, 3, 14);
            let ticket_price =
                TicketPrice::try_from((ticket_id, price, currency, (validity_start, validity_end)))
                    .unwrap();
            assert_eq!(ticket_price.ticket_id, String::from("ticket:1"));
            assert_relative_eq!(ticket_price.price, 4.2);
            assert_eq!(ticket_price.currency, String::from("EUR"));
            assert_eq!(ticket_price.ticket_validity_start, validity_start);
            assert_eq!(ticket_price.ticket_validity_end, validity_end);
        }

        #[test]
        #[should_panic(expected = "Failed to convert \\'XXX\\' as a currency")]
        fn invalid_currency() {
            let ticket_id = String::from("ticket:1");
            let price = 4.2;
            let currency = String::from("XXX");
            let validity_start = NaiveDate::from_ymd(2019, 2, 7);
            let validity_end = NaiveDate::from_ymd(2019, 3, 14);
            TicketPrice::try_from((ticket_id, price, currency, (validity_start, validity_end)))
                .unwrap();
        }
    }

    mod ticket_use {
        use crate::objects::TicketUse;
        use pretty_assertions::assert_eq;
        use std::convert::From;

        #[test]
        fn valid_ticket_use() {
            let ticket_id = String::from("ticket:1");
            let ticket_use = TicketUse::from(ticket_id);
            assert_eq!(ticket_use.id, String::from("TU:ticket:1"));
            assert_eq!(ticket_use.ticket_id, String::from("ticket:1"));
            assert_eq!(ticket_use.max_transfers.unwrap(), 0);
            assert_eq!(ticket_use.boarding_time_limit, None);
            assert_eq!(ticket_use.alighting_time_limit, None);
        }
    }

    mod ticket_use_perimeter {
        use crate::objects::{ObjectType, PerimeterAction, TicketUsePerimeter};
        use pretty_assertions::assert_eq;
        use std::convert::From;

        #[test]
        fn valid_ticket_use() {
            let ticket_use_id = String::from("ticket_use:1");
            let line_id = String::from("line:1");
            let ticket_use_perimeter = TicketUsePerimeter::from((ticket_use_id, line_id));
            assert_eq!(
                ticket_use_perimeter.ticket_use_id,
                String::from("ticket_use:1")
            );
            assert_eq!(ticket_use_perimeter.object_id, String::from("line:1"));
            assert_eq!(ticket_use_perimeter.object_type, ObjectType::Line);
            assert_eq!(
                ticket_use_perimeter.perimeter_action,
                PerimeterAction::Included
            );
        }
    }

    mod ticket_use_restriction {
        use crate::objects::{RestrictionType, TicketUseRestriction};
        use pretty_assertions::assert_eq;
        use std::convert::From;

        #[test]
        fn valid_ticket_use() {
            let ticket_use_id = String::from("ticket_use:1");
            let origin = String::from("stop_area:1");
            let destination = String::from("stop_area:2");
            let ticket_use_restriction =
                TicketUseRestriction::from((ticket_use_id, (origin, destination)));
            assert_eq!(
                ticket_use_restriction.ticket_use_id,
                String::from("ticket_use:1")
            );
            assert_eq!(
                ticket_use_restriction.restriction_type,
                RestrictionType::OriginDestination
            );
            assert_eq!(
                ticket_use_restriction.use_origin,
                String::from("stop_area:1")
            );
            assert_eq!(
                ticket_use_restriction.use_destination,
                String::from("stop_area:2")
            );
        }
    }

    mod direct_price {
        use super::super::calculate_direct_price;
        use minidom::Element;
        use pretty_assertions::assert_eq;

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
            assert_eq!(price, 21.0);
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'prices\\' in element \\'DistanceMatrixElement\\'"
        )]
        fn no_prices() {
            let xml = r#"<DistanceMatrixElement />"#;
            let distance_element_matrix: Element = xml.parse().unwrap();
            calculate_direct_price(&distance_element_matrix).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'DistanceMatrixElementPrice\\' in element \\'prices\\'"
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
            expected = "Failed to find a unique child \\'DistanceMatrixElementPrice\\' in element \\'prices\\'"
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
        use super::super::get_distance;
        use minidom::Element;
        use pretty_assertions::assert_eq;

        #[test]
        fn get_direct_price() {
            let xml = r#"<DistanceMatrixElement>
                    <Distance>50</Distance>
                </DistanceMatrixElement>"#;
            let distance_element_matrix: Element = xml.parse().unwrap();
            let distance = get_distance(&distance_element_matrix).unwrap();
            assert_eq!(distance, 50.0);
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'Distance\\' in element \\'DistanceMatrixElement\\'"
        )]
        fn no_prices() {
            let xml = r#"<DistanceMatrixElement />"#;
            let distance_element_matrix: Element = xml.parse().unwrap();
            get_distance(&distance_element_matrix).unwrap();
        }
    }

    mod line_id {
        use super::super::get_line_id;
        use minidom::Element;
        use pretty_assertions::assert_eq;

        const SERVICE_XML: &'static str = r#"<ServiceFrame>
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
        const FARE_FRAME_XML: &'static str = r#"<FareFrame>
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
            assert_eq!(line_id, "B42");
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'contentValidityConditions\\' in element \\'FareFrame\\'"
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
            assert_eq!(line_id, "Bla");
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'lines\\' in element \\'ServiceFrame\\'"
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
        use super::super::get_origin_destinations;
        use crate::{model::Collections, objects::*};
        use minidom::Element;
        use pretty_assertions::assert_eq;
        use std::default::Default;

        const SERVICE_XML: &'static str = r#"<ServiceFrame>
                <scheduledStopPoints>
                    <ScheduledStopPoint id="ssp:1">
                        <projections>
                            <PointProjection>
                                <ProjectedPointRef ref="sp:1" />
                            </PointProjection>
                        </projections>
                    </ScheduledStopPoint>
                    <ScheduledStopPoint id="ssp:2">
                        <projections>
                            <PointProjection>
                                <ProjectedPointRef ref="sp:2" />
                            </PointProjection>
                            <PointProjection>
                                <ProjectedPointRef ref="sp:3" />
                            </PointProjection>
                        </projections>
                    </ScheduledStopPoint>
                </scheduledStopPoints>
            </ServiceFrame>"#;
        const DISTANCE_MATRIX_ELEMENT_XML: &'static str = r#"<DistanceMatrixElement>
                <Distance>50</Distance>
                <StartStopPointRef ref="ssp:1" />
                <EndStopPointRef ref="ssp:2" />
            </DistanceMatrixElement>"#;

        fn init_collections() -> Collections {
            let mut collections = Collections::default();
            let sa1 = StopArea {
                id: String::from("sa:1"),
                ..Default::default()
            };
            let sa2 = StopArea {
                id: String::from("sa:2"),
                ..Default::default()
            };
            let sa3 = StopArea {
                id: String::from("sa:3"),
                ..Default::default()
            };
            let sp1 = StopPoint {
                id: String::from("sp:1"),
                stop_area_id: String::from("sa:1"),
                ..Default::default()
            };
            let sp2 = StopPoint {
                id: String::from("sp:2"),
                stop_area_id: String::from("sa:2"),
                ..Default::default()
            };
            let sp3 = StopPoint {
                id: String::from("sp:3"),
                stop_area_id: String::from("sa:3"),
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
            let ticket_use_restrictions =
                get_origin_destinations(&collections, &service_frame, &distance_matrix_element)
                    .unwrap();
            assert_eq!(ticket_use_restrictions.len(), 2);
            let ticket_use_restriction = &ticket_use_restrictions[0];
            assert_eq!(ticket_use_restriction.0, "sa:1");
            assert_eq!(ticket_use_restriction.1, "sa:2");
            let ticket_use_restriction = &ticket_use_restrictions[1];
            assert_eq!(ticket_use_restriction.0, "sa:1");
            assert_eq!(ticket_use_restriction.1, "sa:3");
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'StartStopPointRef\\' in element \\'DistanceMatrixElement\\'"
        )]
        fn no_start_stop_point_ref() {
            let collections = init_collections();
            let service_frame: Element = SERVICE_XML.parse().unwrap();
            let distance_matrix_element_xml = r#"<DistanceMatrixElement>
                <Distance>50</Distance>
                <EndStopPointRef ref="ssp:2" />
            </DistanceMatrixElement>"#;
            let distance_matrix_element: Element = distance_matrix_element_xml.parse().unwrap();
            get_origin_destinations(&collections, &service_frame, &distance_matrix_element)
                .unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find attribute \\'id\\' in element \\'StartStopPointRef\\'"
        )]
        fn no_start_stop_point_ref_reference() {
            let collections = init_collections();
            let service_frame: Element = SERVICE_XML.parse().unwrap();
            let distance_matrix_element_xml = r#"<DistanceMatrixElement>
                <Distance>50</Distance>
                <StartStopPointRef />
                <EndStopPointRef ref="ssp:2" />
            </DistanceMatrixElement>"#;
            let distance_matrix_element: Element = distance_matrix_element_xml.parse().unwrap();
            get_origin_destinations(&collections, &service_frame, &distance_matrix_element)
                .unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'scheduledStopPoints\\' in element \\'ServiceFrame\\'"
        )]
        fn no_scheduled_stop_points() {
            let collections = init_collections();
            let service_xml = r#"<ServiceFrame />"#;
            let service_frame: Element = service_xml.parse().unwrap();
            let distance_matrix_element: Element = DISTANCE_MATRIX_ELEMENT_XML.parse().unwrap();
            get_origin_destinations(&collections, &service_frame, &distance_matrix_element)
                .unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a unique \\'ScheduledStopPoint\\' with reference \\'ssp:2\\'"
        )]
        fn scheduled_stop_point_not_found() {
            let collections = init_collections();
            let service_xml = r#"<ServiceFrame>
                <scheduledStopPoints>
                    <ScheduledStopPoint id="ssp:1">
                        <projections>
                            <PointProjection>
                                <ProjectedPointRef ref="sp:1" />
                            </PointProjection>
                        </projections>
                    </ScheduledStopPoint>
                    <ScheduledStopPoint id="ssp:42" />
                </scheduledStopPoints>
            </ServiceFrame>"#;
            let service_frame: Element = service_xml.parse().unwrap();
            let distance_matrix_element: Element = DISTANCE_MATRIX_ELEMENT_XML.parse().unwrap();
            get_origin_destinations(&collections, &service_frame, &distance_matrix_element)
                .unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a unique \\'ScheduledStopPoint\\' with reference \\'ssp:1\\'"
        )]
        fn multiple_scheduled_stop_points_found() {
            let collections = init_collections();
            let service_xml = r#"<ServiceFrame>
                <scheduledStopPoints>
                    <ScheduledStopPoint id="ssp:1" />
                    <ScheduledStopPoint id="ssp:1" />
                </scheduledStopPoints>
            </ServiceFrame>"#;
            let service_frame: Element = service_xml.parse().unwrap();
            let distance_matrix_element: Element = DISTANCE_MATRIX_ELEMENT_XML.parse().unwrap();
            get_origin_destinations(&collections, &service_frame, &distance_matrix_element)
                .unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'projections\\' in element \\'ScheduledStopPoint\\'"
        )]
        fn no_projections() {
            let collections = init_collections();
            let service_xml = r#"<ServiceFrame>
                <scheduledStopPoints>
                    <ScheduledStopPoint id="ssp:1">
                        <projections>
                            <PointProjection>
                                <ProjectedPointRef ref="sp:1" />
                            </PointProjection>
                        </projections>
                    </ScheduledStopPoint>
                    <ScheduledStopPoint id="ssp:2" />
                </scheduledStopPoints>
            </ServiceFrame>"#;
            let service_frame: Element = service_xml.parse().unwrap();
            let distance_matrix_element: Element = DISTANCE_MATRIX_ELEMENT_XML.parse().unwrap();
            get_origin_destinations(&collections, &service_frame, &distance_matrix_element)
                .unwrap();
        }

        #[test]
        fn no_point_projection() {
            let collections = init_collections();
            let service_xml = r#"<ServiceFrame>
                <scheduledStopPoints>
                    <ScheduledStopPoint id="ssp:1">
                        <projections>
                            <PointProjection>
                                <ProjectedPointRef ref="sp:1" />
                            </PointProjection>
                        </projections>
                    </ScheduledStopPoint>
                    <ScheduledStopPoint id="ssp:2">
                        <projections />
                    </ScheduledStopPoint>
                </scheduledStopPoints>
            </ServiceFrame>"#;
            let service_frame: Element = service_xml.parse().unwrap();
            let distance_matrix_element: Element = DISTANCE_MATRIX_ELEMENT_XML.parse().unwrap();
            let origin_destinations =
                get_origin_destinations(&collections, &service_frame, &distance_matrix_element)
                    .unwrap();
            assert_eq!(origin_destinations.len(), 0);
        }

        #[test]
        fn no_stop_point() {
            let collections = init_collections();
            let service_xml = r#"<ServiceFrame>
                <scheduledStopPoints>
                    <ScheduledStopPoint id="ssp:1">
                        <projections>
                            <PointProjection>
                                <ProjectedPointRef ref="sp:42" />
                            </PointProjection>
                        </projections>
                    </ScheduledStopPoint>
                    <ScheduledStopPoint id="ssp:2">
                        <projections>
                            <PointProjection>
                                <ProjectedPointRef ref="sp:2" />
                            </PointProjection>
                            <PointProjection>
                                <ProjectedPointRef ref="sp:3" />
                            </PointProjection>
                        </projections>
                    </ScheduledStopPoint>
                </scheduledStopPoints>
            </ServiceFrame>"#;
            let service_frame: Element = service_xml.parse().unwrap();
            let distance_matrix_element: Element = DISTANCE_MATRIX_ELEMENT_XML.parse().unwrap();
            let origin_destinations =
                get_origin_destinations(&collections, &service_frame, &distance_matrix_element)
                    .unwrap();
            assert_eq!(origin_destinations.len(), 0);
        }
    }

    mod round_price {
        use super::super::round_price;
        use approx::assert_relative_eq;

        #[test]
        fn round_with_unit() {
            let price = round_price(12.0123, 1.0);
            assert_relative_eq!(price, 12.0);
        }

        #[test]
        fn round_with_cents() {
            let price = round_price(12.0123, 0.01);
            assert_relative_eq!(price, 12.01);
        }

        #[test]
        fn round_on_half() {
            let price = round_price(12.125, 0.01);
            assert_relative_eq!(price, 12.13);
        }
    }
}
