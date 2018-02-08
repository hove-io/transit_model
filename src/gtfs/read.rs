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
use Collections;
use objects::{self, Coord, KeysValues};
use std::collections::HashSet;

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
            codes: KeysValues::default(),
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
            object_properties: KeysValues::default(),
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
            object_properties: KeysValues::default(),
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

#[derive(Serialize, Debug, Clone, Eq, PartialEq, Hash)]
enum RouteType {
    #[allow(non_camel_case_types)]
    Tramway_LightRail,
    Metro,
    Rail,
    Bus,
    Ferry,
    CableCar,
    #[allow(non_camel_case_types)]
    Gondola_SuspendedCableCar,
    Funicular,
    Other(u16),
}

impl RouteType {
    fn to_gtfs_value(&self) -> String {
        match self {
            &RouteType::Tramway_LightRail => "0".to_string(),
            &RouteType::Metro => "1".to_string(),
            &RouteType::Rail => "2".to_string(),
            &RouteType::Bus => "3".to_string(),
            &RouteType::Ferry => "4".to_string(),
            &RouteType::CableCar => "5".to_string(),
            &RouteType::Gondola_SuspendedCableCar => "6".to_string(),
            &RouteType::Funicular => "7".to_string(),
            &RouteType::Other(i) => i.to_string(),
        }
    }
}

