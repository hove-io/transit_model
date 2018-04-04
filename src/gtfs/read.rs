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

use Result;
use collection::{CollectionWithId, Id};
use csv;
use failure::ResultExt;
use model::Collections;
use objects::{self, Availability, CommentLinksT, Contributor, Coord, KeysValues, Time,
              TransportType};
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::path;
use std::result::Result as StdResult;
use utils::*;
extern crate serde_json;

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

#[derive(Derivative)]
#[derivative(Default(bound = ""))]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum StopLocationType {
    #[derivative(Default)]
    #[serde(rename = "0")]
    StopArea,
    #[serde(rename = "1")]
    StopPoint,
    #[serde(rename = "2")]
    StopEntrace,
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
    location_type: StopLocationType,
    parent_station: Option<String>,
    #[serde(rename = "stop_timezone")]
    timezone: Option<String>,
    #[serde(default)]
    wheelchair_boarding: Option<String>,
}

pub struct EquipmentList {
    equipments: Vec<objects::Equipment>,
}

impl Default for EquipmentList {
    fn default() -> Self {
        EquipmentList { equipments: vec![] }
    }
}

impl EquipmentList {
    pub fn get_equipments(self) -> Vec<objects::Equipment> {
        self.equipments
    }
    pub fn push(&mut self, mut equipment: objects::Equipment) -> String {
        equipment.id = self.equipments.len().to_string();
        let equipment_id = equipment.id.clone();
        self.equipments.push(equipment);
        equipment_id
    }
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
    fn deserialize<D>(deserializer: D) -> StdResult<RouteType, D::Error>
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

impl Id<Route> for Route {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Route {
    fn get_line_key(&self) -> (Option<String>, String) {
        let name = if self.short_name != "" {
            self.short_name.clone()
        } else {
            self.long_name.clone()
        };

        (self.agency_id.clone(), name)
    }

