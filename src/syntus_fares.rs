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

//! See function read
use crate::collection::{Collection, CollectionWithId};
use crate::objects::{ODRule, StopPoint, Ticket};
use crate::Result;
use chrono::NaiveDate;
use failure::bail;
use failure::format_err;
use log::{info, warn};
use minidom::Element;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path;
use zip;
const DATE_TIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S.0Z";

impl Ticket {
    fn new(id: String, start_date: NaiveDate, end_date: NaiveDate, price: f64) -> Self {
        Ticket {
            id,
            start_date,
            end_date,
            price,
            currency_type: "EUR".to_string(),
            validity_duration: None,
            transfers: None,
        }
    }
}

impl ODRule {
    fn new(
        id: String,
        origin_stop_area_id: String,
        destination_stop_area_id: String,
        ticket_id: String,
    ) -> Self {
        ODRule {
            id,
            origin_stop_area_id,
            destination_stop_area_id,
            ticket_id,
            line_id: None,
            network_id: None,
            physical_mode_id: Some("Bus".to_string()),
        }
    }
}

fn get_value_for_key(key_list_container: &Element, key: &str, name_space: &str) -> Result<f64> {
    let key_list = key_list_container
        .get_child("KeyList", &name_space)
        .ok_or_else(|| format_err!("no KeyList found within {}", key_list_container.name()))?;
    key_list
        .children()
        .find(|key_value| key_value.get_child("Key", &name_space).unwrap().text() == key)
        .map(|key_value| {
            key_value
                .get_child("Value", &name_space)
                .unwrap()
                .text()
                .parse::<f64>()
                .unwrap()
        })
        .ok_or_else(|| format_err!("no value found for key {:?}", &key))
}

fn get_list_element_from_inner_list<'a>(
    element: &'a Element,
    list_tag: &str,
    list_element_tag: &str,
    name_space: &str,
) -> Result<&'a Element> {
    let structures = element
        .get_child(list_tag, name_space)
        .ok_or_else(|| format_err!("{} has no {} element", element.name(), list_tag))?;
    if structures.children().count() != 1 {
        bail!(
            "unable to select reference {} from {}/{}",
            list_element_tag,
            element.name(),
            element.attr("id").unwrap()
        );
    }
    structures
        .get_child(list_element_tag, name_space)
        .ok_or_else(|| format_err!("{} has no {} element", list_tag, list_element_tag))
}

