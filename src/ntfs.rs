use std::collections::HashMap;
use std::path;
use csv;
use serde;

use objects::*;
use collection::{Collection, Id, Idx};
use {Collections, PtObjects};
use utils::*;
use chrono::NaiveDate;

fn make_collection<T>(path: &path::Path, file: &str) -> Collection<T>
where
    T: Id<T>,
    for<'de> T: serde::Deserialize<'de>,
{
    info!("Reading {}", file);
    let mut rdr = csv::Reader::from_path(path.join(file)).unwrap();
    let vec = rdr.deserialize().map(Result::unwrap).collect();
    Collection::new(vec)
}

fn default_visible() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Stop {
    #[serde(rename = "stop_id")] id: String,
    #[serde(rename = "stop_name")] name: String,
    #[serde(default = "default_visible", deserialize_with = "de_from_u8",
            serialize_with = "ser_from_bool")]
    visible: bool,
    #[serde(rename = "stop_lon")] lon: f64,
    #[serde(rename = "stop_lat")] lat: f64,
    #[serde(default)] location_type: i32,
    parent_station: Option<String>,
    #[serde(rename = "stop_timezone")] timezone: Option<String>,
}
impl From<Stop> for StopArea {
    fn from(stop: Stop) -> StopArea {
        StopArea {
            id: stop.id,
            name: stop.name,
            codes: CodesT::default(),
            visible: stop.visible,
            coord: Coord {
                lon: stop.lon,
                lat: stop.lat,
            },
            timezone: stop.timezone,
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
            visible: stop.visible,
            coord: Coord {
                lon: stop.lon,
                lat: stop.lat,
            },
            stop_area_id: stop_area_id,
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
                    stop_areas.push(StopArea::from(stop.clone()));
                }
                stop_points.push(StopPoint::from(stop));
            }
            1 => stop_areas.push(StopArea::from(stop)),
            _ => (),
        }
    }
    collections.stop_areas = Collection::new(stop_areas);
    collections.stop_points = Collection::new(stop_points);
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct StopTime {
    stop_id: String,
    trip_id: String,
    stop_sequence: u32,
    arrival_time: Time,
    departure_time: Time,
    #[serde(default)] boarding_duration: u16,
    #[serde(default)] alighting_duration: u16,
    #[serde(default)] pickup_type: u8,
    #[serde(default)] dropoff_type: u8,
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
    collections.vehicle_journeys = Collection::new(vehicle_journeys);
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Code {
    object_type: String,
    object_id: String,
    object_system: String,
    object_code: String,
}

fn insert_code_with_idx<T>(collection: &mut Collection<T>, idx: Idx<T>, code: Code)
where
    T: Codes + Id<T>,
{
    collection
        .index_mut(idx)
        .codes_mut()
        .push((code.object_system, code.object_code));
}
fn insert_code<T>(collection: &mut Collection<T>, code: Code)
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
    calendar_id: String,
    #[serde(deserialize_with = "de_from_date_string", serialize_with = "ser_from_naive_date")]
    date: NaiveDate,
    exception_type: ExceptionType,
}

fn insert_calendar_date(collection: &mut Collection<Calendar>, calendar_date: CalendarDate) {
    let idx = match collection.get_idx(&calendar_date.calendar_id) {
        Some(idx) => idx,
        None => {
            error!(
                "calendar_dates.txt: calendar_id={} not found",
                calendar_date.calendar_id
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
    info!("Reading calendar.txt");
    collections.calendars = make_collection(path, "calendar.txt");

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
    #[serde(rename = "feed_info_param")] info_param: String,
    #[serde(rename = "feed_info_value")] info_value: String,
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

pub fn read<P: AsRef<path::Path>>(path: P) -> PtObjects {
    let path = path.as_ref();
    info!("Loading NTFS from {:?}", path);
    let mut collections = Collections::default();
    collections.contributors = make_collection(path, "contributors.txt");
    collections.datasets = make_collection(path, "datasets.txt");
    collections.commercial_modes = make_collection(path, "commercial_modes.txt");
    collections.networks = make_collection(path, "networks.txt");
    collections.lines = make_collection(path, "lines.txt");
    collections.routes = make_collection(path, "routes.txt");
    collections.vehicle_journeys = make_collection(path, "trips.txt");
    collections.physical_modes = make_collection(path, "physical_modes.txt");
    manage_calendars(&mut collections, path);
    collections.companies = make_collection(path, "companies.txt");
    manage_feed_infos(&mut collections, path);
    manage_stops(&mut collections, path);
    manage_stop_times(&mut collections, path);
    manage_codes(&mut collections, path);
    info!("Indexing");
    let res = PtObjects::new(collections);
    info!("Loading NTFS done");
    res
}
