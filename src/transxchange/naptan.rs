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

//! Module to help parsing and reading NaPTAN files
//! https://en.wikipedia.org/wiki/NaPTAN

use crate::{
    collection::CollectionWithId,
    model::Collections,
    objects::{Coord, StopArea, StopPoint},
    read_utils::{self, FileHandler},
    Result,
};
use failure::{format_err, ResultExt};
use geo_types::Point;
use log::{info, warn};
#[cfg(feature = "proj")]
use proj::Proj;
use serde::Deserialize;
use std::{collections::HashMap, fs::File, io::Read, path::Path};

#[derive(Debug, Deserialize)]
pub struct NaPTANStop {
    #[serde(rename = "ATCOCode")]
    atco_code: String,
    #[serde(rename = "CommonName")]
    name: String,
    #[serde(rename = "Longitude")]
    longitude: f64,
    #[serde(rename = "Latitude")]
    latitude: f64,
    #[serde(rename = "Indicator")]
    indicator: String,
    #[serde(rename = "Status")]
    status: String,
}

#[derive(Debug, Deserialize)]
pub struct NaPTANStopInArea {
    #[serde(rename = "AtcoCode")]
    atco_code: String,
    #[serde(rename = "StopAreaCode")]
    stop_area_code: String,
}

#[derive(Debug, Deserialize)]
pub struct NaPTANStopArea {
    #[serde(rename = "StopAreaCode")]
    stop_area_code: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Easting")]
    easting: f64,
    #[serde(rename = "Northing")]
    northing: f64,
    #[serde(rename = "Status")]
    status: String,
}

fn read_stop_areas<R>(reader: R) -> Result<CollectionWithId<StopArea>>
where
    R: Read,
{
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b',')
        .trim(csv::Trim::All)
        .from_reader(reader);
    let mut stop_areas = CollectionWithId::default();
    let from = "EPSG:27700";
    // FIXME: String 'EPSG:4326' is failing at runtime (string below is equivalent but works)
    let to = "+proj=longlat +datum=WGS84 +no_defs"; // See https://epsg.io/4326
    let converter = Proj::new_known_crs(from, to, None)
        .ok_or_else(|| format_err!("Proj cannot build a converter from '{}' to '{}'", from, to))?;
    for record in reader.deserialize() {
        let stop_area: NaPTANStopArea =
            record.with_context(|_| "Error parsing the CSV record into a StopArea")?;
        if stop_area.status != "act" {
            continue;
        }
        let point = Point::new(stop_area.easting, stop_area.northing);
        if let Ok(coord) = converter.convert(point).map(Coord::from) {
            stop_areas.push(StopArea {
                id: stop_area.stop_area_code.clone(),
                name: stop_area.name.clone(),
                coord,
                ..Default::default()
            })?;
        } else {
            warn!(
                "Failed to convert point ({}, {}) from {} into WGS84",
                point.x(),
                point.y(),
                from,
            );
        }
    }
    Ok(stop_areas)
}

fn read_stops_in_area<R>(
    reader: R,
    stop_areas: &CollectionWithId<StopArea>,
) -> Result<HashMap<String, String>>
where
    R: Read,
{
    fn is_valid_stop_area(
        stop_in_area: &NaPTANStopInArea,
        stop_areas: &CollectionWithId<StopArea>,
    ) -> bool {
        stop_areas
            .get_idx(&stop_in_area.stop_area_code)
            .map(|_| true)
            .unwrap_or_else(|| {
                warn!("Failed to find Stop Area '{}'", stop_in_area.stop_area_code);
                false
            })
    }
    csv::ReaderBuilder::new()
        .delimiter(b',')
        .trim(csv::Trim::All)
        .from_reader(reader)
        .deserialize()
        .map(|record: csv::Result<NaPTANStopInArea>| {
            record.with_context(|_| "Error parsing the CSV record into a StopInArea")
        })
        .filter(|record| {
            match record {
                Ok(stop_in_area) => is_valid_stop_area(stop_in_area, stop_areas),
                // We want to keep record that are Err(_) so the `.collect()` below report errors
                Err(_) => true,
            }
        })
        .map(|record| {
            let stop_in_area = record?;
            Ok((
                stop_in_area.atco_code.clone(),
                stop_in_area.stop_area_code.clone(),
            ))
        })
        .collect()
}