    fn get_id_by_direction(&self, d: &DirectionType) -> String {
        let id = self.id.clone();
        match *d {
            DirectionType::Forward => id,
            DirectionType::Backward => id + "_R",
        }
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
    #[serde(deserialize_with = "de_with_empty_default", default)]
    wheelchair_accessible: u8,
    #[serde(deserialize_with = "de_with_empty_default", default)]
    bikes_allowed: u8,
}

impl Trip {
    fn to_ntfs_vehicle_journey(
        &self,
        routes: &CollectionWithId<Route>,
        dataset: &objects::Dataset,
        trip_property_id: Option<String>,
    ) -> objects::VehicleJourney {
        let route = routes.get(&self.route_id).unwrap();
        let physical_mode = get_physical_mode(&route.route_type);

        objects::VehicleJourney {
            id: self.id.clone(),
            codes: KeysValues::default(),
            object_properties: KeysValues::default(),
            comment_links: CommentLinksT::default(),
            route_id: route.get_id_by_direction(&self.direction),
            physical_mode_id: physical_mode.id,
            dataset_id: dataset.id.clone(),
            service_id: self.service_id.clone(),
            headsign: self.short_name.clone().or_else(|| self.headsign.clone()),
            block_id: self.block_id.clone(),
            company_id: route.agency_id.clone().unwrap_or_else(default_agency_id),
            trip_property_id,
            geometry_id: self.shape_id.clone(),
            stop_times: vec![],
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct StopTime {
    trip_id: String,
    arrival_time: Time,
    departure_time: Time,
    stop_id: String,
    stop_sequence: u32,
    #[serde(deserialize_with = "de_with_empty_default", default)]
    pickup_type: u8,
    #[serde(deserialize_with = "de_with_empty_default", default)]
    dropoff_type: u8,
}

pub fn manage_stop_times<P: AsRef<path::Path>>(
    collections: &mut Collections,
    path: P,
) -> Result<()> {
    info!("Reading stop_times.txt");
    let path = path.as_ref().join("stop_times.txt");
    let mut rdr = csv::Reader::from_path(&path).with_context(ctx_from_path!(path))?;
    for stop_time in rdr.deserialize() {
        let stop_time: StopTime = stop_time.with_context(ctx_from_path!(path))?;
        let stop_point_idx = collections
            .stop_points
            .get_idx(&stop_time.stop_id)
            .ok_or_else(|| {
                format_err!(
                    "Problem reading {:?}: stop_id={:?} not found",
                    path,
                    stop_time.stop_id
                )
            })?;
        let vj_idx = collections
            .vehicle_journeys
            .get_idx(&stop_time.trip_id)
            .ok_or_else(|| {
                format_err!(
                    "Problem reading {:?}: trip_id={:?} not found",
                    path,
                    stop_time.stop_id
                )
            })?;
        collections
            .vehicle_journeys
            .index_mut(vj_idx)
            .stop_times
            .push(objects::StopTime {
                stop_point_idx,
                sequence: stop_time.stop_sequence,
                arrival_time: stop_time.arrival_time,
                departure_time: stop_time.departure_time,
                boarding_duration: 0,
                alighting_duration: 0,
                pickup_type: stop_time.pickup_type,
                dropoff_type: stop_time.dropoff_type,
                datetime_estimated: false,
                local_zone_id: None,
            });
    }
    let mut vehicle_journeys = collections.vehicle_journeys.take();
    for vj in &mut vehicle_journeys {
        vj.stop_times.sort_unstable_by_key(|st| st.sequence);
    }
    collections.vehicle_journeys = CollectionWithId::new(vehicle_journeys)?;
    Ok(())
}

pub fn read_agency<P: AsRef<path::Path>>(
    path: P,
) -> Result<(
    CollectionWithId<objects::Network>,
    CollectionWithId<objects::Company>,
)> {
    info!("Reading agency.txt");
    let path = path.as_ref().join("agency.txt");
    let mut rdr = csv::Reader::from_path(&path).with_context(ctx_from_path!(path))?;
    let gtfs_agencies: Vec<Agency> = rdr.deserialize()
        .collect::<StdResult<_, _>>()
        .with_context(ctx_from_path!(path))?;
    let networks = gtfs_agencies
        .iter()
        .cloned()
        .map(objects::Network::from)
        .collect();
    let networks = CollectionWithId::new(networks)?;
    let companies = gtfs_agencies
        .into_iter()
        .map(objects::Company::from)
        .collect();
    let companies = CollectionWithId::new(companies)?;
    Ok((networks, companies))
}

fn manage_comment_from_stop(
    comments: &mut CollectionWithId<objects::Comment>,
    stop: &Stop,
) -> CommentLinksT {
    let mut comment_links: CommentLinksT = CommentLinksT::default();
    if !stop.desc.is_empty() {
        let comment_id = "stop:".to_string() + &stop.id;
        let comment = objects::Comment {
            id: comment_id,
            comment_type: objects::CommentType::Information,
            label: None,
            name: stop.desc.to_string(),
            url: None,
        };
        let idx = comments.push(comment).unwrap();
        comment_links.push(idx);
    }
    comment_links
}

fn get_equipment_id_and_populate_equipments(
    equipments: &mut EquipmentList,
    stop: &Stop,
) -> Option<String> {
    stop.wheelchair_boarding
        .as_ref()
        .and_then(|availability| match availability.as_str() {
            "1" => Some(objects::Availability::Available),
            "2" => Some(objects::Availability::NotAvailable),
            _ => None,
        })
        .map(|availlability| {
            equipments.push(objects::Equipment {
                id: "".to_string(),
                wheelchair_boarding: availlability,
                sheltered: objects::Availability::InformationNotAvailable,
                elevator: objects::Availability::InformationNotAvailable,
                escalator: objects::Availability::InformationNotAvailable,
                bike_accepted: objects::Availability::InformationNotAvailable,
                bike_depot: objects::Availability::InformationNotAvailable,
                visual_announcement: objects::Availability::InformationNotAvailable,
                audible_announcement: objects::Availability::InformationNotAvailable,
                appropriate_escort: objects::Availability::InformationNotAvailable,
                appropriate_signage: objects::Availability::InformationNotAvailable,
            })
        })
}

pub fn read_stops<P: AsRef<path::Path>>(
    path: P,
    comments: &mut CollectionWithId<objects::Comment>,
    equipments: &mut EquipmentList,
) -> Result<(
    CollectionWithId<objects::StopArea>,
    CollectionWithId<objects::StopPoint>,
)> {
    info!("Reading stops.txt");
    let path = path.as_ref().join("stops.txt");
    let mut rdr = csv::Reader::from_path(&path).with_context(ctx_from_path!(path))?;
    let gtfs_stops: Vec<Stop> = rdr.deserialize()
        .collect::<StdResult<_, _>>()
        .with_context(ctx_from_path!(path))?;

    let mut stop_areas = vec![];
    let mut stop_points = vec![];
    for mut stop in gtfs_stops {
        let comment_links = manage_comment_from_stop(comments, &stop);
        let equipment_id = get_equipment_id_and_populate_equipments(equipments, &stop);
        match stop.location_type {
            StopLocationType::StopArea => {
                if stop.parent_station.is_none() {
                    let mut new_stop_area = stop.clone();
                    new_stop_area.id = format!("Navitia:{}", new_stop_area.id);
                    new_stop_area.code = None;
                    stop.parent_station = Some(new_stop_area.id.clone());
                    stop_areas.push(objects::StopArea::from(new_stop_area));
                }
                let mut stop_point = objects::StopPoint::from(stop);
                stop_point.comment_links = comment_links;
                stop_point.equipment_id = equipment_id;
                stop_points.push(stop_point);
            }
            StopLocationType::StopPoint => {
                let mut stop_area = objects::StopArea::from(stop);
                stop_area.comment_links = comment_links;
                stop_area.equipment_id = equipment_id;
                stop_areas.push(stop_area);
            }
            StopLocationType::StopEntrace => warn!(
                "stop location type {:?} not handled for the moment, skipping",
                StopLocationType::StopEntrace
            ),
        }
    }
    let stoppoints = CollectionWithId::new(stop_points)?;
    let stopareas = CollectionWithId::new(stop_areas)?;
    Ok((stopareas, stoppoints))
}

#[derive(Deserialize, Debug)]
struct Dataset {
    dataset_id: String,
}

#[derive(Deserialize, Debug)]
struct Config {
    contributor: objects::Contributor,
    dataset: Dataset,
}

pub fn read_config<P: AsRef<path::Path>>(
    config_path: Option<P>,
) -> Result<(
    CollectionWithId<objects::Contributor>,
    CollectionWithId<objects::Dataset>,
)> {
    let contributor;
    let dataset;
    if let Some(config_path) = config_path {
        let json_config_file = File::open(config_path)?;
        let config: Config = serde_json::from_reader(json_config_file)?;
        info!("Reading dataset and contributor from config: {:?}", config);

        contributor = config.contributor;

        use chrono::{Duration, Utc};
        let duration = Duration::days(15);
        let today = Utc::today();
        let start_date = today - duration;
        let end_date = today + duration;
        dataset = objects::Dataset {
            id: config.dataset.dataset_id,
            contributor_id: contributor.id.clone(),
            start_date: start_date.naive_utc(),
            end_date: end_date.naive_utc(),
            dataset_type: None,
            extrapolation: false,
            desc: None,
            system: None,
        };
    } else {
        contributor = Contributor::default();
        dataset = objects::Dataset::default();
    }

    let contributors = CollectionWithId::new(vec![contributor])?;
    let datasets = CollectionWithId::new(vec![dataset])?;
    Ok((contributors, datasets))
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
    gtfs_routes: &CollectionWithId<Route>,
) -> (Vec<objects::CommercialMode>, Vec<objects::PhysicalMode>) {
    let gtfs_mode_types: HashSet<RouteType> =
        gtfs_routes.values().map(|r| r.route_type.clone()).collect();

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

fn get_route_with_smallest_name<'a>(routes: &'a [&Route]) -> &'a Route {
    routes.iter().min_by_key(|r| &r.id).unwrap()
}

type MapLineRoutes<'a> = HashMap<(Option<String>, String), Vec<&'a Route>>;
fn map_line_routes(gtfs_routes: &CollectionWithId<Route>) -> MapLineRoutes {
    let mut map = HashMap::new();
    for r in gtfs_routes.values() {
        map.entry(r.get_line_key())
            .or_insert_with(|| vec![])
            .push(r);
    }
    map
}

fn make_lines(gtfs_trips: &[Trip], map_line_routes: &MapLineRoutes) -> Vec<objects::Line> {
    let mut lines = vec![];

    let line_code = |r: &Route| {
        if r.short_name.is_empty() {
            None
        } else {
            Some(r.short_name.to_string())
        }
    };

    let line_agency = |r: &Route| {
        r.agency_id
            .as_ref()
            .map(|id| id.to_string())
            .unwrap_or_else(default_agency_id)
    };

    for routes in map_line_routes.values() {
        let r = get_route_with_smallest_name(routes);

        if gtfs_trips.iter().any(|t| t.route_id == r.id) {
            lines.push(objects::Line {
                id: r.id.clone(),
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
            });
        }
    }

    lines
}

fn make_routes(gtfs_trips: &[Trip], map_line_routes: &MapLineRoutes) -> Vec<objects::Route> {
    let mut routes = vec![];

    let get_direction_name = |d: &DirectionType| match *d {
        DirectionType::Forward => "forward".to_string(),
        DirectionType::Backward => "backward".to_string(),
    };

    for rs in map_line_routes.values() {
        let sr = get_route_with_smallest_name(rs);
        for r in rs {
            let mut route_directions: HashSet<&DirectionType> = HashSet::new();
            for t in gtfs_trips.iter().filter(|t| t.route_id == r.id) {
                route_directions.insert(&t.direction);
            }
            if route_directions.is_empty() {
                warn!("Coudn't find trips for route_id {}", r.id);
            }

            for d in route_directions {
                routes.push(objects::Route {
                    id: r.get_id_by_direction(d),
                    name: r.long_name.clone(),
                    direction_type: Some(get_direction_name(d)),
                    codes: KeysValues::default(),
                    object_properties: KeysValues::default(),
                    comment_links: CommentLinksT::default(),
                    line_id: sr.id.clone(),
                    geometry_id: None,
                    destination_id: None,
                });
            }
        }
    }
    routes
}

fn get_availability(i: u8) -> Result<Availability> {
    let availability = match i {
        0 => Availability::InformationNotAvailable,
        1 => Availability::Available,
        2 => Availability::NotAvailable,
        i => bail!("invalid trip property {}", i,),
    };

    Ok(availability)
}

fn make_ntfs_vehicle_journeys(
    gtfs_trips: &[Trip],
    routes: &CollectionWithId<Route>,
    datasets: &CollectionWithId<objects::Dataset>,
) -> Result<(Vec<objects::VehicleJourney>, Vec<objects::TripProperty>)> {
    // there always is one dataset from config or a default one
    let (_, dataset) = datasets.iter().next().unwrap();
    let mut vehicle_journeys: Vec<objects::VehicleJourney> = vec![];
    let mut trip_properties: Vec<objects::TripProperty> = vec![];
    let mut map_tps_trips: HashMap<(u8, u8), Vec<&Trip>> = HashMap::new();
    let mut id_incr: u8 = 1;

    for t in gtfs_trips {
        map_tps_trips
            .entry((t.wheelchair_accessible, t.bikes_allowed))
            .or_insert_with(|| vec![])
            .push(t);
    }

    // If there is no accessibility information in the trips
    if map_tps_trips.len() == 1 && map_tps_trips.keys().next() == Some(&(0, 0)) {
        for trips in map_tps_trips.values() {
            for t in trips {
                vehicle_journeys.push(t.to_ntfs_vehicle_journey(routes, dataset, None));
            }
        }
    } else {
        for (&(weelchair_id, bike_id), trips) in &map_tps_trips {
            trip_properties.push(objects::TripProperty {
                id: id_incr.to_string(),
                wheelchair_accessible: get_availability(weelchair_id)?,
                bike_accepted: get_availability(bike_id)?,
                air_conditioned: Availability::InformationNotAvailable,
                visual_announcement: Availability::InformationNotAvailable,
                audible_announcement: Availability::InformationNotAvailable,
                appropriate_escort: Availability::InformationNotAvailable,
                appropriate_signage: Availability::InformationNotAvailable,
                school_vehicle_type: TransportType::Regular,
            });

            for t in trips {
                vehicle_journeys.push(t.to_ntfs_vehicle_journey(
                    routes,
                    dataset,
                    Some(id_incr.to_string()),
                ));
            }
            id_incr += 1;
        }
    }

    Ok((vehicle_journeys, trip_properties))
}

pub fn read_routes<P: AsRef<path::Path>>(path: P, collections: &mut Collections) -> Result<()> {
    info!("Reading routes.txt");
    let path = path.as_ref();
    let routes_path = path.join("routes.txt");
    let mut rdr = csv::Reader::from_path(&routes_path).with_context(ctx_from_path!(routes_path))?;
    let gtfs_routes: Vec<Route> = rdr.deserialize()
        .collect::<StdResult<_, _>>()
        .with_context(ctx_from_path!(routes_path))?;

    let gtfs_routes_collection = CollectionWithId::new(gtfs_routes)?;

    let (commercial_modes, physical_modes) = get_modes_from_gtfs(&gtfs_routes_collection);
    collections.commercial_modes = CollectionWithId::new(commercial_modes)?;
    collections.physical_modes = CollectionWithId::new(physical_modes)?;

    let trips_path = path.join("trips.txt");
    let mut rdr = csv::Reader::from_path(&trips_path).with_context(ctx_from_path!(trips_path))?;
    let gtfs_trips: Vec<Trip> = rdr.deserialize()
        .collect::<StdResult<_, _>>()
        .with_context(ctx_from_path!(trips_path))?;

    let map_line_routes = map_line_routes(&gtfs_routes_collection);
    let lines = make_lines(&gtfs_trips, &map_line_routes);
    collections.lines = CollectionWithId::new(lines)?;

    let routes = make_routes(&gtfs_trips, &map_line_routes);
    collections.routes = CollectionWithId::new(routes)?;

    let (vehicle_journeys, trip_properties) =
        make_ntfs_vehicle_journeys(&gtfs_trips, &gtfs_routes_collection, &collections.datasets)
            .with_context(ctx_from_path!(trips_path))?;
    collections.vehicle_journeys = CollectionWithId::new(vehicle_journeys)?;
    collections.trip_properties = CollectionWithId::new(trip_properties)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate tempdir;
    use self::tempdir::TempDir;
    use collection::{Collection, CollectionWithId, Id};
    use gtfs::add_prefix_to_collections;
    use gtfs::read::EquipmentList;
    use model::Collections;
    use objects::*;
    use std::fs::File;
    use std::io::prelude::*;

    fn create_file_with_content(temp_dir: &TempDir, file_name: &str, content: &str) {
        let file_path = temp_dir.path().join(file_name);
        let mut f = File::create(&file_path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    fn test_in_tmp_dir<F>(func: F)
    where
        F: FnOnce(&TempDir),
    {
        let tmp_dir = TempDir::new("navitia_model_tests").expect("create temp dir");
        func(&tmp_dir);
        tmp_dir.close().expect("delete temp dir");
    }

    #[test]
    fn load_minimal_agency() {
        let agency_content = "agency_name,agency_url,agency_timezone\n\
                              My agency,http://my-agency_url.com,Europe/London";

        test_in_tmp_dir(|ref tmp_dir| {
            create_file_with_content(&tmp_dir, "agency.txt", agency_content);
            let (networks, companies) = super::read_agency(tmp_dir.path()).unwrap();
            assert_eq!(1, networks.len());
            let agency = networks.iter().next().unwrap().1;
            assert_eq!("default_agency_id", agency.id);
            assert_eq!(1, companies.len());
        });
    }

    #[test]
    fn load_standard_agency() {
        let agency_content = "agency_id,agency_name,agency_url,agency_timezone\n\
                              id_1,My agency,http://my-agency_url.com,Europe/London";

        test_in_tmp_dir(|ref tmp_dir| {
            create_file_with_content(&tmp_dir, "agency.txt", agency_content);
            let (networks, companies) = super::read_agency(tmp_dir.path()).unwrap();
            assert_eq!(1, networks.len());
            assert_eq!(1, companies.len());
        });
    }

    #[test]
    fn load_complete_agency() {
        let agency_content =
            "agency_id,agency_name,agency_url,agency_timezone,agency_lang,agency_phone,\
             agency_fare_url,agency_email\n\
             id_1,My agency,http://my-agency_url.com,Europe/London,EN,0123456789,\
             http://my-agency_fare_url.com,my-mail@example.com";

        test_in_tmp_dir(|ref tmp_dir| {
            create_file_with_content(&tmp_dir, "agency.txt", agency_content);
            let (networks, companies) = super::read_agency(tmp_dir.path()).unwrap();
            assert_eq!(1, networks.len());
            let network = networks.iter().next().unwrap().1;
            assert_eq!("id_1", network.id);
            assert_eq!(1, companies.len());
        });
    }

    #[test]
    #[should_panic]
    fn load_2_agencies_with_no_id() {
        let agency_content = "agency_name,agency_url,agency_timezone\n\
                              My agency 1,http://my-agency_url.com,Europe/London\
                              My agency 2,http://my-agency_url.com,Europe/London";

        test_in_tmp_dir(|ref tmp_dir| {
            create_file_with_content(&tmp_dir, "agency.txt", agency_content);
            super::read_agency(tmp_dir.path()).unwrap();
        });
    }

    #[test]
    fn load_one_stop_point() {
        let stops_content = "stop_id,stop_name,stop_lat,stop_lon\n\
                             id1,my stop name,0.1,1.2";

        test_in_tmp_dir(|ref tmp_dir| {
            create_file_with_content(&tmp_dir, "stops.txt", stops_content);
            let mut equipments = EquipmentList::default();
            let mut comments: CollectionWithId<Comment> = CollectionWithId::default();
            let (stop_areas, stop_points) =
                super::read_stops(tmp_dir.path(), &mut comments, &mut equipments).unwrap();
            assert_eq!(1, stop_areas.len());
            assert_eq!(1, stop_points.len());
            let stop_area = stop_areas.iter().next().unwrap().1;
            assert_eq!("Navitia:id1", stop_area.id);

            assert_eq!(1, stop_points.len());
            let stop_point = stop_points.iter().next().unwrap().1;
            assert_eq!("Navitia:id1", stop_point.stop_area_id);
        });
    }

    #[test]
    fn stop_code_on_stops() {
        let stops_content =
            "stop_id,stop_code,stop_name,stop_lat,stop_lon,location_type,parent_station\n\
             stoppoint_id,1234,my stop name,0.1,1.2,0,stop_area_id\n\
             stoparea_id,5678,stop area name,0.1,1.2,1,";

        test_in_tmp_dir(|ref tmp_dir| {
            create_file_with_content(&tmp_dir, "stops.txt", stops_content);
            let mut equipments = EquipmentList::default();
            let mut comments: CollectionWithId<Comment> = CollectionWithId::default();
            let (stop_areas, stop_points) =
                super::read_stops(tmp_dir.path(), &mut comments, &mut equipments).unwrap();
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
        });
    }

    #[test]
    fn no_stop_code_on_autogenerated_stoparea() {
        let stops_content =
            "stop_id,stop_code,stop_name,stop_lat,stop_lon,location_type,parent_station\n\
             stoppoint_id,1234,my stop name,0.1,1.2,0,";

        test_in_tmp_dir(|ref tmp_dir| {
            create_file_with_content(&tmp_dir, "stops.txt", stops_content);
            let mut equipments = EquipmentList::default();
            let mut comments: CollectionWithId<Comment> = CollectionWithId::default();
            let (stop_areas, _) =
                super::read_stops(tmp_dir.path(), &mut comments, &mut equipments).unwrap();
            //validate stop_area code
            assert_eq!(1, stop_areas.len());
            let stop_area = stop_areas.iter().next().unwrap().1;
            assert_eq!(0, stop_area.codes.len());
        });
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

        test_in_tmp_dir(|ref tmp_dir| {
            create_file_with_content(&tmp_dir, "routes.txt", routes_content);
            create_file_with_content(&tmp_dir, "trips.txt", trips_content);
            let mut collections = Collections::default();
            let (contributors, datasets) = super::read_config(None::<&str>).unwrap();
            collections.contributors = contributors;
            collections.datasets = datasets;
            super::read_routes(tmp_dir, &mut collections).unwrap();
            assert_eq!(4, collections.lines.len());
            assert_eq!(2, collections.commercial_modes.len());

            assert_eq!(
                extract(|cm| &cm.name, &collections.commercial_modes),
                &["Bus", "Rail"]
            );

            let lines_commercial_modes_id: Vec<String> = collections
                .lines
                .values()
                .map(|l| l.commercial_mode_id.clone())
                .collect();
            assert!(lines_commercial_modes_id.contains(&"2".to_string()));
            assert!(lines_commercial_modes_id.contains(&"3".to_string()));
            assert!(!lines_commercial_modes_id.contains(&"8".to_string()));

            assert_eq!(2, collections.physical_modes.len());
            assert_eq!(
                extract(|pm| &pm.name, &collections.physical_modes),
                &["Bus", "Train"]
            );

            assert_eq!(5, collections.routes.len());

            assert_eq!(
                extract_ids(&collections.routes),
                &["route_1", "route_1_R", "route_2", "route_3", "route_4"]
            );
        });
    }

    #[test]
    fn gtfs_routes_as_route() {
        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1A,3,8F7A32,FFFFFF\n\
                              route_2,agency_1,1,My line 1B,3,8F7A32,FFFFFF\n\
                              route_4,agency_2,1,My line 1B,3,8F7A32,FFFFFF\n\
                              route_3,agency_2,1,My line 1B,3,8F7A32,FFFFFF\n\
                              route_5,,1,My line 1C,3,8F7A32,FFFFFF";

        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,0,service_1,,\n\
             2,route_2,0,service_1,,\n\
             3,route_3,0,service_2,,\n\
             4,route_4,0,service_2,,\n\
             5,route_5,0,service_3,,";

        test_in_tmp_dir(|ref tmp_dir| {
            create_file_with_content(&tmp_dir, "routes.txt", routes_content);
            create_file_with_content(&tmp_dir, "trips.txt", trips_content);
            let mut collections = Collections::default();
            let (contributors, datasets) = super::read_config(None::<&str>).unwrap();
            collections.contributors = contributors;
            collections.datasets = datasets;
            super::read_routes(tmp_dir, &mut collections).unwrap();

            assert_eq!(3, collections.lines.len());
            assert_eq!(
                extract_ids(&collections.lines),
                &["route_1", "route_3", "route_5"]
            );
            assert_eq!(5, collections.routes.len());

            assert_eq!(
                extract(|r| &r.line_id, &collections.routes),
                &["route_1", "route_1", "route_3", "route_3", "route_5"]
            );
        });
    }

    #[test]
    fn gtfs_routes_as_route_with_backward_trips() {
        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1A,3,8F7A32,FFFFFF\n\
                              route_2,agency_1,1,My line 1B,3,8F7A32,FFFFFF\n\
                              route_3,agency_2,,My line 2,2,7BC142,000000";

        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,0,service_1,,\n\
             2,route_1,1,service_1,,\n\
             3,route_2,0,service_2,,\n
             4,route_3,0,service_3,,\n\
             5,route_3,1,service_3,,";

        test_in_tmp_dir(|ref tmp_dir| {
            create_file_with_content(&tmp_dir, "routes.txt", routes_content);
            create_file_with_content(&tmp_dir, "trips.txt", trips_content);
            let mut collections = Collections::default();
            let (contributors, datasets) = super::read_config(None::<&str>).unwrap();
            collections.contributors = contributors;
            collections.datasets = datasets;
            super::read_routes(tmp_dir, &mut collections).unwrap();

            assert_eq!(2, collections.lines.len());

            assert_eq!(5, collections.routes.len());
            assert_eq!(
                extract_ids(&collections.routes),
                &["route_1", "route_1_R", "route_2", "route_3", "route_3_R",]
            );
        });
    }

    #[test]
    fn gtfs_routes_as_route_same_name_different_agency() {
        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1A,3,8F7A32,FFFFFF\n\
                              route_2,agency_1,1,My line 1B,3,8F7A32,FFFFFF\n\
                              route_3,agency_2,1,My line 1 for agency 2,3,8F7A32,FFFFFF";

        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,0,service_1,,\n\
             2,route_2,0,service_2,,\n
             3,route_3,0,service_3,,";

        test_in_tmp_dir(|ref tmp_dir| {
            create_file_with_content(&tmp_dir, "routes.txt", routes_content);
            create_file_with_content(&tmp_dir, "trips.txt", trips_content);
            let mut collections = Collections::default();
            let (contributors, datasets) = super::read_config(None::<&str>).unwrap();
            collections.contributors = contributors;
            collections.datasets = datasets;
            super::read_routes(tmp_dir, &mut collections).unwrap();

            assert_eq!(2, collections.lines.len());
            assert_eq!(extract_ids(&collections.lines), &["route_1", "route_3"]);
            assert_eq!(
                extract_ids(&collections.routes),
                &["route_1", "route_2", "route_3",]
            );

            assert_eq!(
                extract(|r| &r.line_id, &collections.routes),
                &["route_1", "route_1", "route_3",]
            );
        });
    }

    #[test]
    fn gtfs_routes_with_no_trips() {
        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1,3,8F7A32,FFFFFF\n\
                              route_2,agency_2,2,My line 2,3,8F7A32,FFFFFF";
        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,0,service_1,,";

        test_in_tmp_dir(|ref tmp_dir| {
            create_file_with_content(&tmp_dir, "routes.txt", routes_content);
            create_file_with_content(&tmp_dir, "trips.txt", trips_content);

            let mut collections = Collections::default();
            let (contributors, datasets) = super::read_config(None::<&str>).unwrap();
            collections.contributors = contributors;
            collections.datasets = datasets;
            super::read_routes(tmp_dir, &mut collections).unwrap();
            assert_eq!(1, collections.lines.len());
            assert_eq!(1, collections.routes.len());
        });
    }

    #[test]
    fn prefix_on_all_pt_object_id() {
        let stops_content =
            "stop_id,stop_name,stop_desc,stop_lat,stop_lon,location_type,parent_station\n\
             sp:01,my stop point name,my first desc,0.1,1.2,0,\n\
             sp:02,my stop point name child,,0.2,1.5,0,sp:01\n\
             sa:03,my stop area name,my second desc,0.3,2.2,1,";
        let agency_content = "agency_id,agency_name,agency_url,agency_timezone,agency_lang\n\
                              584,TAM,http://whatever.canaltp.fr/,Europe/Paris,fr\n\
                              285,Phébus,http://plop.kisio.com/,Europe/London,en";

        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1A,3,8F7A32,FFFFFF\n\
                              route_2,agency_1,2,My line 1B,3,8F7A32,FFFFFF";

        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,0,service_1,,\n\
             2,route_2,1,service_2,1,2";

        test_in_tmp_dir(|ref tmp_dir| {
            create_file_with_content(&tmp_dir, "stops.txt", stops_content);
            create_file_with_content(&tmp_dir, "agency.txt", agency_content);
            create_file_with_content(&tmp_dir, "routes.txt", routes_content);
            create_file_with_content(&tmp_dir, "trips.txt", trips_content);

            let mut collections = Collections::default();

            let mut comments: CollectionWithId<Comment> = CollectionWithId::default();
            let mut equipments = EquipmentList::default();
            let (contributors, datasets) = super::read_config(None::<&str>).unwrap();
            collections.contributors = contributors;
            collections.datasets = datasets;
            let (stop_areas, stop_points) =
                super::read_stops(tmp_dir.path(), &mut comments, &mut equipments).unwrap();
            collections.stop_areas = stop_areas;
            collections.stop_points = stop_points;
            let (networks, companies) = super::read_agency(tmp_dir.path()).unwrap();
            collections.networks = networks;
            collections.companies = companies;
            collections.comments = comments;
            super::read_routes(tmp_dir, &mut collections).unwrap();

            add_prefix_to_collections("my_prefix".to_string(), &mut collections).unwrap();

            assert_eq!(
                vec!["my_prefix:285", "my_prefix:584"],
                extract_ids(&collections.companies)
            );
            assert_eq!(
                vec!["my_prefix:285", "my_prefix:584"],
                extract_ids(&collections.networks)
            );
            assert_eq!(
                vec!["my_prefix:Navitia:sp:01", "my_prefix:sa:03"],
                extract_ids(&collections.stop_areas)
            );

            assert_eq!(
                vec![
                    ("my_prefix:sp:01", "my_prefix:Navitia:sp:01"),
                    ("my_prefix:sp:02", "my_prefix:sp:01"),
                ],
                extract(
                    |sp| (sp.id.as_str(), sp.stop_area_id.as_str()),
                    &collections.stop_points,
                )
            );

            assert_eq!(
                vec!["my_prefix:route_1", "my_prefix:route_2"],
                extract_ids(&collections.lines)
            );
            assert_eq!(
                vec!["my_prefix:route_1", "my_prefix:route_2_R"],
                extract_ids(&collections.routes)
            );
            assert_eq!(
                vec!["my_prefix:1", "my_prefix:2"],
                extract_ids(&collections.trip_properties)
            );

            let comment_vec = collections.comments.into_vec();

            assert_eq!(comment_vec[0].id, "my_prefix:stop:sp:01");
            assert_eq!(comment_vec[0].name, "my first desc");
            assert_eq!(comment_vec[1].id, "my_prefix:stop:sa:03");
            assert_eq!(comment_vec[1].name, "my second desc");
        });
    }

    #[test]
    fn gtfs_trips() {
        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1,3,8F7A32,FFFFFF\n\
                              route_2,agency_2,2,My line 2,3,8F7A32,FFFFFF\n\
                              route_3,agency_3,3,My line 3,3,8F7A32,FFFFFF";
        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,0,service_1,,\n\
             2,route_2,0,service_1,1,2\n\
             3,route_3,0,service_1,1,2";

        test_in_tmp_dir(|ref tmp_dir| {
            create_file_with_content(&tmp_dir, "routes.txt", routes_content);
            create_file_with_content(&tmp_dir, "trips.txt", trips_content);

            let mut collections = Collections::default();
            let (contributors, datasets) = super::read_config(None::<&str>).unwrap();
            collections.contributors = contributors;
            collections.datasets = datasets;

            super::read_routes(tmp_dir, &mut collections).unwrap();
            assert_eq!(3, collections.lines.len());
            assert_eq!(3, collections.routes.len());
            assert_eq!(3, collections.vehicle_journeys.len());
            assert_eq!(2, collections.trip_properties.len());
        });
    }

    #[test]
    fn gtfs_trips_with_no_accessibility_information() {
        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1,3,8F7A32,FFFFFF";
        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,0,service_1,,\n\
             2,route_1,0,service_2,,";

        test_in_tmp_dir(|ref tmp_dir| {
            create_file_with_content(&tmp_dir, "routes.txt", routes_content);
            create_file_with_content(&tmp_dir, "trips.txt", trips_content);

            let mut collections = Collections::default();
            let (contributors, datasets) = super::read_config(None::<&str>).unwrap();
            collections.contributors = contributors;
            collections.datasets = datasets;

            super::read_routes(tmp_dir, &mut collections).unwrap();
            assert_eq!(2, collections.vehicle_journeys.len());
            assert_eq!(0, collections.trip_properties.len());
            for vj in collections.vehicle_journeys.values() {
                assert!(vj.trip_property_id.is_none());
            }
        });
    }

    #[test]
    fn push_on_collection() {
        let mut c = CollectionWithId::default();
        c.push(Comment {
            id: "foo".into(),
            name: "toto".into(),
            comment_type: CommentType::Information,
            url: None,
            label: None,
        }).unwrap();
        assert!(c.push(Comment {
            id: "foo".into(),
            name: "tata".into(),
            comment_type: CommentType::Information,
            url: None,
            label: None,
        }).is_err());
        let id = c.get_idx("foo").unwrap();
        assert_eq!(id, c.iter().next().unwrap().0);
    }

    #[test]
    fn stops_generates_equipments() {
        let stops_content = "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station,wheelchair_boarding\n\
                             sp:01,my stop point name,0.1,1.2,0,,1\n\
                             sp:02,my stop point name child,0.2,1.5,0,sp:01,\n\
                             sa:03,my stop area name,0.3,2.2,1,,2";

        test_in_tmp_dir(|ref tmp_dir| {
            create_file_with_content(&tmp_dir, "stops.txt", stops_content);

            let mut comments: CollectionWithId<Comment> = CollectionWithId::default();
            let mut equipments = EquipmentList::default();
            let (stop_areas, stop_points) =
                super::read_stops(tmp_dir.path(), &mut comments, &mut equipments).unwrap();
            let equipments_collection = CollectionWithId::new(equipments.get_equipments()).unwrap();
            assert_eq!(2, stop_areas.len());
            assert_eq!(2, stop_points.len());
            assert_eq!(2, equipments_collection.len());

            let mut stop_point_equipment_ids: Vec<Option<String>> = stop_points
                .iter()
                .map(|(_, stop_point)| stop_point.equipment_id.clone())
                .collect();
            stop_point_equipment_ids.sort();
            assert_eq!(vec![None, Some("0".to_string())], stop_point_equipment_ids);

            assert_eq!(
                vec![&None, &Some("1".to_string())],
                extract(|sa| &sa.equipment_id, &stop_areas)
            );
            assert_eq!(
                equipments_collection.into_vec(),
                vec![
                    Equipment {
                        id: "0".to_string(),
                        wheelchair_boarding: Availability::Available,
                        sheltered: Availability::InformationNotAvailable,
                        elevator: Availability::InformationNotAvailable,
                        escalator: Availability::InformationNotAvailable,
                        bike_accepted: Availability::InformationNotAvailable,
                        bike_depot: Availability::InformationNotAvailable,
                        visual_announcement: Availability::InformationNotAvailable,
                        audible_announcement: Availability::InformationNotAvailable,
                        appropriate_escort: Availability::InformationNotAvailable,
                        appropriate_signage: Availability::InformationNotAvailable,
                    },
                    Equipment {
                        id: "1".to_string(),
                        wheelchair_boarding: Availability::NotAvailable,
                        sheltered: Availability::InformationNotAvailable,
                        elevator: Availability::InformationNotAvailable,
                        escalator: Availability::InformationNotAvailable,
                        bike_accepted: Availability::InformationNotAvailable,
                        bike_depot: Availability::InformationNotAvailable,
                        visual_announcement: Availability::InformationNotAvailable,
                        audible_announcement: Availability::InformationNotAvailable,
                        appropriate_escort: Availability::InformationNotAvailable,
                        appropriate_signage: Availability::InformationNotAvailable,
                    },
                ]
            );
        });
    }

    #[test]
    fn gtfs_stop_times() {
        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1,3,8F7A32,FFFFFF";

        let stops_content =
            "stop_id,stop_name,stop_desc,stop_lat,stop_lon,location_type,parent_station\n\
             sp:01,my stop point name 1,my first desc,0.1,1.2,0,\n\
             sp:02,my stop point name 2,,0.2,1.5,0,";

        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,0,service_1,,";

        let stop_times_content = "trip_id,arrival_time,departure_time,stop_id,stop_sequence,stop_headsign,pickup_type,drop_off_type,shape_dist_traveled\n\
                                  1,06:00:00,06:00:00,sp:01,1,,,,\n\
                                  1,06:06:27,06:06:27,sp:02,2,,,,";

        test_in_tmp_dir(|ref tmp_dir| {
            create_file_with_content(&tmp_dir, "routes.txt", routes_content);
            create_file_with_content(&tmp_dir, "trips.txt", trips_content);
            create_file_with_content(&tmp_dir, "stop_times.txt", stop_times_content);
            create_file_with_content(&tmp_dir, "stops.txt", stops_content);

            let mut collections = Collections::default();
            let (contributors, datasets) = super::read_config(None::<&str>).unwrap();
            collections.contributors = contributors;
            collections.datasets = datasets;

            let mut comments: CollectionWithId<Comment> = CollectionWithId::default();
            let mut equipments = EquipmentList::default();
            let (_, stop_points) =
                super::read_stops(&tmp_dir, &mut comments, &mut equipments).unwrap();
            collections.stop_points = stop_points;

            super::read_routes(&tmp_dir, &mut collections).unwrap();
            super::manage_stop_times(&mut collections, &tmp_dir).unwrap();

            assert_eq!(
                2,
                collections.vehicle_journeys.into_vec()[0].stop_times.len()
            );
        });
    }

    fn extract<'a, T, S: ::std::cmp::Ord>(f: fn(&'a T) -> S, c: &'a Collection<T>) -> Vec<S> {
        let mut extracted_props: Vec<S> = c.values().map(|l| f(l)).collect();
        extracted_props.sort();
        extracted_props
    }

    fn extract_ids<T: Id<T>>(c: &Collection<T>) -> Vec<&str> {
        extract(T::id, c)
    }
}
