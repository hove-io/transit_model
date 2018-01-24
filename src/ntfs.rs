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

use std::collections::HashMap;
use std::path;
use csv;
use serde;

use objects::*;
use collection::{Collection, CollectionWithId, Id, Idx};
use {Collections, PtObjects};
use utils::*;

fn make_collection_with_id<T>(path: &path::Path, file: &str) -> CollectionWithId<T>
where
    T: Id<T>,
    for<'de> T: serde::Deserialize<'de>,
{
    info!("Reading {}", file);
    let mut rdr = csv::Reader::from_path(path.join(file)).unwrap();
    let vec = rdr.deserialize().map(Result::unwrap).collect();
    CollectionWithId::new(vec)
}

fn make_collection<T>(path: &path::Path, file: &str) -> Collection<T>
where
    for<'de> T: serde::Deserialize<'de>,
{
    info!("Reading {}", file);
    if !path.join(file).exists() {
        panic!("file {} does not exist", file);
    }
    let mut rdr = csv::Reader::from_path(path.join(file)).unwrap();
    let vec = rdr.deserialize().map(Result::unwrap).collect();
    Collection::new(vec)
}

fn default_visible() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Stop {
    #[serde(rename = "stop_id")]
    id: String,
    #[serde(rename = "stop_name")]
    name: String,
    #[serde(default = "default_visible", deserialize_with = "de_from_u8",
            serialize_with = "ser_from_bool")]
    visible: bool,
    fare_zone_id: Option<String>,
    #[serde(rename = "stop_lon")]
    lon: f64,
    #[serde(rename = "stop_lat")]
    lat: f64,
    #[serde(default)]
    location_type: i32,
    parent_station: Option<String>,
    #[serde(rename = "stop_timezone")]
    timezone: Option<String>,
    geometry_id: Option<String>,
    equipment_id: Option<String>,
}
impl From<Stop> for StopArea {
    fn from(stop: Stop) -> StopArea {
        StopArea {
            id: stop.id,
            name: stop.name,
            codes: CodesT::default(),
            comment_links: CommentLinksT::default(),
            visible: stop.visible,
            coord: Coord {
                lon: stop.lon,
                lat: stop.lat,
            },
            timezone: stop.timezone,
            geometry_id: stop.geometry_id,
            equipment_id: stop.equipment_id,
        }
    }
}
impl From<Stop> for StopPoint {
    fn from(stop: Stop) -> StopPoint {
        let id = stop.id;
        let stop_area_id = stop.parent_station.unwrap_or_else(|| id.clone());
        StopPoint {
            id: id,
            name: stop.name,
            codes: CodesT::default(),
            comment_links: CommentLinksT::default(),
            visible: stop.visible,
            coord: Coord {
                lon: stop.lon,
                lat: stop.lat,
            },
            stop_area_id: stop_area_id,
            timezone: stop.timezone,
            geometry_id: stop.geometry_id,
            equipment_id: stop.equipment_id,
        }
    }
}