// Create stop points and create missing stop areas for stop points without
// a corresponding stop area in NaPTAN dataset
fn read_stops<R>(
    reader: R,
    stops_in_area: &HashMap<String, String>,
) -> Result<(CollectionWithId<StopPoint>, CollectionWithId<StopArea>)>
where
    R: Read,
{
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b',')
        .trim(csv::Trim::All)
        .from_reader(reader);
    let mut stop_points = CollectionWithId::default();
    let mut stop_areas = CollectionWithId::default();
    for record in reader.deserialize() {
        let stop: NaPTANStop =
            record.with_context(|_| "Error parsing the CSV record into a Stop")?;
        if stop.status != "act" {
            continue;
        }
        let coord = Coord {
            lon: stop.longitude,
            lat: stop.latitude,
        };
        let stop_area_id = match stops_in_area.get(&stop.atco_code).cloned() {
            Some(stop_area_id) => stop_area_id,
            None => {
                // If the stop point don't have a corresponding stop area
                // create the stop area based on stop point information
                let id = format!("Navitia:{}", stop.atco_code);
                info!(
                    "Creating StopArea {} for corresponding StopPoint {}",
                    id, stop.atco_code
                );
                stop_areas.push(StopArea {
                    id: id.clone(),
                    name: stop.name.clone(),
                    visible: true,
                    coord,
                    ..Default::default()
                })?;
                id
            }
        };
        let stop_point = StopPoint {
            id: stop.atco_code.clone(),
            name: stop.name.clone(),
            coord,
            stop_area_id,
            platform_code: Some(stop.indicator.clone()),
            ..Default::default()
        };
        stop_points.push(stop_point)?;
    }
    Ok((stop_points, stop_areas))
}

const STOP_AREAS_FILENAME: &str = "StopAreas.csv";
const STOPS_IN_AREA_FILENAME: &str = "StopsInArea.csv";
const STOPS_FILENAME: &str = "Stops.csv";

fn read<H>(file_handler: &mut H, collections: &mut Collections) -> Result<()>
where
    for<'a> &'a mut H: read_utils::FileHandler,
{
    info!("reading NaPTAN file for {}", STOP_AREAS_FILENAME);
    let (reader, _) = file_handler.get_file(STOP_AREAS_FILENAME)?;
    let stop_areas = read_stop_areas(reader)?;
    info!("reading NaPTAN file for {}", STOPS_IN_AREA_FILENAME);
    let (reader, _) = file_handler.get_file(STOPS_IN_AREA_FILENAME)?;
    let stops_in_area = read_stops_in_area(reader, &stop_areas)?;
    info!("reading NaPTAN file for {}", STOPS_FILENAME);
    let (reader, _) = file_handler.get_file(STOPS_FILENAME)?;
    let (stop_points, additional_stop_areas) = read_stops(reader, &stops_in_area)?;

    collections.stop_areas.try_merge(stop_areas)?;
    collections.stop_points.try_merge(stop_points)?;
    collections.stop_areas.try_merge(additional_stop_areas)?;
    Ok(())
}

pub fn read_from_zip<P>(path: P, collections: &mut Collections) -> Result<()>
where
    P: AsRef<Path>,
{
    let reader = File::open(path.as_ref())?;
    let mut file_handle = read_utils::ZipHandler::new(reader, path)?;
    read(&mut file_handle, collections)
}

pub fn read_from_path<P>(path: P, collections: &mut Collections) -> Result<()>
where
    P: AsRef<Path>,
{
    let mut file_handle = read_utils::PathFileHandler::new(path.as_ref().to_path_buf());
    read(&mut file_handle, collections)
}

#[cfg(test)]
mod tests {
    use super::*;

    mod read_stop_areas {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn parsing_works() {
            let csv_content = r#""StopAreaCode","Name","Easting","Northing","Status"
"010G0001","Bristol Bus Station",358929,173523,"act"
"010G0002","Temple Meads",359657,172418,"act"
"010G0003","Old Market Street",359670,173162,"del""#;
            let stop_areas = read_stop_areas(csv_content.as_bytes()).unwrap();
            assert_eq!(stop_areas.len(), 2);
            let stop_area = stop_areas.get("010G0001").unwrap();
            assert_eq!(stop_area.name, "Bristol Bus Station");
            let stop_area = stop_areas.get("010G0002").unwrap();
            assert_eq!(stop_area.name, "Temple Meads");
        }

        #[test]
        #[should_panic]
        fn no_stop_area_code() {
            let csv_content = r#""Name","Easting","Northing","Status"
"Temple Meads",359657,172418,"act""#;
            read_stop_areas(csv_content.as_bytes()).unwrap();
        }

        #[test]
        #[should_panic]
        fn empty_stop_area_code() {
            let csv_content = r#""StopAreaCode","Name","Easting","Northing","Status"
,"Bristol Bus Station",358929,173523,"act"
,"Temple Meads",359657,172418,"act""#;
            read_stop_areas(csv_content.as_bytes()).unwrap();
        }

