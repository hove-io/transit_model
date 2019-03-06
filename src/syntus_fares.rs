// Copyright 2017-2018 Kisio Digital and/or its affiliates.
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
use crate::collection::CollectionWithId;
use crate::objects::StopPoint;
use crate::Result;
use crate::objects::{Ticket, ODRule};
use chrono::NaiveDate;
use failure::bail;
use failure::format_err;
use log::{info, warn};
use minidom::Element;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path;
use zip;
const DATE_TIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S.0Z";

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
    stop_code_to_stop_area: &HashMap<String, String>,
    tickets: &mut Vec<Ticket>,
    od_rules: &mut Vec<ODRule>,
) -> Result<()> {
    let mut file_content = "".to_string();
    file.read_to_string(&mut file_content)?;
    let root: Element = file_content.parse()?;

    let ns = root.ns().unwrap_or_else(|| "".to_string());

    let mut frames = root
        .get_child("dataObjects", &ns)
        .ok_or_else(|| format_err!("Netex file does't contain a 'dataObjects' node"))?
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
            .parse::<f64>()
            .unwrap()
            * price
                .get_child("Units", &ns)
                .unwrap()
                .text()
                .parse::<f64>()?;
        for distance_matrix_frame in frame_by_type.get("DistanceMatrix").unwrap_or(&vec![]) {
            for distance_matrix_elt in get_list_element_from_inner_list(
                distance_matrix_frame,
                "fareStructures",
                "FareStructure",
                &ns,
            )?
            .get_child("distanceMatrixElements", &ns)
            .unwrap()
            .children()
            {
                let distance = distance_matrix_elt
                    .get_child("Distance", &ns)
                    .unwrap()
                    .text()
                    .parse::<f64>()?;
                let start_stop_point = distance_matrix_elt
                    .get_child("StartStopPointRef", &ns)
                    .unwrap()
                    .attr("ref")
                    .unwrap();
                let end_stop_point = distance_matrix_elt
                    .get_child("EndStopPointRef", &ns)
                    .unwrap()
                    .attr("ref")
                    .unwrap();
                let id = distance_matrix_elt.attr("id").unwrap().to_string();
                let mut price =
                    ((boarding_fee + base_price * distance) / rounding).round() * rounding;
                if let Ok(capping) = capping {
                    price = price.min(capping);
                }
                let ticket = Ticket::new(id.clone(), start_date, end_date, price);
                let od_rule = match (
                    stop_point_ref_to_gtfs_stop_codes.get(start_stop_point),
                    stop_point_ref_to_gtfs_stop_codes.get(end_stop_point),
                ) {
                    (Some(start_gtfs_stop_codes), Some(end_gtfs_stop_codes)) => {
                        let origin_stop_area_ids = start_gtfs_stop_codes
                            .iter()
                            .filter_map(|code| stop_code_to_stop_area.get(code))
                            .collect::<HashSet<_>>();
                        let destination_stop_area_ids = end_gtfs_stop_codes
                            .iter()
                            .filter_map(|code| stop_code_to_stop_area.get(code))
                            .collect::<HashSet<_>>();
                        match (origin_stop_area_ids.len(), destination_stop_area_ids.len()) {
                            (1, 1) => ODRule::new(format!("OD:{}", id.clone()), origin_stop_area_ids
                                    .iter()
                                    .last()
                                    .unwrap()
                                    .to_string(),
                                destination_stop_area_ids
                                    .iter()
                                    .last()
                                    .unwrap()
                                    .to_string(),id.clone()),
                            (nb_stop_areas, 1) => {
                                warn!(
                                    "found {} stop area matches for origin {:?}",
                                    nb_stop_areas, start_gtfs_stop_codes
                                );
                                continue;
                            }
                            (1, nb_stop_areas) => {
                                warn!(
                                    "found {} stop area matches for destination {:?}",
                                    nb_stop_areas, end_gtfs_stop_codes
                                );
                                continue;
                            }
                            (origin_nb_stop_areas, destination_nb_stop_areas) => {
                                warn!(
                                    "found {} stop area matches for origin {:?} and {} matches for destination {:?}",
                                    origin_nb_stop_areas,
                                    start_gtfs_stop_codes,
                                    destination_nb_stop_areas,
                                    end_gtfs_stop_codes
                                );
                                continue;
                            }
                        }
                    }
                    (Some(_), None) => {
                        warn!("StartStopPointRef {:?} has no corresponding scheduledStopPoints/projections/ProjectedPointRef", start_stop_point);
                        continue;
                    }
                    (None, Some(_)) => {
                        warn!("EndStopPointRef {:?} has no corresponding scheduledStopPoints/projections/ProjectedPointRef", end_stop_point);
                        continue;
                    }
                    (None, None) => {
                        warn!("StartStopPointRef and EndStopPointRef {:?} have no corresponding scheduledStopPoints/projections/ProjectedPointRef", end_stop_point);
                        continue;
                    }
                };
                od_rules.push(od_rule);
                tickets.push(ticket);
            }
        }
    }
    Ok(())
}

/// Read Syntus fares data from provided `path` parameter which should be a link to a directory
/// containing at least one zip file containing some xml files in Netex format.
/// Fares will be calculated using the `stop_points` parameter collection as a referential for
/// mapping Netex stop points to NTFS ones using `object_codes.txt` data from `object_system`
/// `gtfs_stop_code`
pub fn read<P: AsRef<path::Path>>(
    path: P,
    stop_points: &CollectionWithId<StopPoint>,
) -> Result<(
    CollectionWithId<Ticket>,
    CollectionWithId<ODRule>,
)> {
    let files: Vec<String> = fs::read_dir(&path)
        .unwrap()
        .map(|f| f.unwrap().file_name().into_string().unwrap())
        .collect();
    if files.is_empty() {
        bail!("no files found into syntus fares directory");
    }
    let stop_code_to_stop_area: HashMap<String, String> = stop_points
        .values()
        .filter_map(|sp| {
            sp.codes
                .iter()
                .find(|(key, _)| key == "gtfs_stop_code")
                .map(|(_, code)| (code.clone(), sp.stop_area_id.clone()))
        })
        .collect();
    let mut tickets = vec![];
    let mut od_rules = vec![];
    for filename in files {
        let file = fs::File::open(path.as_ref().join(filename))?;
        let mut zip = zip::ZipArchive::new(file)?;
        for i in 0..zip.len() {
            let file = zip.by_index(i)?;
            match file.sanitized_name().extension() {
                Some(ext) if ext == "xml" => {
                    info!("reading fares file {:?}", file.name());
                    load_syntus_file(file, &stop_code_to_stop_area, &mut tickets, &mut od_rules)?;
                }
                _ => {
                    info!("skipping file in ZIP : {:?}", file.sanitized_name());
                }
            }
        }
    }
    Ok((CollectionWithId::new(tickets)?, CollectionWithId::new(od_rules)?))
}