fn load_syntus_file<R: Read>(
    mut file: R,
    stop_code_to_stop_area: &HashMap<&str, &str>,
    tickets: &mut Vec<Ticket>,
    od_rules: &mut BTreeMap<(String, String), ODRule>,
) -> Result<()> {
    let mut file_content = "".to_string();
    file.read_to_string(&mut file_content)?;
    let root: Element = file_content.parse()?;

    let ns = root.ns().unwrap_or_else(|| "".to_string());

    let mut frames = root
        .get_child("dataObjects", &ns)
        .ok_or_else(|| format_err!("Netex file doesn't contain a 'dataObjects' node"))?
        .children()
        .find(|frame| frame.name() == "CompositeFrame")
        .unwrap()
        .get_child("frames", &ns)
        .unwrap()
        .children();
    let resource_frame = frames
        .find(|frame| frame.name() == "ResourceFrame")
        .unwrap();
    let version = resource_frame
        .get_child("versions", &ns)
        .unwrap()
        .get_child("Version", &ns)
        .unwrap();
    let start_date = NaiveDate::parse_from_str(
        version.get_child("StartDate", &ns).unwrap().text().as_str(),
        DATE_TIME_FORMAT,
    )?;
    let end_date = NaiveDate::parse_from_str(
        version.get_child("EndDate", &ns).unwrap().text().as_str(),
        DATE_TIME_FORMAT,
    )?;
    let service_frame = frames.find(|frame| frame.name() == "ServiceFrame").unwrap();
    let fare_frames = frames.filter(|frame| frame.name() == "FareFrame");
    let mut frame_by_type = HashMap::new();
    for fare_frame in fare_frames {
        let fare_type =
            get_list_element_from_inner_list(fare_frame, "fareStructures", "FareStructure", &ns)?
                .get_child("KeyList", &ns)
                .unwrap()
                .children()
                .find(|key_value| {
                    key_value.get_child("Key", &ns).unwrap().text() == "FareStructureType"
                })
                .unwrap()
                .get_child("Value", &ns)
                .unwrap()
                .text();
        frame_by_type
            .entry(fare_type)
            .or_insert_with(|| vec![])
            .push(fare_frame);
    }
    let stop_point_ref_to_gtfs_stop_codes: HashMap<String, Vec<String>> = service_frame
        .get_child("scheduledStopPoints", &ns)
        .unwrap()
        .children()
        .map(|schedule_stop_point| {
            let stop_codes: Vec<String> = schedule_stop_point
                .get_child("projections", &ns)
                .unwrap()
                .children()
                .map(|proj| {
                    proj.get_child("ProjectedPointRef", &ns)
                        .unwrap()
                        .attr("ref")
                        .unwrap()
                        .replace("SYN:", "")
                })
                .collect();
            (
                schedule_stop_point.attr("id").unwrap().to_string(),
                stop_codes,
            )
        })
        .collect();
    if frame_by_type.get("UnitPrice").is_none() && frame_by_type.get("DistanceMatrix").is_some() {
        bail!("no UnitPrice FareFrame found for the DistanceMatrix FareFrame")
    }
    if let Some(unit_price_frames) = frame_by_type.get("UnitPrice") {
        if unit_price_frames.len() > 1 {
            bail!("unable to pick a reference UnitPrice FareFrame for the DistanceMatrix FareFrame")
        }
        let unit_price_frame = unit_price_frames[0];
        let fare_struct = get_list_element_from_inner_list(
            unit_price_frame,
            "fareStructures",
            "FareStructure",
            &ns,
        )?;
        let geo_interval = get_list_element_from_inner_list(
            fare_struct,
            "geographicalIntervals",
            "GeographicalInterval",
            &ns,
        )?;
        let rounding =
            get_value_for_key(unit_price_frame, "RoundingWrtCurrencyRule", &ns).unwrap_or(1.);
        let capping = get_value_for_key(unit_price_frame, "CappingWrtCurrencyRule", &ns);
        let boarding_fee = get_value_for_key(unit_price_frame, "EntranceRateWrtCurrency", &ns)?;
        let price = get_list_element_from_inner_list(
            geo_interval,
            "prices",
            "GeographicalIntervalPrice",
            &ns,
        )?;
        let base_price = price
            .get_child("Amount", &ns)
            .unwrap()
            .text()
            .parse::<f64>()?
            * price
                .get_child("Units", &ns)
                .unwrap()
                .text()
                .parse::<f64>()?;
        for distance_matrix_frame in frame_by_type.get("DistanceMatrix").unwrap_or(&vec![]) {
            for (id, distance_elt, start_stop_point, end_stop_point) in
                get_matrix_elts(distance_matrix_frame, &ns, "Distance")?
            {
                let distance = distance_elt.text().parse::<f64>()?;
                let mut price =
                    ((boarding_fee + base_price * distance) / rounding).round() * rounding;
                if let Ok(capping) = capping {
                    price = price.min(capping);
                }
                let ticket = Ticket::new(id.clone(), start_date, end_date, price);
                let od_rule = skip_fail!(get_od_rule(
                    &stop_point_ref_to_gtfs_stop_codes,
                    id,
                    start_stop_point,
                    end_stop_point,
                    stop_code_to_stop_area
                ));
                try_add_od_rule_and_ticket(od_rules, tickets, od_rule, ticket);
            }
        }
    }
    for direct_matrix_frame in frame_by_type.get("DirectPriceMatrix").unwrap_or(&vec![]) {
        let boarding_fee = get_value_for_key(direct_matrix_frame, "EntranceRateWrtCurrency", &ns)?;
        for (id, prices, start_stop_point, end_stop_point) in
            get_matrix_elts(direct_matrix_frame, &ns, "prices")?
        {
            let matrix_elt_price = prices.get_child("DistanceMatrixElementPrice", &ns).unwrap();
            let price = boarding_fee
                + matrix_elt_price
                    .get_child("Amount", &ns)
                    .unwrap()
                    .text()
                    .parse::<f64>()?
                    * matrix_elt_price
                        .get_child("Units", &ns)
                        .unwrap()
                        .text()
                        .parse::<f64>()?;
            let ticket = Ticket::new(id.clone(), start_date, end_date, price);
            let od_rule = skip_fail!(get_od_rule(
                &stop_point_ref_to_gtfs_stop_codes,
                id,
                start_stop_point,
                end_stop_point,
                &stop_code_to_stop_area
            ));
            try_add_od_rule_and_ticket(od_rules, tickets, od_rule, ticket);
        }
    }
    Ok(())
}