        #[test]
        #[should_panic]
        fn duplicate_id() {
            let csv_content = r#""StopAreaCode","Name","Easting","Northing","Status"
"010G0001","Bristol Bus Station",358929,173523,"act"
"010G0001","Bristol Bus Station",358929,173523,"act"
"010G0002","Temple Meads",359657,172418,"act""#;
            read_stop_areas(csv_content.as_bytes()).unwrap();
        }
    }

    mod read_stop_in_area {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn parsing_works() {
            let csv_content = r#""StopAreaCode","AtcoCode"
"010G0005","01000053220"
"910GBDMNSTR","0100BDMNSTR0""#;
            let mut stop_areas = CollectionWithId::default();
            stop_areas
                .push(StopArea {
                    id: String::from("010G0005"),
                    ..Default::default()
                })
                .unwrap();
            stop_areas
                .push(StopArea {
                    id: String::from("910GBDMNSTR"),
                    ..Default::default()
                })
                .unwrap();
            let stops_in_area = read_stops_in_area(csv_content.as_bytes(), &stop_areas).unwrap();
            assert_eq!(stops_in_area.len(), 2);
            let stop_area_code = stops_in_area.get("01000053220").unwrap();
            assert_eq!(stop_area_code, "010G0005");
            let stop_area_code = stops_in_area.get("0100BDMNSTR0").unwrap();
            assert_eq!(stop_area_code, "910GBDMNSTR");
        }

        #[test]
        fn missing_stop_area() {
            let csv_content = r#""StopAreaCode","AtcoCode"
"010G0005","01000053220"
"910GBDMNSTR","0100BDMNSTR0""#;
            let mut stop_areas = CollectionWithId::default();
            stop_areas
                .push(StopArea {
                    id: String::from("010G0005"),
                    ..Default::default()
                })
                .unwrap();
            let stops_in_area = read_stops_in_area(csv_content.as_bytes(), &stop_areas).unwrap();
            assert_eq!(stops_in_area.len(), 1);
            let stop_area_code = stops_in_area.get("01000053220").unwrap();
            assert_eq!(stop_area_code, "010G0005");
        }

        #[test]
        #[should_panic]
        fn no_atco_code() {
            let csv_content = r#""StopAreaCode"
"010G0005"
"910GBDMNSTR""#;
            read_stops_in_area(csv_content.as_bytes(), &CollectionWithId::default()).unwrap();
        }
    }

    mod read_stops {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn parsing_works() {
            let csv_content =
                r#""ATCOCode","CommonName","Indicator","Longitude","Latitude","Status"
"0100053316","Broad Walk Shops","Stop B",-2.5876178397,51.4558382170,"act"
"0100053264","Alberton Road","NE-bound",-2.5407019785,51.4889912765,"act"
"0100053308","Counterslip","SW-bound",-2.5876736730,51.4557030625,"del""#;
            let mut stop_in_area = HashMap::new();
            stop_in_area.insert(String::from("0100053316"), String::from("stop-area-1"));
            stop_in_area.insert(String::from("0100053308"), String::from("stop-area-3"));
            let (stop_points, stop_areas) =
                read_stops(csv_content.as_bytes(), &stop_in_area).unwrap();

            assert_eq!(stop_points.len(), 2);
            let stop_point = stop_points.get("0100053316").unwrap();
            assert_eq!(stop_point.name, "Broad Walk Shops");
            let stop_point = stop_points.get("0100053264").unwrap();
            assert_eq!(stop_point.name, "Alberton Road");

            assert_eq!(stop_areas.len(), 1);
            let stop_area = stop_areas.get("Navitia:0100053264").unwrap();
            assert_eq!(stop_area.name, "Alberton Road");
        }

        #[test]
        #[should_panic]
        fn no_atco_code() {
            let csv_content = r#""CommonName","Indicator","Longitude","Latitude","Status"
"Broad Walk Shops","Stop B",-2.5876178397,51.4558382170,"act"
"Alberton Road","NE-bound",-2.5407019785,51.4889912765,"act""#;
            let stop_in_area = HashMap::new();
            read_stops(csv_content.as_bytes(), &stop_in_area).unwrap();
        }

        #[test]
        #[should_panic]
        fn duplicate_id() {
            let csv_content =
                r#""ATCOCode","CommonName","Indicator","Longitude","Latitude","Status"
"0100053316","Broad Walk Shops","Stop B",-2.5876178397,51.4558382170,"act"
"0100053316","Broad Walk Shops","Stop B",-2.5876178397,51.4558382170,"act"
"0100053264","Alberton Road","NE-bound",-2.5407019785,51.4889912765,"act""#;
            let mut stop_in_area = HashMap::new();
            stop_in_area.insert(String::from("0100053316"), String::from("stop-area-1"));
            stop_in_area.insert(String::from("0100053264"), String::from("stop-area-2"));
            read_stops(csv_content.as_bytes(), &stop_in_area).unwrap();
        }
    }
}
