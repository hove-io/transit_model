use std::path;
use csv;
use serde;

use objects::*;
use collection::{Collection, Id, Idx};
use {Collections, PtObjects};

fn make_collection<T>(path: &path::Path, file: &str) -> Collection<T>
where
    T: Id<T>,
    for<'de> T: serde::Deserialize<'de>,
{
    let mut rdr = csv::Reader::from_path(path.join(file)).unwrap();
    let vec = rdr.deserialize().map(Result::unwrap).collect();
    Collection::new(vec)
}

fn default_visible() -> bool {
    true
}

fn de_from_i32<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: ::serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let i = i32::deserialize(deserializer)?;
    Ok(if i == 0 { true } else { false })
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Stop {
    #[serde(rename = "stop_id")] id: String,
    #[serde(rename = "stop_name")] name: String,
    #[serde(default = "default_visible", deserialize_with = "de_from_i32")] visible: bool,
    #[serde(rename = "stop_lon")] lon: f64,
    #[serde(rename = "stop_lat")] lat: f64,
    #[serde(default)] location_type: i32,
    parent_station: Option<String>,
    #[serde(rename = "stop_timezone")] timezone: Option<String>,
    contributor_id: Option<String>,
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
            contributor_id: stop.contributor_id.unwrap(),
        }
    }
}

fn manage_stops(collections: &mut Collections, path: &path::Path) {
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
}

fn manage_stop_times(collections: &mut Collections, path: &path::Path) {
    let mut rdr = csv::Reader::from_path(path.join("stop_times.txt")).unwrap();
    for stop_time in rdr.deserialize().map(Result::unwrap) {
        let stop_time: StopTime = stop_time;
        let stop_point_idx = collections.stop_points.get_idx(&stop_time.stop_id).unwrap();
        let vj_idx = collections
            .vehicle_journeys
            .get_idx(&stop_time.trip_id)
            .unwrap();
        collections.vehicle_journeys.mut_elt(vj_idx, |obj| {
            obj.stop_times.push(::objects::StopTime {
                stop_point_idx: stop_point_idx,
                sequence: stop_time.stop_sequence,
            });
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
    collection.mut_elt(idx, move |obj| {
        obj.codes_mut().push((code.object_system, code.object_code));
    });
}
fn insert_code<T>(collection: &mut Collection<T>, code: Code)
where
    T: Codes + Id<T>,
{
    let idx = match collection.get_idx(&code.object_id) {
        Some(idx) => idx,
        None => {
            eprintln!(
                "object_codes.txt: object {} {} not found",
                code.object_type, code.object_id
            );
            return;
        }
    };
    insert_code_with_idx(collection, idx, code);
}

fn manage_codes(collections: &mut Collections, path: &path::Path) {
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

pub fn read<P: AsRef<path::Path>>(path: P) -> PtObjects {
    let path = path.as_ref();
    let mut collections = Collections::default();
    collections.contributors = make_collection(path, "contributors.txt");
    collections.commercial_modes = make_collection(path, "commercial_modes.txt");
    collections.networks = make_collection(path, "networks.txt");
    collections.lines = make_collection(path, "lines.txt");
    collections.routes = make_collection(path, "routes.txt");
    collections.vehicle_journeys = make_collection(path, "trips.txt");
    collections.physical_modes = make_collection(path, "physical_modes.txt");
    manage_stops(&mut collections, path);
    manage_stop_times(&mut collections, path);
    manage_codes(&mut collections, path);
    PtObjects::new(collections)
}
