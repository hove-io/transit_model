use std::path;
use csv;
use collection::CollectionWithId;
use {Collections, PtObjects};
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
    #[serde(default, rename = "wheelchair_boarding")]
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
        let stop_area_id = stop.parent_station.unwrap_or_else(|| format!("SA{}", id));
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
        }
    }
}

pub fn read<P: AsRef<path::Path>>(path: P) -> PtObjects {
    let path = path.as_ref();
    let mut collections = Collections::default();
    let (networks, companies) = read_agency(path);
    collections.networks = networks;
    collections.companies = companies;
    let (stopareas, stoppoints) = read_stops(path);
    collections.stop_areas = stopareas;
    collections.stop_points = stoppoints;
    PtObjects::new(collections)
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
        .map(|agency| objects::Network::from(agency))
        .collect();
    let networks = CollectionWithId::new(networks);
    let companies = gtfs_agencies
        .into_iter()
        .map(|agency| objects::Company::from(agency))
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
                    new_stop_area.id = format!("SA{}", new_stop_area.id);
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