fn try_add_od_rule_and_ticket(
    od_rules: &mut BTreeMap<(String, String), ODRule>,
    tickets: &mut Vec<Ticket>,
    od_rule: ODRule,
    ticket: Ticket,
) {
    match od_rules.get(&(
        od_rule.origin_stop_area_id.clone(),
        od_rule.destination_stop_area_id.clone(),
    )) {
        Some(existing_rule) => warn!(
            "od_rule for {:?} / {:?} already exists, skipping the following one",
            existing_rule.origin_stop_area_id, existing_rule.destination_stop_area_id
        ),
        None => {
            od_rules.insert(
                (
                    od_rule.origin_stop_area_id.clone(),
                    od_rule.destination_stop_area_id.clone(),
                ),
                od_rule,
            );
            tickets.push(ticket);
        }
    }
}

fn get_matrix_elts<'a>(
    distance_matrix_frame: &'a Element,
    name_space: &str,
    tag_for_price_ref: &str,
) -> Result<Vec<(String, &'a Element, &'a str, &'a str)>> {
    let matrix_elts = get_list_element_from_inner_list(
        distance_matrix_frame,
        "fareStructures",
        "FareStructure",
        name_space,
    )?
    .get_child("distanceMatrixElements", name_space)
    .unwrap()
    .children()
    .map(|distance_matrix_elt| {
        (
            distance_matrix_elt.attr("id").unwrap().to_string(),
            distance_matrix_elt
                .get_child(tag_for_price_ref, name_space)
                .unwrap(),
            distance_matrix_elt
                .get_child("StartStopPointRef", name_space)
                .unwrap()
                .attr("ref")
                .unwrap(),
            distance_matrix_elt
                .get_child("EndStopPointRef", name_space)
                .unwrap()
                .attr("ref")
                .unwrap(),
        )
    })
    .collect();
    Ok(matrix_elts)
}

