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

use std::path;
use csv;
use collection::CollectionWithId;
use objects::{self, CodesT, Coord};

fn default_agency_id() -> String {
    "default_agency_id".to_string()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Agency {
    #[serde(rename = "agency_id")]
    id: Option<String>,
    #[serde(rename = "agency_name")]
    name: String,
    #[serde(rename = "agency_url")]
    url: String,
    #[serde(rename = "agency_timezone")]
    timezone: Option<String>,
    #[serde(rename = "agency_lang")]
    lang: Option<String>,
    #[serde(rename = "agency_phone")]
    phone: Option<String>,
    #[serde(rename = "agency_email")]
    email: Option<String>,
}
impl From<Agency> for objects::Network {
    fn from(agency: Agency) -> objects::Network {
        objects::Network {
            id: agency.id.unwrap_or_else(default_agency_id),
            name: agency.name,
            codes: CodesT::default(),
            timezone: agency.timezone,
            url: Some(agency.url),
            lang: agency.lang,
            phone: agency.phone,
            address: None,
            sort_order: None,
        }
    }
}
impl From<Agency> for objects::Company {
    fn from(agency: Agency) -> objects::Company {
        objects::Company {
            id: agency.id.unwrap_or_else(default_agency_id),
            name: agency.name,
            address: None,
            url: Some(agency.url),
            mail: agency.email,
            phone: agency.phone,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Stop {
    #[serde(rename = "stop_id")]
    id: String,
    #[serde(rename = "stop_code")]
    code: Option<String>,
    #[serde(rename = "stop_name")]
    name: String,
    #[serde(default, rename = "stop_desc")]
    desc: String,
    #[serde(rename = "stop_lon")]
    lon: f64,
    #[serde(rename = "stop_lat")]
    lat: f64,
    #[serde(rename = "zone_id")]
    fare_zone_id: Option<String>,
    #[serde(rename = "stop_url")]
    url: Option<String>,
    #[serde(default)]
    location_type: i32,
    parent_station: Option<String>,
    #[serde(rename = "stop_timezone")]
    timezone: Option<String>,
    #[serde(default)]
    wheelchair_boarding: Option<String>,
}
impl From<Stop> for objects::StopArea {
    fn from(stop: Stop) -> objects::StopArea {
        let mut stop_codes: Vec<(String, String)> = vec![];
        if let Some(c) = stop.code {
            stop_codes.push(("gtfs_stop_code".to_string(), c));
        }
        objects::StopArea {
            id: stop.id,
            name: stop.name,
            codes: stop_codes,
            comment_links: objects::CommentLinksT::default(),
            coord: Coord {
                lon: stop.lon,
                lat: stop.lat,
            },
            timezone: stop.timezone,
            visible: true,
            geometry_id: None,
            equipment_id: None,
        }
    }
}
impl From<Stop> for objects::StopPoint {
    fn from(stop: Stop) -> objects::StopPoint {
        let id = stop.id;
        let stop_area_id = stop.parent_station
            .unwrap_or_else(|| format!("Navitia:{}", id));
        let mut stop_codes: Vec<(String, String)> = vec![];
        if let Some(c) = stop.code {
            stop_codes.push(("gtfs_stop_code".to_string(), c));
        }
        objects::StopPoint {
            id: id,
            name: stop.name,
            codes: stop_codes,
            comment_links: objects::CommentLinksT::default(),
            coord: Coord {
                lon: stop.lon,
                lat: stop.lat,
            },
            stop_area_id: stop_area_id,
            timezone: stop.timezone,
            visible: true,
            geometry_id: None,
            equipment_id: None,
            fare_zone_id: None,
        }
    }
}

pub fn read_agency<P: AsRef<path::Path>>(
    path: P,
) -> (
    CollectionWithId<objects::Network>,
    CollectionWithId<objects::Company>,
) {
    let path = path.as_ref().join("agency.txt");
    let mut rdr = csv::Reader::from_path(path).unwrap();
    let gtfs_agencies: Vec<Agency> = rdr.deserialize().map(Result::unwrap).collect();
    let networks = gtfs_agencies
        .iter()
        .cloned()
        .map(objects::Network::from)
        .collect();
    let networks = CollectionWithId::new(networks);
    let companies = gtfs_agencies
        .into_iter()
        .map(objects::Company::from)
        .collect();
    let companies = CollectionWithId::new(companies);
    (networks, companies)
}

pub fn read_stops<P: AsRef<path::Path>>(
    path: P,
) -> (
    CollectionWithId<objects::StopArea>,
    CollectionWithId<objects::StopPoint>,
) {
    let path = path.as_ref().join("stops.txt");
    let mut rdr = csv::Reader::from_path(path).unwrap();
    let gtfs_stops: Vec<Stop> = rdr.deserialize().map(Result::unwrap).collect();

    let mut stop_areas = vec![];
    let mut stop_points = vec![];
    for stop in gtfs_stops {
        match stop.location_type {
            0 => {
                if stop.parent_station.is_none() {
                    let mut new_stop_area = stop.clone();
                    new_stop_area.id = format!("Navitia:{}", new_stop_area.id);
                    new_stop_area.code = None;
                    stop_areas.push(objects::StopArea::from(new_stop_area));
                }
                stop_points.push(objects::StopPoint::from(stop));
            }
            1 => stop_areas.push(objects::StopArea::from(stop)),
            _ => (),
        }
    }

    let stoppoints = CollectionWithId::new(stop_points);
    let stopareas = CollectionWithId::new(stop_areas);
    (stopareas, stoppoints)
}

#[cfg(test)]
mod tests {
    extern crate tempdir;
    use self::tempdir::TempDir;
    use std::fs::File;
    use std::io::prelude::*;

    #[test]
    fn load_minimal_agency() {
        let agency_content = "agency_name,agency_url,agency_timezone\n\
                              My agency,http://my-agency_url.com,Europe/London";
        let tmp_dir = TempDir::new("navitia_model_tests").expect("create temp dir");
        let file_path = tmp_dir.path().join("agency.txt");
        let mut f = File::create(&file_path).unwrap();
        f.write_all(agency_content.as_bytes()).unwrap();

        let (networks, companies) = super::read_agency(tmp_dir.path());
        tmp_dir.close().expect("delete temp dir");
        assert_eq!(1, networks.len());
        let agency = networks.iter().next().unwrap().1;
        assert_eq!("default_agency_id", agency.id);
        assert_eq!(1, companies.len());
    }

    #[test]
    fn load_standard_agency() {
        let agency_content = "agency_id,agency_name,agency_url,agency_timezone\n\
                              id_1,My agency,http://my-agency_url.com,Europe/London";
        let tmp_dir = TempDir::new("navitia_model_tests").expect("create temp dir");
        let file_path = tmp_dir.path().join("agency.txt");
        let mut f = File::create(&file_path).unwrap();
        f.write_all(agency_content.as_bytes()).unwrap();

        let (networks, companies) = super::read_agency(tmp_dir.path());
        tmp_dir.close().expect("delete temp dir");
        assert_eq!(1, networks.len());
        assert_eq!(1, companies.len());
    }

    #[test]
    fn load_complete_agency() {
        let agency_content =
            "agency_id,agency_name,agency_url,agency_timezone,agency_lang,agency_phone,agency_fare_url,agency_email\n\
             id_1,My agency,http://my-agency_url.com,Europe/London,EN,0123456789,http://my-agency_fare_url.com,my-mail@example.com";
        let tmp_dir = TempDir::new("navitia_model_tests").expect("create temp dir");
        let file_path = tmp_dir.path().join("agency.txt");
        let mut f = File::create(&file_path).unwrap();
        f.write_all(agency_content.as_bytes()).unwrap();

        let (networks, companies) = super::read_agency(tmp_dir.path());
        tmp_dir.close().expect("delete temp dir");
        assert_eq!(1, networks.len());
        let network = networks.iter().next().unwrap().1;
        assert_eq!("id_1", network.id);
        assert_eq!(1, companies.len());
    }

    #[test]
    #[should_panic]
    fn load_2_agencies_with_no_id() {
        let agency_content = "agency_name,agency_url,agency_timezone\n\
                              My agency 1,http://my-agency_url.com,Europe/London\
                              My agency 2,http://my-agency_url.com,Europe/London";
        let tmp_dir = TempDir::new("navitia_model_tests").expect("create temp dir");
        let file_path = tmp_dir.path().join("agency.txt");
        let mut f = File::create(&file_path).unwrap();
        f.write_all(agency_content.as_bytes()).unwrap();
        super::read_agency(tmp_dir.path());
        tmp_dir.close().expect("delete temp dir");
    }

    #[test]
    fn load_one_stop_point() {
        let stops_content = "stop_id,stop_name,stop_lat,stop_lon\n\
                             id1,my stop name,0.1,1.2";
        let tmp_dir = TempDir::new("navitia_model_tests").expect("create temp dir");
        let file_path = tmp_dir.path().join("stops.txt");
        let mut f = File::create(&file_path).unwrap();
        f.write_all(stops_content.as_bytes()).unwrap();

        let (stop_areas, stop_points) = super::read_stops(tmp_dir.path());
        tmp_dir.close().expect("delete temp dir");
        assert_eq!(1, stop_areas.len());
        assert_eq!(1, stop_points.len());
        let stop_area = stop_areas.iter().next().unwrap().1;
        assert_eq!("Navitia:id1", stop_area.id);

        assert_eq!(1, stop_points.len());
        let stop_point = stop_points.iter().next().unwrap().1;
        assert_eq!("Navitia:id1", stop_point.stop_area_id);
    }

    #[test]
    fn stop_code_on_stops() {
        let stops_content =
            "stop_id,stop_code,stop_name,stop_lat,stop_lon,location_type,parent_station\n\
             stoppoint_id,1234,my stop name,0.1,1.2,0,stop_area_id\n\
             stoparea_id,5678,stop area name,0.1,1.2,1,";
        let tmp_dir = TempDir::new("navitia_model_tests").expect("create temp dir");
        let file_path = tmp_dir.path().join("stops.txt");
        let mut f = File::create(&file_path).unwrap();
        f.write_all(stops_content.as_bytes()).unwrap();

        let (stop_areas, stop_points) = super::read_stops(tmp_dir.path());
        tmp_dir.close().expect("delete temp dir");
        //validate stop_point code
        assert_eq!(1, stop_points.len());
        let stop_point = stop_points.iter().next().unwrap().1;
        assert_eq!(1, stop_point.codes.len());
        let code = stop_point.codes.iter().next().unwrap();
        assert_eq!(code.0, "gtfs_stop_code");
        assert_eq!(code.1, "1234");

        //validate stop_area code
        assert_eq!(1, stop_areas.len());
        let stop_area = stop_areas.iter().next().unwrap().1;
        assert_eq!(1, stop_area.codes.len());
        let code = stop_area.codes.iter().next().unwrap();
        assert_eq!(code.0, "gtfs_stop_code");
        assert_eq!(code.1, "5678");
    }

    #[test]
    fn no_stop_code_on_autogenerated_stoparea() {
        let stops_content =
            "stop_id,stop_code,stop_name,stop_lat,stop_lon,location_type,parent_station\n\
             stoppoint_id,1234,my stop name,0.1,1.2,0,";
        let tmp_dir = TempDir::new("navitia_model_tests").expect("create temp dir");
        let file_path = tmp_dir.path().join("stops.txt");
        let mut f = File::create(&file_path).unwrap();
        f.write_all(stops_content.as_bytes()).unwrap();

        let (stop_areas, _) = super::read_stops(tmp_dir.path());
        tmp_dir.close().expect("delete temp dir");
        //validate stop_area code
        assert_eq!(1, stop_areas.len());
        let stop_area = stop_areas.iter().next().unwrap().1;
        assert_eq!(0, stop_area.codes.len());
    }

}