impl<'de> ::serde::Deserialize<'de> for RouteType {
    fn deserialize<D>(deserializer: D) -> Result<RouteType, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        let mut i = u16::deserialize(deserializer)?;
        if i > 7 && i < 99 {
            i = 3;
            error!("illegal route_type: '{}', using '3' as fallback", i);
        }
        let i = match i {
            0 => RouteType::Tramway_LightRail,
            1 => RouteType::Metro,
            2 => RouteType::Rail,
            3 => RouteType::Bus,
            4 => RouteType::Ferry,
            5 => RouteType::CableCar,
            6 => RouteType::Gondola_SuspendedCableCar,
            7 => RouteType::Funicular,
            _ => RouteType::Other(i),
        };
        Ok(i)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Route {
    #[serde(rename = "route_id")]
    id: String,
    agency_id: Option<String>,
    #[serde(rename = "route_short_name")]
    short_name: String,
    #[serde(rename = "route_long_name")]
    long_name: String,
    #[serde(rename = "route_desc")]
    desc: Option<String>,
    route_type: RouteType,
    #[serde(rename = "route_url")]
    url: Option<String>,
    #[serde(rename = "route_color", default)]
    color: Option<objects::Rgb>,
    #[serde(rename = "route_text_color", default)]
    text_color: Option<objects::Rgb>,
    #[serde(rename = "route_sort_order")]
    sort_order: Option<u32>,
}

impl Route {
    fn is_same_line(&self, other: &Route) -> bool {
        if self.agency_id == other.agency_id {
            if self.short_name != "" {
                if self.short_name == other.short_name {
                    return true;
                }
            } else if self.long_name == other.long_name {
                return true;
            }
        }
        return false;
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

#[derive(Eq, PartialEq)]
enum RouteReadType {
    RouteAsNtmRoute,
    RouteAsNtmLine,
}

fn define_route_file_read_mode(gtfs_routes: &Vec<Route>) -> RouteReadType {
    let mut iter = gtfs_routes.iter();
    while let Some(r1) = iter.next() {
        for r2 in iter.clone() {
            if r1.is_same_line(r2) {
                return RouteReadType::RouteAsNtmRoute;
            }
        }
    }
    return RouteReadType::RouteAsNtmLine;
}

fn get_commercial_mode_label(route_type: &RouteType) -> String {
    use self::RouteType::*;
    let result = match *route_type {
        Tramway_LightRail => "Tram, Streetcar, Light rail",
        Metro => "Subway, Metro",
        Rail => "Rail",
        Bus => "Bus",
        Ferry => "Ferry",
        CableCar => "Cable car",
        Gondola_SuspendedCableCar => "Gondola, Suspended cable car",
        Funicular => "Funicular",
        Other(_) => "Unknown Mode",
    };
    result.to_string()
}

fn get_commercial_mode(route_type: &RouteType) -> objects::CommercialMode {
    objects::CommercialMode {
        id: route_type.to_gtfs_value(),
        name: get_commercial_mode_label(&route_type),
    }
}

fn get_physical_mode(route_type: &RouteType) -> objects::PhysicalMode {
    use self::RouteType::*;
    match *route_type {
        Tramway_LightRail => objects::PhysicalMode {
            id: "RailShuttle".to_string(),
            name: "Rail Shuttle".to_string(),
            co2_emission: None,
        },
        Metro => objects::PhysicalMode {
            id: "Metro".to_string(),
            name: "Metro".to_string(),
            co2_emission: None,
        },
        Rail => objects::PhysicalMode {
            id: "Train".to_string(),
            name: "Train".to_string(),
            co2_emission: None,
        },
        Bus => objects::PhysicalMode {
            id: "Bus".to_string(),
            name: "Bus".to_string(),
            co2_emission: None,
        },
        Ferry => objects::PhysicalMode {
            id: "Ferry".to_string(),
            name: "Ferry".to_string(),
            co2_emission: None,
        },
        CableCar | Gondola_SuspendedCableCar | Funicular => objects::PhysicalMode {
            id: "Funicular".to_string(),
            name: "Funicular".to_string(),
            co2_emission: None,
        },
        Other(_) => objects::PhysicalMode {
            id: "Bus".to_string(),
            name: "Bus".to_string(),
            co2_emission: None,
        },
    }
}

fn get_modes_from_gtfs(
    gtfs_routes: &[Route],
) -> (Vec<objects::CommercialMode>, Vec<objects::PhysicalMode>) {
    let gtfs_mode_types: HashSet<RouteType> =
        gtfs_routes.iter().map(|r| r.route_type.clone()).collect();

    let commercial_modes = gtfs_mode_types
        .iter()
        .map(|mt| get_commercial_mode(mt))
        .collect();
    let physical_modes = gtfs_mode_types
        .iter()
        .map(|mt| get_physical_mode(mt))
        .collect();
    (commercial_modes, physical_modes)
}

fn get_lines_from_gtfs(gtfs_routes: &Vec<Route>, read_mode: RouteReadType) -> Vec<objects::Line> {
    let mut lines = vec![];
    match read_mode {
        RouteReadType::RouteAsNtmLine => for r in gtfs_routes {
            let line_code = match r.short_name.is_empty() {
                true => None,
                false => Some(r.short_name.to_string()),
            };
            let line_agency = match r.agency_id {
                Some(ref agency_id) => agency_id.to_string(),
                None => default_agency_id(),
            };
            let l = objects::Line {
                id: r.id.to_string(),
                code: line_code.clone(),
                codes: vec![],
                object_properties: KeysValues::default(),
                comment_links: vec![],
                name: r.long_name.to_string(),
                forward_name: None,
                forward_direction: None,
                backward_name: None,
                backward_direction: None,
                color: r.color.clone(),
                text_color: r.text_color.clone(),
                sort_order: r.sort_order.clone(),
                network_id: line_agency,
                commercial_mode_id: r.route_type.to_gtfs_value(),
                geometry_id: None,
                opening_time: None,
                closing_time: None,
            };
            lines.push(l);
        },
        RouteReadType::RouteAsNtmRoute => {
            // TODO Build lines from GTFS routes as routes
            unimplemented!();
        }
    }
    lines
}

pub fn read_routes<P: AsRef<path::Path>>(path: P, collections: &mut Collections) {
    let path = path.as_ref().join("routes.txt");
    let mut rdr = csv::Reader::from_path(path).unwrap();
    let gtfs_routes: Vec<Route> = rdr.deserialize().map(Result::unwrap).collect();
    let (commercial_modes, physical_modes) = get_modes_from_gtfs(&gtfs_routes);
    let commercial_modes = CollectionWithId::new(commercial_modes);
    let physical_modes = CollectionWithId::new(physical_modes);
    collections.commercial_modes = commercial_modes;
    collections.physical_modes = physical_modes;

    let gtfs_reading_mode = define_route_file_read_mode(&gtfs_routes);
    let lines = get_lines_from_gtfs(&gtfs_routes, gtfs_reading_mode);
    collections.lines = CollectionWithId::new(lines);;
}

#[cfg(test)]
mod tests {
    extern crate tempdir;
    use self::tempdir::TempDir;
    use std::fs::File;
    use std::io::prelude::*;
    use Collections;

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
            "agency_id,agency_name,agency_url,agency_timezone,agency_lang,agency_phone,\
             agency_fare_url,agency_email\n\
             id_1,My agency,http://my-agency_url.com,Europe/London,EN,0123456789,\
             http://my-agency_fare_url.com,my-mail@example.com";
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

    #[test]
    fn gtfs_routes_as_line() {
        let stops_content = "route_id,agency_id,route_short_name,route_long_name,route_type\n\
                             route_1,agency_1,1,My line 1,3\n\
                             route_2,agency_2,2,My line 2,8\n\
                             route_3,agency_3,3,My line 3,2";
        let tmp_dir = TempDir::new("navitia_model_tests").expect("create temp dir");
        let file_path = tmp_dir.path().join("routes.txt");
        let mut f = File::create(&file_path).unwrap();
        f.write_all(stops_content.as_bytes()).unwrap();

        let mut collections = Collections::default();
        super::read_routes(tmp_dir.path(), &mut collections);
        assert_eq!(3, collections.lines.len());
        assert_eq!(2, collections.commercial_modes.len());

        let mut commercial_modes: Vec<String> = collections
            .commercial_modes
            .iter()
            .map(|(_, ref cm)| cm.name.to_string())
            .collect();
        commercial_modes.sort();
        assert_eq!(commercial_modes, &["Bus", "Rail"]);

        let lines_commercial_modes_id: Vec<String> = collections
            .lines
            .iter()
            .map(|(_, ref l)| l.commercial_mode_id.to_string())
            .collect();
        assert!(lines_commercial_modes_id.contains(&"2".to_string()));
        assert!(lines_commercial_modes_id.contains(&"3".to_string()));
        assert!(!lines_commercial_modes_id.contains(&"8".to_string()));
    }

}
