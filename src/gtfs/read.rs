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
use objects::{self, CommentLinksT, Coord, KeysValues};
use std::collections::HashSet;
use utils::*;

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
impl Agency {
    fn alter_id_with_prefix(&mut self, prefix: &Option<String>) {
        let mut altered_id = self.id.clone().unwrap_or_else(default_agency_id);
        if let Some(ref id_prefix) = *prefix {
            altered_id = id_prefix.clone() + &altered_id;
        }
        self.id = Some(altered_id);
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
        let mut stop_codes: Vec<(String, String)> = vec![];
        if let Some(c) = stop.code {
            stop_codes.push(("gtfs_stop_code".to_string(), c));
        }
        objects::StopPoint {
            id: stop.id,
            name: stop.name,
            codes: stop_codes,
            object_properties: KeysValues::default(),
            comment_links: objects::CommentLinksT::default(),
            coord: Coord {
                lon: stop.lon,
                lat: stop.lat,
            },
            stop_area_id: stop.parent_station.unwrap(),
            timezone: stop.timezone,
            visible: true,
            geometry_id: None,
            equipment_id: None,
            fare_zone_id: None,
        }
    }
}

impl Stop {
    fn alter_id_with_prefix(&mut self, prefix: &Option<String>) {
        if let Some(ref id_prefix) = *prefix {
            self.id = id_prefix.clone() + &self.id;
        }
    }
    fn alter_parent_with_prefix(&mut self, prefix: &Option<String>) {
        if prefix.is_some() && self.parent_station.is_some() {
            self.parent_station =
                Some(prefix.clone().unwrap() + &self.parent_station.clone().unwrap());
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
        match *self {
            RouteType::Tramway_LightRail => "0".to_string(),
            RouteType::Metro => "1".to_string(),
            RouteType::Rail => "2".to_string(),
            RouteType::Bus => "3".to_string(),
            RouteType::Ferry => "4".to_string(),
            RouteType::CableCar => "5".to_string(),
            RouteType::Gondola_SuspendedCableCar => "6".to_string(),
            RouteType::Funicular => "7".to_string(),
            RouteType::Other(i) => i.to_string(),
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

#[derive(Serialize, Deserialize, Debug, Clone, Eq, Hash, PartialEq)]
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
        false
    }
}

#[derive(Derivative)]
#[derivative(Default(bound = ""))]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
enum DirectionType {
    #[derivative(Default)]
    #[serde(rename = "0")]
    Forward,
    #[serde(rename = "1")]
    Backward,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Trip {
    route_id: String,
    service_id: String,
    #[serde(rename = "trip_id")]
    id: String,
    #[serde(rename = "trip_headsign")]
    headsign: Option<String>,
    #[serde(rename = "trip_short_name")]
    short_name: Option<String>,
    #[serde(deserialize_with = "de_with_empty_default", rename = "direction_id")]
    direction: DirectionType,
    block_id: Option<String>,
    shape_id: Option<String>,
    #[serde(deserialize_with = "de_with_empty_default")]
    wheelchair_accessible: u8,
    #[serde(deserialize_with = "de_with_empty_default")]
    bikes_allowed: u8,
}

pub fn read_agency<P: AsRef<path::Path>>(
    path: P,
    id_prefix: &Option<String>,
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
        .map(|mut agency| {
            agency.alter_id_with_prefix(id_prefix);
            agency
        })
        .map(objects::Network::from)
        .collect();
    let networks = CollectionWithId::new(networks).unwrap();
    let companies = gtfs_agencies
        .into_iter()
        .map(|mut agency| {
            agency.alter_id_with_prefix(&id_prefix);
            agency
        })
        .map(objects::Company::from)
        .collect();
    let companies = CollectionWithId::new(companies).unwrap();
    (networks, companies)
}

pub fn read_stops<P: AsRef<path::Path>>(
    path: P,
    id_prefix: &Option<String>,
) -> (
    CollectionWithId<objects::StopArea>,
    CollectionWithId<objects::StopPoint>,
) {
    let path = path.as_ref().join("stops.txt");
    let mut rdr = csv::Reader::from_path(path).unwrap();
    let gtfs_stops: Vec<Stop> = rdr.deserialize().map(Result::unwrap).collect();

    let mut stop_areas = vec![];
    let mut stop_points = vec![];
    for mut stop in gtfs_stops {
        match stop.location_type {
            0 => {
                if stop.parent_station.is_none() {
                    let mut new_stop_area = stop.clone();
                    let mut prefix_generated = "Navitia:".to_string();
                    if let Some(ref prefix) = *id_prefix {
                        prefix_generated = prefix.to_string() + &prefix_generated;
                    }
                    new_stop_area.code = None;
                    new_stop_area.alter_id_with_prefix(&Some(prefix_generated));
                    stop.parent_station = Some(new_stop_area.id.clone());
                    stop_areas.push(objects::StopArea::from(new_stop_area));
                } else {
                    stop.alter_parent_with_prefix(id_prefix);
                }
                stop.alter_id_with_prefix(id_prefix);
                stop_points.push(objects::StopPoint::from(stop));
            }
            1 => {
                stop.alter_id_with_prefix(id_prefix);
                stop_areas.push(objects::StopArea::from(stop))
            }
            _ => (),
        }
    }
    let stoppoints = CollectionWithId::new(stop_points).unwrap();
    let stopareas = CollectionWithId::new(stop_areas).unwrap();
    (stopareas, stoppoints)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub contributor: objects::Contributor,
    pub dataset: objects::Dataset,
}

pub fn read_config(
    config: Config,
) -> (
    CollectionWithId<objects::Contributor>,
    CollectionWithId<objects::Dataset>,
) {
    let contributors = CollectionWithId::new(vec![config.contributor]).unwrap();
    let datasets = CollectionWithId::new(vec![config.dataset]).unwrap();
    (contributors, datasets)
}

#[derive(Eq, PartialEq)]
enum RouteReadType {
    RouteAsNtmRoute,
    RouteAsNtmLine,
}

fn define_route_file_read_mode(gtfs_routes: &[Route]) -> RouteReadType {
    let mut iter = gtfs_routes.iter();
    while let Some(r1) = iter.next() {
        for r2 in iter.clone() {
            if r1.is_same_line(r2) {
                return RouteReadType::RouteAsNtmRoute;
            }
        }
    }
    RouteReadType::RouteAsNtmLine
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
        name: get_commercial_mode_label(route_type),
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
        Bus | Other(_) => objects::PhysicalMode {
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

fn get_lines_from_gtfs(gtfs_routes: &[Route], read_mode: &RouteReadType) -> Vec<objects::Line> {
    let mut lines = vec![];

    let line_code = |r: &Route| {
        if r.short_name.is_empty() {
            None
        } else {
            Some(r.short_name.to_string())
        }
    };

    let line_agency = |r: &Route| match r.agency_id {
        Some(ref agency_id) => agency_id.to_string(),
        None => default_agency_id(),
    };

    match *read_mode {
        RouteReadType::RouteAsNtmLine => for r in gtfs_routes {
            let l = objects::Line {
                id: r.id.to_string(),
                code: line_code(r),
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                name: r.long_name.to_string(),
                forward_name: None,
                forward_direction: None,
                backward_name: None,
                backward_direction: None,
                color: r.color.clone(),
                text_color: r.text_color.clone(),
                sort_order: r.sort_order,
                network_id: line_agency(r),
                commercial_mode_id: r.route_type.to_gtfs_value(),
                geometry_id: None,
                opening_time: None,
                closing_time: None,
            };
            lines.push(l);
        },
        RouteReadType::RouteAsNtmRoute => {
            let mut routes: HashSet<&Route> = HashSet::new();
            let mut backward_routes: HashSet<&Route> = HashSet::new();
            let mut iter = gtfs_routes.iter();
            while let Some(r1) = iter.next() {
                if !backward_routes.contains(r1) {
                    routes.insert(r1);
                }
                for r2 in iter.clone() {
                    if r1.is_same_line(r2) {
                        backward_routes.insert(r2);
                        if r1.id > r2.id {
                            routes.remove(r1);
                            routes.insert(r2);
                        }
                    } else {
                        routes.insert(r2);
                    }
                }
            }

            for r in routes {
                let l = objects::Line {
                    id: r.id.to_string(),
                    code: line_code(r),
                    codes: KeysValues::default(),
                    object_properties: KeysValues::default(),
                    comment_links: CommentLinksT::default(),
                    name: r.long_name.to_string(),
                    forward_name: None,
                    forward_direction: None,
                    backward_name: None,
                    backward_direction: None,
                    color: r.color.clone(),
                    text_color: r.text_color.clone(),
                    sort_order: r.sort_order,
                    network_id: line_agency(r),
                    commercial_mode_id: r.route_type.to_gtfs_value(),
                    geometry_id: None,
                    opening_time: None,
                    closing_time: None,
                };
                lines.push(l);
            }
        }
    }
    lines
}

fn get_routes_from_gtfs(
    gtfs_routes: &[Route],
    gtfs_trips: &[Trip],
    read_mode: &RouteReadType,
) -> Vec<objects::Route> {
    let mut routes = vec![];

    match *read_mode {
        RouteReadType::RouteAsNtmLine => {
            let backward_routes: HashSet<_> = gtfs_trips
                .iter()
                .filter(|t| t.direction == DirectionType::Backward)
                .map(|t| t.route_id.clone())
                .collect();
            for r in gtfs_routes {
                let route = objects::Route {
                    id: r.id.to_string(),
                    name: r.long_name.to_string(),
                    direction_type: Some("forward".to_string()),
                    codes: KeysValues::default(),
                    object_properties: KeysValues::default(),
                    comment_links: CommentLinksT::default(),
                    line_id: r.id.to_string(),
                    geometry_id: None,
                    destination_id: None,
                };
                routes.push(route);
                if backward_routes.contains(&r.id) {
                    let route = objects::Route {
                        id: r.id.to_string() + "_R",
                        name: r.long_name.to_string(),
                        direction_type: Some("backward".to_string()),
                        codes: KeysValues::default(),
                        object_properties: KeysValues::default(),
                        comment_links: CommentLinksT::default(),
                        line_id: r.id.to_string(),
                        geometry_id: None,
                        destination_id: None,
                    };
                    routes.push(route);
                }
            }
        }
        RouteReadType::RouteAsNtmRoute => {
            let routes_directions: HashSet<(String, DirectionType)> = gtfs_trips
                .iter()
                .map(|t| (t.route_id.clone(), t.direction.clone()))
                .collect();

            let same_lines = get_same_lines(gtfs_routes);
            for r in gtfs_routes {
                let current_route_directions: HashSet<_> = routes_directions
                    .iter()
                    .filter(|h| h.0 == r.id)
                    .map(|h| h.1.clone())
                    .collect();
                if current_route_directions.len() > 1 {
                    let route = objects::Route {
                        id: r.id.to_string(),
                        name: r.long_name.to_string(),
                        direction_type: Some("forward".to_string()),
                        codes: KeysValues::default(),
                        object_properties: KeysValues::default(),
                        comment_links: CommentLinksT::default(),
                        line_id: get_line_id_of_route(r, &same_lines),
                        geometry_id: None,
                        destination_id: None,
                    };
                    routes.push(route);
                    let route = objects::Route {
                        id: r.id.to_string() + "_R",
                        name: r.long_name.to_string(),
                        direction_type: Some("backward".to_string()),
                        codes: KeysValues::default(),
                        object_properties: KeysValues::default(),
                        comment_links: CommentLinksT::default(),
                        line_id: get_line_id_of_route(r, &same_lines),
                        geometry_id: None,
                        destination_id: None,
                    };
                    routes.push(route);
                } else if current_route_directions.is_empty() {
                    panic!("RouteAsNtmRoute - Coudn't find trips for route_id {}", r.id);
                } else {
                    let direction = current_route_directions.iter().next().unwrap();
                    let direction_name = if *direction == DirectionType::Forward {
                        "forward"
                    } else {
                        "backward"
                    };
                    let route = objects::Route {
                        id: r.id.to_string(),
                        name: r.long_name.to_string(),
                        direction_type: Some(direction_name.to_string()),
                        codes: KeysValues::default(),
                        object_properties: KeysValues::default(),
                        comment_links: CommentLinksT::default(),
                        line_id: get_line_id_of_route(r, &same_lines),
                        geometry_id: None,
                        destination_id: None,
                    };
                    routes.push(route);
                }
            }
        }
    }
    routes
}

fn get_line_id_of_route(r: &Route, same_lines: &HashMap<String, String>) -> String {
    same_lines
        .get(&r.id)
        .map_or_else(|| r.id.clone(), |id| id.clone())
}

use std::collections::HashMap;
fn get_same_lines(gtfs_routes: &[Route]) -> HashMap<String, String> {
    let mut same_lines = HashMap::new();

    let mut iter = gtfs_routes.iter();

    while let Some(r1) = iter.next() {
        for r2 in iter.clone() {
            if r1.is_same_line(r2) {
                if r2.id > r1.id {
                    same_lines.insert(r2.id.clone(), r1.id.clone());
                } else {
                    same_lines.insert(r1.id.clone(), r2.id.clone());
                }
            }
        }
    }

    same_lines
}

pub fn read_routes<P: AsRef<path::Path>>(path: P, collections: &mut Collections) {
    let path = path.as_ref();
    let mut rdr = csv::Reader::from_path(path.join("routes.txt")).unwrap();
    let gtfs_routes: Vec<Route> = rdr.deserialize().map(Result::unwrap).collect();
    let (commercial_modes, physical_modes) = get_modes_from_gtfs(&gtfs_routes);
    collections.commercial_modes = CollectionWithId::new(commercial_modes).unwrap();
    collections.physical_modes = CollectionWithId::new(physical_modes).unwrap();

    let gtfs_reading_mode = define_route_file_read_mode(&gtfs_routes);
    let lines = get_lines_from_gtfs(&gtfs_routes, &gtfs_reading_mode);
    collections.lines = CollectionWithId::new(lines).unwrap();

    let mut rdr = csv::Reader::from_path(path.join("trips.txt")).unwrap();
    let gtfs_trips: Vec<Trip> = rdr.deserialize().map(Result::unwrap).collect();
    let routes = get_routes_from_gtfs(&gtfs_routes, &gtfs_trips, &gtfs_reading_mode);
    collections.routes = CollectionWithId::new(routes).unwrap();
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

        let (networks, companies) = super::read_agency(tmp_dir.path(), &None);
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

        let (networks, companies) = super::read_agency(tmp_dir.path(), &None);
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

        let (networks, companies) = super::read_agency(tmp_dir.path(), &None);
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
        super::read_agency(tmp_dir.path(), &None);
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

        let (stop_areas, stop_points) = super::read_stops(tmp_dir.path(), &None);
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

        let (stop_areas, stop_points) = super::read_stops(tmp_dir.path(), &None);
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

        let (stop_areas, _) = super::read_stops(tmp_dir.path(), &None);
        tmp_dir.close().expect("delete temp dir");
        //validate stop_area code
        assert_eq!(1, stop_areas.len());
        let stop_area = stop_areas.iter().next().unwrap().1;
        assert_eq!(0, stop_area.codes.len());
    }

    #[test]
    fn gtfs_routes_as_line() {
        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1,3,8F7A32,FFFFFF\n\
                              route_2,agency_2,,My line 2,2,7BC142,000000\n\
                              route_3,agency_3,3,My line 3,8,,\n\
                              route_4,agency_4,3,My line 3 for agency 3,8,,";

        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,,service_1,,\n\
             2,route_1,1,service_1,,\n\
             3,route_2,0,service_2,,\n\
             4,route_3,0,service_3,,\n\
             5,route_4,0,service_4,,";

        let tmp_dir = TempDir::new("navitia_model_tests").expect("create temp dir");
        let file_path = tmp_dir.path().join("routes.txt");
        let mut f = File::create(&file_path).unwrap();
        f.write_all(routes_content.as_bytes()).unwrap();

        let file_path = tmp_dir.path().join("trips.txt");
        let mut f = File::create(&file_path).unwrap();
        f.write_all(trips_content.as_bytes()).unwrap();

        let mut collections = Collections::default();
        super::read_routes(tmp_dir, &mut collections);
        assert_eq!(4, collections.lines.len());
        assert_eq!(2, collections.commercial_modes.len());

        let mut commercial_modes: Vec<String> = collections
            .commercial_modes
            .iter()
            .map(|(_, cm)| cm.name.clone())
            .collect();
        commercial_modes.sort();
        assert_eq!(commercial_modes, &["Bus", "Rail"]);

        let lines_commercial_modes_id: Vec<String> = collections
            .lines
            .iter()
            .map(|(_, l)| l.commercial_mode_id.clone())
            .collect();
        assert!(lines_commercial_modes_id.contains(&"2".to_string()));
        assert!(lines_commercial_modes_id.contains(&"3".to_string()));
        assert!(!lines_commercial_modes_id.contains(&"8".to_string()));

        assert_eq!(2, collections.physical_modes.len());
        let mut physical_modes: Vec<String> = collections
            .physical_modes
            .iter()
            .map(|(_, pm)| pm.name.clone())
            .collect();
        physical_modes.sort();
        assert_eq!(physical_modes, &["Bus", "Train"]);

        assert_eq!(5, collections.routes.len());

        let mut route_ids: Vec<String> = collections
            .routes
            .iter()
            .map(|(_, r)| r.id.clone())
            .collect();
        route_ids.sort();

        assert_eq!(
            route_ids,
            &["route_1", "route_1_R", "route_2", "route_3", "route_4"]
        );
    }

    #[test]
    fn gtfs_routes_as_route() {
        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1A,3,8F7A32,FFFFFF\n\
                              route_2,agency_1,1,My line 1B,3,8F7A32,FFFFFF\n\
                              route_3,agency_2,,My line 2,2,7BC142,000000\n\
                              route_4,agency_3,3,My line 3,8,,\n\
                              route_5,agency_2,1,My line 1 for agency 2,3,8F7A32,FFFFFF";

        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,0,service_1,,\n\
             2,route_1,1,service_1,,\n\
             3,route_2,0,service_2,,\n\
             4,route_3,0,service_2,,\n\
             5,route_3,1,service_2,,\n\
             6,route_4,1,service_2,,\n\
             7,route_5,0,service_3,,";

        let tmp_dir = TempDir::new("navitia_model_tests").expect("create temp dir");
        let file_path = tmp_dir.path().join("routes.txt");
        let mut f = File::create(&file_path).unwrap();
        f.write_all(routes_content.as_bytes()).unwrap();

        let file_path = tmp_dir.path().join("trips.txt");
        let mut f = File::create(&file_path).unwrap();
        f.write_all(trips_content.as_bytes()).unwrap();

        let mut collections = Collections::default();
        super::read_routes(tmp_dir, &mut collections);

        assert_eq!(4, collections.lines.len());
        assert_eq!(2, collections.commercial_modes.len());

        let mut lines_ids: Vec<String> = collections
            .lines
            .iter()
            .map(|(_, l)| l.id.clone())
            .collect();
        lines_ids.sort();

        assert_eq!(lines_ids, &["route_1", "route_3", "route_4", "route_5"]);

        assert_eq!(7, collections.routes.len());
        let mut route_ids: Vec<String> = collections
            .routes
            .iter()
            .map(|(_, r)| r.id.clone())
            .collect();
        route_ids.sort();

        assert_eq!(
            route_ids,
            &[
                "route_1",
                "route_1_R",
                "route_2",
                "route_3",
                "route_3_R",
                "route_4",
                "route_5"
            ]
        );

        // route_1 and route_2 belong to the same line
        use objects::Route;
        let route_2: &Route = collections
            .routes
            .iter()
            .map(|(_, r)| r)
            .filter(|r| r.id == "route_2".to_string())
            .next()
            .unwrap();

        assert_eq!(route_2.line_id, "route_1");
    }

    fn create_file_with_content(temp_dir: &TempDir, file_name: String, content: String) {
        let file_path = temp_dir.path().join(file_name);
        let mut f = File::create(&file_path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn prefix_on_all_pt_object_id() {
        let stops_content = "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\n\
                             sp:01,my stop point name,0.1,1.2,0,\n\
                             sp:02,my stop point name child,0.2,1.5,0,sp:01\n\
                             sa:03,my stop area name,0.3,2.2,1,"
            .to_string();
        let agency_content = "agency_id,agency_name,agency_url,agency_timezone,agency_lang\n\
                              584,TAM,http://whatever.canaltp.fr/,Europe/Paris,fr\n\
                              285,Phébus,http://plop.kisio.com/,Europe/London,en"
            .to_string();
        let tmp_dir = TempDir::new("navitia_model_tests").expect("create temp dir");
        create_file_with_content(&tmp_dir, "stops.txt".to_string(), stops_content);
        create_file_with_content(&tmp_dir, "agency.txt".to_string(), agency_content);
        let prefix = Some("my_prefix:".to_string());
        let (stop_areas, stop_points) = super::read_stops(tmp_dir.path(), &prefix);
        let (networks, companies) = super::read_agency(tmp_dir.path(), &prefix);
        tmp_dir.close().expect("delete temp dir");

        assert_eq!(2, stop_areas.len());
        assert_eq!(2, stop_points.len());
        assert_eq!(2, networks.len());
        assert_eq!(2, companies.len());

        let mut companies_ids: Vec<String> = companies
            .iter()
            .map(|(_, company)| company.id.clone())
            .collect();
        companies_ids.sort();
        assert_eq!(vec!["my_prefix:285", "my_prefix:584"], companies_ids);

        let mut networks_ids: Vec<String> = networks
            .iter()
            .map(|(_, network)| network.id.clone())
            .collect();
        networks_ids.sort();
        assert_eq!(vec!["my_prefix:285", "my_prefix:584"], networks_ids);

        let mut stop_areas_ids: Vec<String> = stop_areas
            .iter()
            .map(|(_, stop_area)| stop_area.id.clone())
            .collect();
        stop_areas_ids.sort();
        assert_eq!(
            vec!["my_prefix:Navitia:sp:01", "my_prefix:sa:03"],
            stop_areas_ids
        );

        let mut stop_points_ids: Vec<(String, String)> = stop_points
            .iter()
            .map(|(_, stop_point)| (stop_point.id.clone(), stop_point.stop_area_id.clone()))
            .collect();
        stop_points_ids.sort();
        assert_eq!(
            vec![
                (
                    "my_prefix:sp:01".to_string(),
                    "my_prefix:Navitia:sp:01".to_string(),
                ),
                ("my_prefix:sp:02".to_string(), "my_prefix:sp:01".to_string()),
            ],
            stop_points_ids
        );
    }
}
