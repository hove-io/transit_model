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
    objects::{Coord, StopArea as NTMStopArea, StopPoint as NTMStopPoint},
    Result, WGS84Coordinates,
};
use failure::{format_err, ResultExt};
use geo_types::Point;
use log::info;
#[cfg(feature = "proj")]
use proj::Proj;
use serde::Deserialize;
use std::{collections::HashMap, fs::File, io::Read, path::Path};
use zip::ZipArchive;

#[derive(Debug, Deserialize)]
pub struct Stop {
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
}

impl WGS84Coordinates for Stop {
    fn coord(&self) -> Result<Coord> {
        Ok(Coord {
            lon: self.longitude,
            lat: self.latitude,
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct StopInArea {
    #[serde(rename = "AtcoCode")]
    atco_code: String,
    #[serde(rename = "StopAreaCode")]
    stop_area_code: String,
}

#[derive(Debug, Deserialize)]
pub struct StopArea {
    #[serde(rename = "StopAreaCode")]
    stop_area_code: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Easting")]
    easting: f64,
    #[serde(rename = "Northing")]
    northing: f64,
}

const EPSG_27700: &str = "EPSG:27700";
// FIXME: String 'EPSG:4326' is failing at runtime (string below is equivalent but works)
const EPSG_4326: &str = "+proj=longlat +datum=WGS84 +no_defs"; // See https://epsg.io/4326
#[cfg(feature = "proj")]
impl WGS84Coordinates for StopArea {
    fn coord(&self) -> Result<Coord> {
        let epsg_27700_point = Point::new(self.easting, self.northing);
        let converter = Proj::new_known_crs(&EPSG_27700, &EPSG_4326, None).ok_or_else(|| {
            format_err!(
                "Proj cannot build a converter from '{}' to '{}'",
                EPSG_27700,
                EPSG_4326
            )
        })?;
        converter.convert(epsg_27700_point).map(Coord::from)
    }
}

fn read_stop_areas<R>(reader: R) -> Result<CollectionWithId<NTMStopArea>>
where
    R: Read,
{
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b',')
        .trim(csv::Trim::All)
        .from_reader(reader);
    let mut stop_areas = CollectionWithId::default();
    for record in reader.deserialize() {
        let stop_area: StopArea =
            record.with_context(|_| "Error parsing the CSV record into a StopArea")?;
        let ntm_stop_area = NTMStopArea {
            id: stop_area.stop_area_code.clone(),
            name: stop_area.name.clone(),
            coord: stop_area.coord()?,
            ..Default::default()
        };
        stop_areas.push(ntm_stop_area)?;
    }
    Ok(stop_areas)
}

fn read_stops_in_area<R>(reader: R) -> Result<HashMap<String, String>>
where
    R: Read,
{
    csv::ReaderBuilder::new()
        .delimiter(b',')
        .trim(csv::Trim::All)
        .from_reader(reader)
        .deserialize()
        .map(|record: csv::Result<StopInArea>| {
            record.with_context(|_| "Error parsing the CSV record into a StopInArea")
        })
        .map(|record| {
            let stop_in_area = record?;
            let key_value = (
                stop_in_area.atco_code.clone(),
                stop_in_area.stop_area_code.clone(),
            );
            Ok(key_value)
        })
        .collect()
}

fn read_stops<R>(
    reader: R,
    _stops_in_area: &HashMap<String, String>,
) -> Result<CollectionWithId<NTMStopPoint>>
where
    R: Read,
{
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b',')
        .trim(csv::Trim::All)
        .from_reader(reader);
    for record in reader.deserialize() {
        let _stop: Stop = record.with_context(|_| "Error parsing the CSV record into a Stop")?;
    }
    unimplemented!()
}

fn validate_stops(
    _stop_areas: &CollectionWithId<NTMStopArea>,
    _stop_points: &CollectionWithId<NTMStopPoint>,
) -> Result<()> {
    unimplemented!()
}

const STOP_AREAS_FILENAME: &str = "StopAreas.csv";
const STOPS_IN_AREA_FILENAME: &str = "StopsInArea.csv";
const STOPS_FILENAME: &str = "Stops.csv";
pub fn read_naptan<P>(naptan_path: P, collections: &mut Collections) -> Result<()>
where
    P: AsRef<Path>,
{
    let zip_file = File::open(naptan_path)?;
    let mut zip_archive = ZipArchive::new(zip_file)?;
    info!("reading NaPTAN file for {}", STOP_AREAS_FILENAME);
    let stop_areas = read_stop_areas(zip_archive.by_name(STOP_AREAS_FILENAME)?)?;
    info!("reading NaPTAN file for {}", STOPS_IN_AREA_FILENAME);
    let stops_in_area = read_stops_in_area(zip_archive.by_name(STOPS_IN_AREA_FILENAME)?)?;
    info!("reading NaPTAN file for {}", STOPS_FILENAME);
    let stop_points = read_stops(zip_archive.by_name(STOPS_FILENAME)?, &stops_in_area)?;
    validate_stops(&stop_areas, &stop_points)?;
    collections.stop_areas.try_merge(stop_areas)?;
    collections.stop_points.try_merge(stop_points)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    mod stop {
        use super::*;
        use approx::assert_relative_eq;

        #[test]
        fn get_wgs84() {
            let stop = Stop {
                atco_code: String::from("4TC0_C0D3"),
                name: String::from("This is a beautiful stop"),
                longitude: -2.5876178397,
                latitude: 51.4558382170,
                indicator: String::from("Stop B"),
            };
            let wgs84 = stop.coord().unwrap();
            assert_relative_eq!(wgs84.lon, -2.5876178397);
            assert_relative_eq!(wgs84.lat, 51.4558382170);
        }
    }

    mod stop_area {
        use super::*;
        use approx::assert_relative_eq;

        // This test uses the Bristol Bus Station coordinates
        // in EPSG:27700 format (Easting, Northing)
        // https://epsg.io/map#srs=27700&x=358929&y=173523&z=20
        // and in the corresponding EPSG:4326 format (aka WGS84)
        // https://epsg.io/map#srs=4326&x=-2.592540&y=51.459184&z=20
        #[test]
        fn convert_wgs84() {
            let stop_area = StopArea {
                stop_area_code: String::from("010G0001"),
                name: String::from("Bristol Bus Station"),
                easting: 358929f64,
                northing: 173523f64,
            };
            let wgs84 = stop_area.coord().unwrap();
            assert_relative_eq!(wgs84.lon, -2.5925401721561157);
            assert_relative_eq!(wgs84.lat, 51.45918359900175);
        }
    }

    mod read_stop_areas {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn parsing_works() {
            let csv_content = r#""StopAreaCode","Name","Easting","Northing"
"010G0001","Bristol Bus Station",358929,173523
"010G0002","Temple Meads",359657,172418"#;
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
            let csv_content = r#""Name","NameLang","AdministrativeAreaCode","StopAreaType","GridType","Easting","Northing"
"Temple Meads",359657,172418"#;
            read_stop_areas(csv_content.as_bytes()).unwrap();
        }

        #[test]
        #[should_panic]
        fn empty_stop_area_code() {
            let csv_content = r#""StopAreaCode","Name","NameLang","AdministrativeAreaCode","StopAreaType","GridType","Easting","Northing"
,"Bristol Bus Station",358929,173523
,"Temple Meads",359657,172418"#;
            read_stop_areas(csv_content.as_bytes()).unwrap();
        }

        #[test]
        #[should_panic]
        fn missing_coords() {
            let csv_content = r#""StopAreaCode","Name"
"010G0001","Bristol Bus Station"
"010G0002","Temple Meads""#;
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
            let stops_in_area = read_stops_in_area(csv_content.as_bytes()).unwrap();
            assert_eq!(stops_in_area.len(), 2);
            let stop_area_code = stops_in_area.get("01000053220").unwrap();
            assert_eq!(stop_area_code, "010G0005");
            let stop_area_code = stops_in_area.get("0100BDMNSTR0").unwrap();
            assert_eq!(stop_area_code, "910GBDMNSTR");
        }

        #[test]
        #[should_panic]
        fn no_atco_code() {
            let csv_content = r#""StopAreaCode"
"010G0005"
"910GBDMNSTR""#;
            read_stops_in_area(csv_content.as_bytes()).unwrap();
        }
    }
}