fn manage_stops(collections: &mut Collections, path: &path::Path) {
    info!("Reading stops.txt");
    let mut rdr = csv::Reader::from_path(path.join("stops.txt")).unwrap();
    let mut stop_areas = vec![];
    let mut stop_points = vec![];
    for stop in rdr.deserialize().map(Result::unwrap) {
        let stop: Stop = stop;
        match stop.location_type {
            0 => {
                if stop.parent_station.is_none() {
                    let mut new_stop_area = stop.clone();
                    new_stop_area.id = format!("Navitia:{}", new_stop_area.id);
                    stop_areas.push(StopArea::from(new_stop_area));
                }
                stop_points.push(StopPoint::from(stop));
            }
            1 => stop_areas.push(StopArea::from(stop)),
            _ => (),
        }
    }
    collections.stop_areas = CollectionWithId::new(stop_areas);
    collections.stop_points = CollectionWithId::new(stop_points);
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct StopTime {
    stop_id: String,
    trip_id: String,
    stop_sequence: u32,
    arrival_time: Time,
    departure_time: Time,
    #[serde(default)]
    boarding_duration: u16,
    #[serde(default)]
    alighting_duration: u16,
    #[serde(default)]
    pickup_type: u8,
    #[serde(default)]
    dropoff_type: u8,
    #[serde(default, deserialize_with = "de_from_u8", serialize_with = "ser_from_bool")]
    datetime_estimated: bool,
    local_zone_id: Option<u16>,
}

fn manage_stop_times(collections: &mut Collections, path: &path::Path) {
    info!("Reading stop_times.txt");
    let mut rdr = csv::Reader::from_path(path.join("stop_times.txt")).unwrap();
    for stop_time in rdr.deserialize().map(Result::unwrap) {
        let stop_time: StopTime = stop_time;
        let stop_point_idx = collections.stop_points.get_idx(&stop_time.stop_id).unwrap();
        let vj_idx = collections
            .vehicle_journeys
            .get_idx(&stop_time.trip_id)
            .unwrap();
        collections
            .vehicle_journeys
            .index_mut(vj_idx)
            .stop_times
            .push(::objects::StopTime {
                stop_point_idx: stop_point_idx,
                sequence: stop_time.stop_sequence,
                arrival_time: stop_time.arrival_time,
                departure_time: stop_time.departure_time,
                boarding_duration: stop_time.boarding_duration,
                alighting_duration: stop_time.alighting_duration,
                pickup_type: stop_time.pickup_type,
                dropoff_type: stop_time.dropoff_type,
                datetime_estimated: stop_time.datetime_estimated,
                local_zone_id: stop_time.local_zone_id,
            });
    }
    let mut vehicle_journeys = collections.vehicle_journeys.take();
    for vj in &mut vehicle_journeys {
        vj.stop_times.sort_unstable_by_key(|st| st.sequence);
    }
    collections.vehicle_journeys = CollectionWithId::new(vehicle_journeys);
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Code {
    object_type: String,
    object_id: String,
    object_system: String,
    object_code: String,
}

fn insert_code_with_idx<T>(collection: &mut CollectionWithId<T>, idx: Idx<T>, code: Code)
where
    T: Codes + Id<T>,
{
    collection
        .index_mut(idx)
        .codes_mut()
        .push((code.object_system, code.object_code));
}
fn insert_code<T>(collection: &mut CollectionWithId<T>, code: Code)
where
    T: Codes + Id<T>,
{
    let idx = match collection.get_idx(&code.object_id) {
        Some(idx) => idx,
        None => {
            error!(
                "object_codes.txt: object_type={} object_id={} not found",
                code.object_type, code.object_id
            );
            return;
        }
    };
    insert_code_with_idx(collection, idx, code);
}

fn manage_codes(collections: &mut Collections, path: &path::Path) {
    info!("Reading object_codes.txt");
    let mut rdr = csv::Reader::from_path(path.join("object_codes.txt")).unwrap();
    for code in rdr.deserialize().map(Result::unwrap) {
        let code: Code = code;
        match code.object_type.as_str() {
            "stop_area" => insert_code(&mut collections.stop_areas, code),
            "stop_point" => insert_code(&mut collections.stop_points, code),
            "network" => insert_code(&mut collections.networks, code),
            "line" => insert_code(&mut collections.lines, code),
            "route" => insert_code(&mut collections.routes, code),
            "trip" => insert_code(&mut collections.vehicle_journeys, code),
            _ => panic!("{} is not a valid object_type", code.object_type),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct CalendarDate {
    service_id: String,
    #[serde(deserialize_with = "de_from_date_string", serialize_with = "ser_from_naive_date")]
    date: Date,
    exception_type: ExceptionType,
}

fn insert_calendar_date(collection: &mut CollectionWithId<Calendar>, calendar_date: CalendarDate) {
    let idx = match collection.get_idx(&calendar_date.service_id) {
        Some(idx) => idx,
        None => {
            error!(
                "calendar_dates.txt: service_id={} not found",
                calendar_date.service_id
            );
            return;
        }
    };
    collection
        .index_mut(idx)
        .calendar_dates
        .push((calendar_date.date, calendar_date.exception_type))
}

fn manage_calendars(collections: &mut Collections, path: &path::Path) {
    collections.calendars = make_collection_with_id(path, "calendar.txt");

    info!("Reading calendar_dates.txt");
    if let Ok(mut rdr) = csv::Reader::from_path(path.join("calendar_dates.txt")) {
        for calendar_date in rdr.deserialize().map(Result::unwrap) {
            let calendar_date: CalendarDate = calendar_date;
            insert_calendar_date(&mut collections.calendars, calendar_date);
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct FeedInfo {
    #[serde(rename = "feed_info_param")]
    info_param: String,
    #[serde(rename = "feed_info_value")]
    info_value: String,
}

fn manage_feed_infos(collections: &mut Collections, path: &path::Path) {
    info!("Reading feed_infos.txt");
    let mut rdr = csv::Reader::from_path(path.join("feed_infos.txt")).unwrap();
    collections.feed_infos = rdr.deserialize::<FeedInfo>().map(Result::unwrap).fold(
        HashMap::default(),
        |mut acc, r| {
            assert!(
                acc.insert(r.info_param.to_string(), r.info_value.to_string())
                    .is_none(),
                "{} already found in file feed_infos.txt",
                r.info_param,
            );
            acc
        },
    )
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct CommentLink {
    object_id: String,
    object_type: String,
    comment_id: String,
}

fn insert_comment_link<T>(collection: &mut CollectionWithId<T>, comment_link: CommentLink)
where
    T: CommentLinks + Id<T>,
{
    let idx = match collection.get_idx(&comment_link.object_id) {
        Some(idx) => idx,
        None => {
            error!(
                "comment_links.txt: object_type={} object_id={} not found",
                comment_link.object_type, comment_link.object_id
            );
            return;
        }
    };
    collection
        .index_mut(idx)
        .comment_links_mut()
        .push(comment_link.comment_id);
}

fn manage_comments(collections: &mut Collections, path: &path::Path) {
    if path.join("comments.txt").exists() {
        collections.comments = make_collection_with_id(path, "comments.txt");

        if let Ok(mut rdr) = csv::Reader::from_path(path.join("comment_links.txt")) {
            info!("Reading comment_links.txt");
            for comment_link in rdr.deserialize().map(Result::unwrap) {
                let comment_link: CommentLink = comment_link;
                match comment_link.object_type.as_str() {
                    "stop_area" => insert_comment_link(&mut collections.stop_areas, comment_link),
                    "stop_point" => insert_comment_link(&mut collections.stop_points, comment_link),
                    "line" => insert_comment_link(&mut collections.lines, comment_link),
                    "route" => insert_comment_link(&mut collections.routes, comment_link),
                    "trip" => insert_comment_link(&mut collections.vehicle_journeys, comment_link),
                    "stop_time" => warn!("comments are not added to StopTime yet"),
                    "line_group" => warn!("line_groups.txt is not parsed yet"),
                    _ => panic!("{} is not a valid object_type", comment_link.object_type),
                }
            }
        }
    }
}

pub fn read<P: AsRef<path::Path>>(path: P) -> PtObjects {
    let path = path.as_ref();
    info!("Loading NTFS from {:?}", path);
    let mut collections = Collections::default();
    collections.contributors = make_collection_with_id(path, "contributors.txt");
    collections.datasets = make_collection_with_id(path, "datasets.txt");
    collections.commercial_modes = make_collection_with_id(path, "commercial_modes.txt");
    collections.networks = make_collection_with_id(path, "networks.txt");
    collections.lines = make_collection_with_id(path, "lines.txt");
    collections.routes = make_collection_with_id(path, "routes.txt");
    collections.vehicle_journeys = make_collection_with_id(path, "trips.txt");
    collections.physical_modes = make_collection_with_id(path, "physical_modes.txt");
    manage_calendars(&mut collections, path);
    collections.companies = make_collection_with_id(path, "companies.txt");
    manage_feed_infos(&mut collections, path);
    manage_stops(&mut collections, path);
    manage_stop_times(&mut collections, path);
    manage_codes(&mut collections, path);
    manage_comments(&mut collections, path);
    collections.equipments = make_collection_with_id(path, "equipments.txt");
    collections.transfers = make_collection(path, "transfers.txt");
    info!("Indexing");
    let res = PtObjects::new(collections);
    info!("Loading NTFS done");
    res
}
