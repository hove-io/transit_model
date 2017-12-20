use std::path;
use csv;
use serde;

use objects::*;
use collection::{Collection, Id};
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
}
impl From<Stop> for StopArea {
    fn from(stop: Stop) -> StopArea {
        StopArea {
            id: stop.id,
            name: stop.name,
            visible: stop.visible,
            coord: Coord {
                lon: stop.lon,
                lat: stop.lat,
            },
        }
    }
}
impl From<Stop> for StopPoint {
    fn from(stop: Stop) -> StopPoint {
        let id = stop.id;
        let stop_area_id = stop.parent_station.unwrap_or_else(|| id.clone());
        StopPoint {
            id: id,
            stop_area_id: stop_area_id,
            name: stop.name,
            visible: stop.visible,
            coord: Coord {
                lon: stop.lon,
                lat: stop.lat,
            },
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

pub fn read<P: AsRef<path::Path>>(path: P) -> PtObjects {
    let path = path.as_ref();
    let mut collections = Collections::default();
    collections.commercial_modes = make_collection(path, "commercial_modes.txt");
    collections.networks = make_collection(path, "networks.txt");
    collections.lines = make_collection(path, "lines.txt");
    collections.routes = make_collection(path, "routes.txt");
    collections.vehicle_journeys = make_collection(path, "trips.txt");
    collections.physical_modes = make_collection(path, "physical_modes.txt");
    manage_stops(&mut collections, path);
    PtObjects::new(collections)
}