fn get_od_rule(
    stop_point_ref_to_gtfs_stop_codes: &HashMap<String, Vec<String>>,
    ticket_id: String,
    start_stop_point: &str,
    end_stop_point: &str,
    stop_code_to_stop_area: &HashMap<&str, &str>,
) -> Result<ODRule> {
    match (
        stop_point_ref_to_gtfs_stop_codes.get(start_stop_point),
        stop_point_ref_to_gtfs_stop_codes.get(end_stop_point),
    ) {
        (Some(start_gtfs_stop_codes), Some(end_gtfs_stop_codes)) => {
            let origin_stop_area_ids = start_gtfs_stop_codes
                .iter()
                .filter_map(|code| stop_code_to_stop_area.get::<str>(&code.to_string()))
                .collect::<HashSet<_>>();
            let destination_stop_area_ids = end_gtfs_stop_codes
                .iter()
                .filter_map(|code| stop_code_to_stop_area.get::<str>(&code.to_string()))
                .collect::<HashSet<_>>();
            match (origin_stop_area_ids.len(), destination_stop_area_ids.len()) {
                (1, 1) => Ok(
                    ODRule::new(format!("OD:{}",
                    ticket_id.clone()),
                    origin_stop_area_ids.iter().last().unwrap().to_string(),
                    destination_stop_area_ids
                        .iter()
                        .last()
                        .unwrap()
                        .to_string(),
                    ticket_id.clone())
                ),
                (nb_stop_areas, 1) =>
                    bail!(
                        "found {} stop area matches for origin {:?}",
                        nb_stop_areas, start_gtfs_stop_codes
                    ),
                (1, nb_stop_areas) =>
                    bail!(
                        "found {} stop area matches for destination {:?}",
                        nb_stop_areas, end_gtfs_stop_codes
                    ),
                (origin_nb_stop_areas, destination_nb_stop_areas) =>
                    bail!(
                        "found {} stop area matches for origin {:?} and {} matches for destination {:?}",
                        origin_nb_stop_areas,
                        start_gtfs_stop_codes,
                        destination_nb_stop_areas,
                        end_gtfs_stop_codes
                    ),
            }
        }
        (Some(_), None) =>
            bail!("StartStopPointRef {:?} has no corresponding scheduledStopPoints/projections/ProjectedPointRef", start_stop_point),
        (None, Some(_)) =>
            bail!("EndStopPointRef {:?} has no corresponding scheduledStopPoints/projections/ProjectedPointRef", end_stop_point),
        (None, None) => bail!("StartStopPointRef and EndStopPointRef {:?} have no corresponding scheduledStopPoints/projections/ProjectedPointRef", end_stop_point),
    }
}

/// Read Syntus fares data from provided `path` parameter which should be a link to a directory
/// containing at least one zip file containing some xml files in Netex format.
/// Fares will be calculated using the `stop_points` parameter collection as a referential for
/// mapping Netex stop points to NTFS ones using `object_codes.txt` data from `object_system`
/// `gtfs_stop_code`
pub fn read<P: AsRef<path::Path>>(
    path: P,
    stop_points: &CollectionWithId<StopPoint>,
) -> Result<(Collection<Ticket>, Collection<ODRule>)> {
    let files = fs::read_dir(&path)?
        .map(|f| {
            f?.file_name()
                .into_string()
                .map_err(|_| format_err!("syntus fares filename is not convertible into utf-8"))
        })
        .collect::<Result<BTreeSet<_>>>()?;
    if files.is_empty() {
        bail!("no files found into syntus fares directory");
    }
    let stop_code_to_stop_area: HashMap<&str, &str> = stop_points
        .values()
        .filter_map(|sp| {
            sp.codes
                .iter()
                .find(|(key, _)| key == "gtfs_stop_code")
                .map(|(_, code)| (code.as_ref(), sp.stop_area_id.as_ref()))
        })
        .collect();
    let mut tickets = vec![];
    let mut od_rules = BTreeMap::new();
    for filename in files {
        let file = fs::File::open(path.as_ref().join(filename))?;
        let mut zip = skip_fail!(zip::ZipArchive::new(file));
        for i in 0..zip.len() {
            let file = zip.by_index(i)?;
            match file.sanitized_name().extension() {
                Some(ext) if ext == "xml" => {
                    info!("reading fares file {:?}", file.name());
                    load_syntus_file(file, &stop_code_to_stop_area, &mut tickets, &mut od_rules)?;
                }
                _ => {
                    info!("skipping file in zip: {:?}", file.sanitized_name());
                }
            }
        }
    }
    let od_rules = od_rules.into_iter().map(|(_, od_rule)| od_rule).collect();
    Ok((Collection::new(tickets), Collection::new(od_rules)))
}
