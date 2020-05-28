// Copyright (C) 2017 Kisio Digital and/or its affiliates.
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by the
// Free Software Foundation, version 3.

// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.

// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>

//! [NTFS](https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fr.md)
//! format management.

#![allow(missing_docs)]
pub mod filter;
mod read;
mod write;

use crate::{
    calendars::{manage_calendars, write_calendar_dates},
    model::{Collections, Model},
    objects::*,
    read_utils,
    utils::*,
    Result,
};
use chrono::{DateTime, FixedOffset};
use derivative::Derivative;
use log::info;
use serde::{Deserialize, Serialize};
use std::path;
use tempfile::tempdir;

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
    drop_off_type: u8,
    datetime_estimated: Option<u8>,
    local_zone_id: Option<u16>,
    stop_headsign: Option<String>,
    stop_time_id: Option<String>,
    #[serde(rename = "stop_time_precision")]
    precision: Option<StopTimePrecision>,
}

#[derivative(Default)]
#[derive(Derivative, Serialize, Deserialize, Debug, Clone, PartialEq)]
enum StopLocationType {
    #[derivative(Default)]
    #[serde(rename = "0")]
    StopPoint,
    #[serde(rename = "1")]
    StopArea,
    #[serde(rename = "2")]
    GeographicArea,
    #[serde(rename = "3")]
    EntranceExit,
    #[serde(rename = "4")]
    PathwayInterconnectionNode,
    #[serde(rename = "5")]
    BoardingArea,
}

impl From<StopLocationType> for StopType {
    fn from(stop_location_type: StopLocationType) -> StopType {
        match stop_location_type {
            StopLocationType::StopPoint => StopType::Point,
            StopLocationType::StopArea => StopType::Zone,
            StopLocationType::GeographicArea => StopType::Zone,
            StopLocationType::EntranceExit => StopType::StopEntrance,
            StopLocationType::PathwayInterconnectionNode => StopType::GenericNode,
            StopLocationType::BoardingArea => StopType::BoardingArea,
        }
    }
}

impl From<StopType> for StopLocationType {
    fn from(stop_type: StopType) -> StopLocationType {
        match stop_type {
            StopType::Point => StopLocationType::StopPoint,
            StopType::Zone => StopLocationType::StopArea,
            StopType::StopEntrance => StopLocationType::EntranceExit,
            StopType::GenericNode => StopLocationType::PathwayInterconnectionNode,
            StopType::BoardingArea => StopLocationType::BoardingArea,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Stop {
    #[serde(rename = "stop_id")]
    id: String,
    #[serde(rename = "stop_name")]
    name: String,
    #[serde(rename = "stop_code")]
    code: Option<String>,
    #[serde(
        default = "default_visible",
        deserialize_with = "de_from_u8",
        serialize_with = "ser_from_bool"
    )]
    visible: bool,
    fare_zone_id: Option<String>,
    #[serde(rename = "stop_lon")]
    lon: String,
    #[serde(rename = "stop_lat")]
    lat: String,
    #[serde(default, deserialize_with = "de_with_empty_default")]
    location_type: StopLocationType,
    parent_station: Option<String>,
    #[serde(rename = "stop_timezone")]
    timezone: Option<String>,
    geometry_id: Option<String>,
    equipment_id: Option<String>,
    level_id: Option<String>,
    platform_code: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct CommentLink {
    object_id: String,
    object_type: ObjectType,
    comment_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Code {
    object_type: ObjectType,
    object_id: String,
    object_system: String,
    object_code: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ObjectProperty {
    object_type: ObjectType,
    object_id: String,
    object_property_name: String,
    object_property_value: String,
}

fn default_visible() -> bool {
    true
}

/// Checks if minimum FaresV2 collections are defined and not empty (ticket_use_restrictions and ticket_prices are optional)
/// See https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fare_extension.md
fn has_fares_v2(collections: &Collections) -> bool {
    !collections.tickets.is_empty()
        && !collections.ticket_uses.is_empty()
        && !collections.ticket_use_perimeters.is_empty()
}

/// Checks if minimum FaresV1 collections are defined and not empty (fares_v1 is optional)
/// `prices.csv` and `od_fares.csv` are mandatory but od_fares.csv can be empty.
/// See https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fare_extension_fr_deprecated.md
fn has_fares_v1(collections: &Collections) -> bool {
    !collections.prices_v1.is_empty()
}
/// Imports a `Model` from the
/// [NTFS](https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fr.md)
/// files in the given directory.
pub fn read<P: AsRef<path::Path>>(path: P) -> Result<Model> {
    let path = path.as_ref();
    let mut file_handle = read_utils::PathFileHandler::new(path.to_path_buf());

    info!("Loading NTFS from {:?}", path);
    let mut collections = Collections::default();
    collections.contributors = make_collection_with_id(path, "contributors.txt")?;
    collections.datasets = make_collection_with_id(path, "datasets.txt")?;
    collections.commercial_modes = make_collection_with_id(path, "commercial_modes.txt")?;
    collections.networks = make_collection_with_id(path, "networks.txt")?;
    collections.lines = make_collection_with_id(path, "lines.txt")?;
    collections.routes = make_collection_with_id(path, "routes.txt")?;
    collections.vehicle_journeys = make_collection_with_id(path, "trips.txt")?;
    collections.frequencies = make_opt_collection(path, "frequencies.txt")?;
    collections.physical_modes = make_collection_with_id(path, "physical_modes.txt")?;
    collections.companies = make_collection_with_id(path, "companies.txt")?;
    collections.equipments = make_opt_collection_with_id(path, "equipments.txt")?;
    collections.trip_properties = make_opt_collection_with_id(path, "trip_properties.txt")?;
    collections.transfers = make_opt_collection(path, "transfers.txt")?;
    collections.admin_stations = make_opt_collection(path, "admin_stations.txt")?;
    collections.tickets = make_opt_collection_with_id(path, "tickets.txt")?;
    collections.ticket_uses = make_opt_collection_with_id(path, "ticket_uses.txt")?;
    collections.ticket_prices = make_opt_collection(path, "ticket_prices.txt")?;
    collections.ticket_use_perimeters = make_opt_collection(path, "ticket_use_perimeters.txt")?;
    collections.ticket_use_restrictions = make_opt_collection(path, "ticket_use_restrictions.txt")?;
    collections.levels = make_opt_collection_with_id(path, "levels.txt")?;
    collections.grid_calendars = make_opt_collection_with_id(path, "grid_calendars.txt")?;
    collections.grid_exception_dates = make_opt_collection(path, "grid_exception_dates.txt")?;
    collections.grid_periods = make_opt_collection(path, "grid_periods.txt")?;
    collections.grid_rel_calendar_line = make_opt_collection(path, "grid_rel_calendar_line.txt")?;
    manage_calendars(&mut file_handle, &mut collections)?;
    read::manage_geometries(&mut collections, path)?;
    read::manage_feed_infos(&mut collections, path)?;
    read::manage_stops(&mut collections, path)?;
    read::manage_pathways(&mut collections, path)?;
    read::manage_stop_times(&mut collections, path)?;
    read::manage_codes(&mut collections, path)?;
    read::manage_comments(&mut collections, path)?;
    read::manage_object_properties(&mut collections, path)?;
    read::manage_fares_v1(&mut collections, path)?;
    read::manage_companies_on_vj(&mut collections)?;
    info!("Indexing");
    let res = Model::new(collections)?;
    info!("Loading NTFS done");
    Ok(res)
}

/// Exports a `Model` to the
/// [NTFS](https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fr.md)
/// files in the given directory.
pub fn write<P: AsRef<path::Path>>(
    model: &Model,
    path: P,
    current_datetime: DateTime<FixedOffset>,
) -> Result<()> {
    let path = path.as_ref();
    info!("Writing NTFS to {:?}", path);

    write::write_feed_infos(path, &model, current_datetime)?;
    write_collection_with_id(path, "contributors.txt", &model.contributors)?;
    write_collection_with_id(path, "datasets.txt", &model.datasets)?;
    write_collection_with_id(path, "networks.txt", &model.networks)?;
    write_collection_with_id(path, "commercial_modes.txt", &model.commercial_modes)?;
    write_collection_with_id(path, "companies.txt", &model.companies)?;
    write_collection_with_id(path, "lines.txt", &model.lines)?;
    write_collection_with_id(path, "physical_modes.txt", &model.physical_modes)?;
    write_collection_with_id(path, "equipments.txt", &model.equipments)?;
    write_collection_with_id(path, "routes.txt", &model.routes)?;
    write_collection_with_id(path, "trip_properties.txt", &model.trip_properties)?;
    write_collection_with_id(path, "geometries.txt", &model.geometries)?;
    write_collection(path, "transfers.txt", &model.transfers)?;
    write_collection(path, "admin_stations.txt", &model.admin_stations)?;
    write_collection_with_id(path, "tickets.txt", &model.tickets)?;
    write_collection_with_id(path, "ticket_uses.txt", &model.ticket_uses)?;
    write_collection(path, "ticket_prices.txt", &model.ticket_prices)?;
    write_collection(
        path,
        "ticket_use_perimeters.txt",
        &model.ticket_use_perimeters,
    )?;
    write_collection(
        path,
        "ticket_use_restrictions.txt",
        &model.ticket_use_restrictions,
    )?;
    write_collection_with_id(path, "grid_calendars.txt", &model.grid_calendars)?;
    write_collection(
        path,
        "grid_exception_dates.txt",
        &model.grid_exception_dates,
    )?;
    write_collection(path, "grid_periods.txt", &model.grid_periods)?;
    write_collection(
        path,
        "grid_rel_calendar_line.txt",
        &model.grid_rel_calendar_line,
    )?;
    write::write_vehicle_journeys_and_stop_times(
        path,
        &model.vehicle_journeys,
        &model.stop_points,
        &model.stop_time_headsigns,
        &model.stop_time_ids,
    )?;
    write_collection(path, "frequencies.txt", &model.frequencies)?;
    write_calendar_dates(path, &model.calendars)?;
    write::write_stops(
        path,
        &model.stop_points,
        &model.stop_areas,
        &model.stop_locations,
    )?;
    write::write_comments(path, model)?;
    write::write_codes(path, model)?;
    write::write_object_properties(path, model)?;
    write::write_fares_v1(path, &model)?;
    write_collection_with_id(path, "pathways.txt", &model.pathways)?;
    write_collection_with_id(path, "levels.txt", &model.levels)?;

    Ok(())
}

/// Exports a `Model` to a
/// [NTFS](https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fr.md)
/// ZIP archive at the given full path.
pub fn write_to_zip<P: AsRef<path::Path>>(
    model: &Model,
    path: P,
    current_datetime: DateTime<FixedOffset>,
) -> Result<()> {
    let path = path.as_ref();
    info!("Writing NTFS to ZIP File {:?}", path);
    let input_tmp_dir = tempdir()?;
    write(model, input_tmp_dir.path(), current_datetime)?;
    zip_to(input_tmp_dir.path(), path)?;
    input_tmp_dir.close()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Collections;
    use super::*;
    use super::{read, write};
    use crate::calendars::{manage_calendars, write_calendar_dates};
    use crate::objects::Availability;
    use crate::{read_utils::PathFileHandler, test_utils::*};
    use geo_types::line_string;
    use pretty_assertions::assert_eq;
    use std::{
        collections::{BTreeMap, BTreeSet, HashMap},
        fmt::Debug,
    };
    use typed_index_collection::{Collection, CollectionWithId, Id};

    fn test_serialize_deserialize_collection_with_id<T>(objects: Vec<T>)
    where
        T: Id<T> + PartialEq + Debug + serde::Serialize,
        for<'de> T: serde::Deserialize<'de>,
    {
        let collection = CollectionWithId::new(objects).unwrap();
        test_in_tmp_dir(|path| {
            write_collection_with_id(path, "file.txt", &collection).unwrap();
            let des_collection = make_collection_with_id(path, "file.txt").unwrap();
            assert_eq!(collection, des_collection);
        });
    }

    fn test_serialize_deserialize_collection<T>(objects: Vec<T>)
    where
        T: PartialEq + Debug + serde::Serialize,
        for<'de> T: serde::Deserialize<'de>,
    {
        let collection = Collection::new(objects);
        test_in_tmp_dir(|path| {
            write_collection(path, "file.txt", &collection).unwrap();
            let des_collection = make_opt_collection(path, "file.txt").unwrap();
            assert_eq!(collection, des_collection);
        });
    }

    fn btree_set_from_vec<T: Ord>(input: Vec<T>) -> BTreeSet<T> {
        input.into_iter().collect()
    }

    #[test]
    fn feed_infos_serialization_deserialization() {
        let mut feed_infos = BTreeMap::default();
        feed_infos.insert("tartare_platform".to_string(), "dev".to_string());
        feed_infos.insert("feed_publisher_name".to_string(), "Nicaragua".to_string());

        let dataset = Dataset {
            id: "Foo:0".to_string(),
            contributor_id: "Foo".to_string(),
            start_date: chrono::NaiveDate::from_ymd(2018, 1, 30),
            end_date: chrono::NaiveDate::from_ymd(2018, 1, 31),
            dataset_type: Some(DatasetType::Theorical),
            extrapolation: false,
            desc: Some("description".to_string()),
            system: Some("GTFS V2".to_string()),
        };

        let mut collections = Collections::default();
        collections.datasets = CollectionWithId::from(dataset);
        collections.feed_infos = feed_infos;

        test_in_tmp_dir(|path| {
            write::write_feed_infos(path, &collections, get_test_datetime()).unwrap();
            read::manage_feed_infos(&mut collections, path).unwrap();
            assert_eq!(
                vec![
                    ("feed_creation_date".to_string(), "20190403".to_string()),
                    (
                        "feed_creation_datetime".to_string(),
                        "2019-04-03T17:19:00+00:00".to_string()
                    ),
                    ("feed_creation_time".to_string(), "17:19:00".to_string()),
                    ("feed_end_date".to_string(), "20180131".to_string()),
                    ("feed_publisher_name".to_string(), "Nicaragua".to_string()),
                    ("feed_start_date".to_string(), "20180130".to_string()),
                    ("ntfs_version".to_string(), "0.11.2".to_string()),
                    ("tartare_platform".to_string(), "dev".to_string()),
                ],
                collections
                    .feed_infos
                    .into_iter()
                    .map(|(k, v)| (k, v))
                    .collect::<Vec<(String, String)>>()
            );
        });
    }

    #[test]
    fn networks_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            Network {
                id: "OIF:101".to_string(),
                name: "SAVAC".to_string(),
                url: Some("http://www.vianavigo.com,Europe/Paris".to_string()),
                timezone: Some("Europe/Paris".to_string()),
                lang: Some("fr".to_string()),
                phone: Some("0123456789".to_string()),
                address: Some("somewhere".to_string()),
                sort_order: Some(1),
                codes: KeysValues::default(),
            },
            Network {
                id: "OIF:102".to_string(),
                name: "SAVAC".to_string(),
                url: None,
                timezone: None,
                lang: None,
                phone: None,
                address: None,
                sort_order: None,
                codes: KeysValues::default(),
            },
        ]);
    }

    #[test]
    fn commercial_modes_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            CommercialMode {
                id: "boarding_landing".to_string(),
                name: "Boarding - Landing".to_string(),
            },
            CommercialMode {
                id: "bus".to_string(),
                name: "Bus".to_string(),
            },
        ]);
    }

    #[test]
    fn companies_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            Company {
                id: "OIF:101".to_string(),
                name: "Foo".to_string(),
                address: Some("foo address".to_string()),
                url: Some("http://www.foo.fr/".to_string()),
                mail: Some("contact@foo.fr".to_string()),
                phone: Some("0123456789".to_string()),
            },
            Company {
                id: "OIF:102".to_string(),
                name: "Bar".to_string(),
                address: None,
                url: None,
                mail: None,
                phone: None,
            },
        ]);
    }

    #[test]
    fn lines_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            Line {
                id: "OIF:002002002:BDEOIF829".to_string(),
                name: "DEF".to_string(),
                code: Some("DEF".to_string()),
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                forward_name: Some("Hôtels - Hôtels".to_string()),
                forward_direction: None,
                backward_name: Some("Hôtels - Hôtels".to_string()),
                backward_direction: None,
                color: Some(Rgb {
                    red: 155,
                    green: 12,
                    blue: 89,
                }),
                text_color: Some(Rgb {
                    red: 10,
                    green: 0,
                    blue: 45,
                }),
                sort_order: Some(1342),
                network_id: "OIF:829".to_string(),
                commercial_mode_id: "bus".to_string(),
                geometry_id: Some("Geometry:Line:Relation:6883353".to_string()),
                opening_time: Some(Time::new(9, 0, 0)),
                closing_time: Some(Time::new(18, 0, 0)),
            },
            Line {
                id: "OIF:002002003:3OIF829".to_string(),
                name: "3".to_string(),
                code: None,
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                forward_name: None,
                forward_direction: None,
                backward_name: None,
                backward_direction: None,
                color: None,
                text_color: None,
                sort_order: None,
                network_id: "OIF:829".to_string(),
                commercial_mode_id: "bus".to_string(),
                geometry_id: None,
                opening_time: None,
                closing_time: None,
            },
        ]);
    }

    #[test]
    fn physical_modes_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            PhysicalMode {
                id: "Bus".to_string(),
                name: "Bus".to_string(),
                co2_emission: Some(6.2),
            },
            PhysicalMode {
                id: "Funicular".to_string(),
                name: "Funicular".to_string(),
                co2_emission: None,
            },
            PhysicalMode {
                id: "SuspendedCableCar".to_string(),
                name: "Suspended Cable Car".to_string(),
                co2_emission: None,
            },
        ]);
    }

    #[test]
    fn routes_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            Route {
                id: "IF:002002002:BDE".to_string(),
                name: "Hôtels - Hôtels".to_string(),
                direction_type: Some("forward".to_string()),
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                line_id: "OIF:002002002:BDEOIF829".to_string(),
                geometry_id: Some("Geometry:Line:Relation:6883353".to_string()),
                destination_id: Some("OIF,OIF:SA:4:126".to_string()),
            },
            Route {
                id: "OIF:002002002:CEN".to_string(),
                name: "Hôtels - Hôtels".to_string(),
                direction_type: None,
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                line_id: "OIF:002002002:BDEOIF829".to_string(),
                geometry_id: None,
                destination_id: None,
            },
        ]);
    }

    #[test]
    fn vehicle_journeys_and_stop_times_serialization_deserialization() {
        let stop_points = CollectionWithId::new(vec![
            StopPoint {
                id: "OIF:SP:36:2085".to_string(),
                name: "Gare de Saint-Cyr l'École".to_string(),
                visible: true,
                coord: Coord {
                    lon: 2.073_034,
                    lat: 48.799_115,
                },
                stop_area_id: "OIF:SA:8739322".to_string(),
                timezone: Some("Europe/Paris".to_string()),
                fare_zone_id: Some("1".to_string()),
                stop_type: StopType::Point,
                ..Default::default()
            },
            StopPoint {
                id: "OIF:SP:36:2127".to_string(),
                name: "Division Leclerc".to_string(),
                visible: true,
                coord: Coord {
                    lon: 2.073_407,
                    lat: 48.800_598,
                },
                stop_area_id: "OIF:SA:2:1468".to_string(),
                timezone: Some("Europe/Paris".to_string()),
                stop_type: StopType::Point,
                ..Default::default()
            },
        ])
        .unwrap();
        let vehicle_journeys = CollectionWithId::new(vec![
            VehicleJourney {
                id: "OIF:87604986-1_11595-1".to_string(),
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                route_id: "OIF:078078001:1".to_string(),
                physical_mode_id: "Bus".to_string(),
                dataset_id: "OIF:0".to_string(),
                service_id: "2".to_string(),
                headsign: Some("2005".to_string()),
                short_name: Some("42".to_string()),
                block_id: Some("PLOI".to_string()),
                company_id: "OIF:743".to_string(),
                trip_property_id: Some("0".to_string()),
                geometry_id: Some("Geometry:Line:Relation:6883353".to_string()),
                stop_times: vec![
                    StopTime {
                        id: None,
                        stop_point_idx: stop_points.get_idx("OIF:SP:36:2085").unwrap(),
                        sequence: 0,
                        headsign: None,
                        arrival_time: Time::new(14, 40, 0),
                        departure_time: Time::new(14, 40, 0),
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 0,
                        drop_off_type: 1,
                        datetime_estimated: false,
                        local_zone_id: None,
                        precision: Some(StopTimePrecision::Exact),
                        comment_links: None,
                    },
                    StopTime {
                        id: None,
                        stop_point_idx: stop_points.get_idx("OIF:SP:36:2127").unwrap(),
                        sequence: 1,
                        headsign: None,
                        arrival_time: Time::new(14, 42, 0),
                        departure_time: Time::new(14, 42, 0),
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 0,
                        drop_off_type: 0,
                        datetime_estimated: false,
                        local_zone_id: None,
                        precision: Some(StopTimePrecision::Exact),
                        comment_links: None,
                    },
                ],
                journey_pattern_id: Some(String::from("OIF:JP:1")),
            },
            VehicleJourney {
                id: "OIF:90014407-1_425283-1".to_string(),
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                route_id: "OIF:800:TER".to_string(),
                physical_mode_id: "Bus".to_string(),
                dataset_id: "OIF:0".to_string(),
                service_id: "2".to_string(),
                headsign: None,
                short_name: Some("43".to_string()),
                block_id: None,
                company_id: "OIF:743".to_string(),
                trip_property_id: None,
                geometry_id: None,
                stop_times: vec![],
                journey_pattern_id: Some(String::from("OIF:JP:1")),
            },
        ])
        .unwrap();

        let mut headsigns = HashMap::new();
        headsigns.insert(
            ("OIF:87604986-1_11595-1".to_string(), 1),
            "somewhere".to_string(),
        );
        let mut stop_time_ids = HashMap::new();
        stop_time_ids.insert(
            ("OIF:87604986-1_11595-1".to_string(), 0),
            "StopTime:OIF:87604986-1_11595-1:0".to_string(),
        );

        test_in_tmp_dir(|path| {
            write::write_vehicle_journeys_and_stop_times(
                path,
                &vehicle_journeys,
                &stop_points,
                &headsigns,
                &stop_time_ids,
            )
            .unwrap();

            let mut collections = Collections::default();
            collections.vehicle_journeys =
                make_collection_with_id::<VehicleJourney>(path, "trips.txt").unwrap();
            collections.stop_points = stop_points;

            read::manage_stop_times(&mut collections, path).unwrap();
            assert_eq!(vehicle_journeys, collections.vehicle_journeys);
            assert_eq!(collections.stop_time_headsigns, headsigns);
            assert_eq!(collections.stop_time_ids, stop_time_ids);
        });
    }

    #[test]
    fn contributors_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            Contributor {
                id: "Foo".to_string(),
                name: "Foo".to_string(),
                license: Some("ODbL".to_string()),
                website: Some("http://www.foo.com".to_string()),
            },
            Contributor {
                id: "Bar".to_string(),
                name: "Bar".to_string(),
                license: None,
                website: None,
            },
        ]);
    }

    #[test]
    fn datasets_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            Dataset {
                id: "Foo:0".to_string(),
                contributor_id: "Foo".to_string(),
                start_date: chrono::NaiveDate::from_ymd(2018, 1, 30),
                end_date: chrono::NaiveDate::from_ymd(2018, 1, 31),
                dataset_type: Some(DatasetType::Theorical),
                extrapolation: false,
                desc: Some("description".to_string()),
                system: Some("GTFS V2".to_string()),
            },
            Dataset {
                id: "Bar:0".to_string(),
                contributor_id: "Bar".to_string(),
                start_date: chrono::NaiveDate::from_ymd(2018, 1, 30),
                end_date: chrono::NaiveDate::from_ymd(2018, 1, 31),
                dataset_type: None,
                extrapolation: false,
                desc: None,
                system: None,
            },
        ]);
    }

    #[test]
    fn equipments_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![Equipment {
            id: "1".to_string(),
            wheelchair_boarding: Availability::Available,
            sheltered: Availability::InformationNotAvailable,
            elevator: Availability::Available,
            escalator: Availability::Available,
            bike_accepted: Availability::Available,
            bike_depot: Availability::Available,
            visual_announcement: Availability::Available,
            audible_announcement: Availability::Available,
            appropriate_escort: Availability::Available,
            appropriate_signage: Availability::Available,
        }]);
    }

    #[test]
    fn transfers_serialization_deserialization() {
        test_serialize_deserialize_collection(vec![
            Transfer {
                from_stop_id: "st_1".to_string(),
                to_stop_id: "st_1".to_string(),
                min_transfer_time: Some(20),
                real_min_transfer_time: Some(30),
                equipment_id: Some("eq_1".to_string()),
            },
            Transfer {
                from_stop_id: "st_1".to_string(),
                to_stop_id: "st_2".to_string(),
                min_transfer_time: None,
                real_min_transfer_time: None,
                equipment_id: Some("eq_1".to_string()),
            },
        ]);
    }

    #[test]
    fn calendar_serialization_deserialization() {
        let mut dates1 = ::std::collections::BTreeSet::new();
        dates1.insert(chrono::NaiveDate::from_ymd(2018, 5, 5));
        dates1.insert(chrono::NaiveDate::from_ymd(2018, 5, 6));

        let mut dates2 = ::std::collections::BTreeSet::new();
        dates2.insert(chrono::NaiveDate::from_ymd(2018, 6, 1));

        let calendars = CollectionWithId::new(vec![
            Calendar {
                id: "0".to_string(),
                dates: dates1,
            },
            Calendar {
                id: "1".to_string(),
                dates: dates2,
            },
        ])
        .unwrap();

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            write_calendar_dates(path, &calendars).unwrap();

            let mut collections = Collections::default();
            manage_calendars(&mut handler, &mut collections).unwrap();

            assert_eq!(calendars, collections.calendars);
        });
    }

    #[test]
    fn stops_serialization_deserialization() {
        let stop_points = CollectionWithId::new(vec![
            StopPoint {
                id: "sp_1".to_string(),
                name: "sp_name_1".to_string(),
                visible: true,
                coord: Coord {
                    lon: 2.073_034,
                    lat: 48.799_115,
                },
                timezone: Some("Europe/Paris".to_string()),
                geometry_id: Some("geometry_1".to_string()),
                equipment_id: Some("equipment_1".to_string()),
                stop_area_id: "sa_1".to_string(),
                fare_zone_id: Some("1".to_string()),
                stop_type: StopType::Point,
                ..Default::default()
            },
            // stop point with no parent station
            StopPoint {
                id: "sa_2".to_string(),
                name: "sa_name_2".to_string(),
                visible: true,
                coord: Coord {
                    lon: 2.173_034,
                    lat: 47.899_115,
                },
                stop_area_id: "Navitia:sa_2".to_string(),
                stop_type: StopType::Point,
                ..Default::default()
            },
        ])
        .unwrap();

        let stop_areas = CollectionWithId::new(vec![
            StopArea {
                id: "Navitia:sa_2".to_string(),
                name: "sa_name_2".to_string(),
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                visible: true,
                coord: Coord {
                    lon: 2.173_034,
                    lat: 47.899_115,
                },
                timezone: None,
                geometry_id: None,
                equipment_id: None,
                level_id: None,
            },
            StopArea {
                id: "sa_1".to_string(),
                name: "sa_name_1".to_string(),
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                visible: true,
                coord: Coord {
                    lon: 2.073_034,
                    lat: 48.799_115,
                },
                timezone: Some("Europe/Paris".to_string()),
                geometry_id: Some("geometry_3".to_string()),
                equipment_id: Some("equipment_1".to_string()),
                level_id: Some("level2".to_string()),
            },
        ])
        .unwrap();

        let stop_locations: CollectionWithId<StopLocation> = CollectionWithId::default();

        test_in_tmp_dir(|path| {
            write::write_stops(path, &stop_points, &stop_areas, &stop_locations).unwrap();

            let mut collections = Collections::default();
            read::manage_stops(&mut collections, path).unwrap();

            assert_eq!(stop_points, collections.stop_points);
            assert_eq!(stop_areas, collections.stop_areas);
        });
    }

    #[test]
    fn comments_codes_object_properties_serialization_deserialization() {
        let mut ser_collections = Collections::default();
        let comments = CollectionWithId::new(vec![
            Comment {
                id: "c:1".to_string(),
                comment_type: CommentType::Information,
                label: Some("label:".to_string()),
                name: "value:1".to_string(),
                url: Some("http://www.foo.bar".to_string()),
            },
            Comment {
                id: "c:2".to_string(),
                comment_type: CommentType::OnDemandTransport,
                label: Some("label:2".to_string()),
                name: "value:3".to_string(),
                url: Some("http://www.foo.bar".to_string()),
            },
            Comment {
                id: "c:3".to_string(),
                comment_type: CommentType::Information,
                label: None,
                name: "value:1".to_string(),
                url: None,
            },
        ])
        .unwrap();

        let stop_points = CollectionWithId::from(StopPoint {
            id: "sp_1".to_string(),
            name: "sp_name_1".to_string(),
            codes: btree_set_from_vec(vec![(
                "object_system:1".to_string(),
                "object_code:1".to_string(),
            )]),
            object_properties: btree_set_from_vec(vec![(
                "prop_name:1".to_string(),
                "prop_value:1".to_string(),
            )]),
            comment_links: btree_set_from_vec(vec![comments.get_idx("c:1").unwrap()]),
            visible: true,
            coord: Coord {
                lon: 2.073_034,
                lat: 48.799_115,
            },
            stop_area_id: "sa_1".to_string(),
            stop_type: StopType::Point,
            ..Default::default()
        });

        let stop_areas = CollectionWithId::from(StopArea {
            id: "sa_1".to_string(),
            name: "sa_name_1".to_string(),
            codes: btree_set_from_vec(vec![(
                "object_system:2".to_string(),
                "object_code:2".to_string(),
            )]),
            object_properties: btree_set_from_vec(vec![(
                "prop_name:2".to_string(),
                "prop_value:2".to_string(),
            )]),
            comment_links: btree_set_from_vec(vec![comments.get_idx("c:2").unwrap()]),
            visible: true,
            coord: Coord {
                lon: 2.073_034,
                lat: 48.799_115,
            },
            timezone: None,
            geometry_id: None,
            equipment_id: None,
            level_id: Some("level1".to_string()),
        });

        let stop_locations: CollectionWithId<StopLocation> = CollectionWithId::default();

        let lines = CollectionWithId::from(Line {
            id: "OIF:002002003:3OIF829".to_string(),
            name: "3".to_string(),
            code: None,
            codes: btree_set_from_vec(vec![(
                "object_system:3".to_string(),
                "object_code:3".to_string(),
            )]),
            object_properties: btree_set_from_vec(vec![(
                "prop_name:3".to_string(),
                "prop_value:3".to_string(),
            )]),
            comment_links: btree_set_from_vec(vec![
                comments.get_idx("c:1").unwrap(),
                comments.get_idx("c:2").unwrap(),
            ]),
            forward_name: None,
            forward_direction: None,
            backward_name: None,
            backward_direction: None,
            color: None,
            text_color: None,
            sort_order: None,
            network_id: "OIF:829".to_string(),
            commercial_mode_id: "bus".to_string(),
            geometry_id: None,
            opening_time: None,
            closing_time: None,
        });

        let routes = CollectionWithId::from(Route {
            id: "OIF:002002002:CEN".to_string(),
            name: "Hôtels - Hôtels".to_string(),
            direction_type: None,
            codes: btree_set_from_vec(vec![
                ("object_system:4".to_string(), "object_code:4".to_string()),
                ("object_system:5".to_string(), "object_code:5".to_string()),
            ]),
            object_properties: btree_set_from_vec(vec![(
                "prop_name:4".to_string(),
                "prop_value:4".to_string(),
            )]),
            comment_links: btree_set_from_vec(vec![comments.get_idx("c:3").unwrap()]),
            line_id: "OIF:002002002:BDEOIF829".to_string(),
            geometry_id: None,
            destination_id: None,
        });

        let vehicle_journeys = CollectionWithId::from(VehicleJourney {
            id: "VJ:1".to_string(),
            codes: btree_set_from_vec(vec![(
                "object_system:6".to_string(),
                "object_code:6".to_string(),
            )]),
            object_properties: btree_set_from_vec(vec![(
                "prop_name:6".to_string(),
                "prop_value:6".to_string(),
            )]),
            comment_links: CommentLinksT::default(),
            route_id: "OIF:800:TER".to_string(),
            physical_mode_id: "Bus".to_string(),
            dataset_id: "OIF:0".to_string(),
            service_id: "2".to_string(),
            headsign: None,
            short_name: Some("42".to_string()),
            block_id: None,
            company_id: "OIF:743".to_string(),
            trip_property_id: None,
            geometry_id: None,
            stop_times: vec![StopTime {
                id: None,
                stop_point_idx: stop_points.get_idx("sp_1").unwrap(),
                sequence: 0,
                headsign: None,
                arrival_time: Time::new(9, 0, 0),
                departure_time: Time::new(9, 2, 0),
                boarding_duration: 2,
                alighting_duration: 3,
                pickup_type: 1,
                drop_off_type: 2,
                datetime_estimated: false,
                local_zone_id: None,
                precision: None,
                comment_links: None,
            }],
            journey_pattern_id: None,
        });

        let networks = CollectionWithId::from(Network {
            id: "OIF:102".to_string(),
            name: "SAVAC".to_string(),
            url: None,
            timezone: None,
            lang: None,
            phone: None,
            address: None,
            sort_order: None,
            codes: KeysValues::default(),
        });

        let mut stop_time_ids = HashMap::new();
        stop_time_ids.insert((("VJ:1").to_string(), 0), "StopTime:VJ:1:0".to_string());
        let mut stop_time_comments = HashMap::new();
        stop_time_comments.insert(("VJ:1".to_string(), 0), "c:2".to_string());

        ser_collections.comments = comments;
        ser_collections.stop_areas = stop_areas;
        ser_collections.stop_points = stop_points;
        ser_collections.stop_locations = stop_locations;
        ser_collections.lines = lines;
        ser_collections.routes = routes;
        ser_collections.vehicle_journeys = vehicle_journeys;
        ser_collections.networks = networks;
        ser_collections.stop_time_ids = stop_time_ids;
        ser_collections.stop_time_comments = stop_time_comments;

        test_in_tmp_dir(|path| {
            write_collection_with_id(path, "lines.txt", &ser_collections.lines).unwrap();
            write::write_stops(
                path,
                &ser_collections.stop_points,
                &ser_collections.stop_areas,
                &ser_collections.stop_locations,
            )
            .unwrap();
            write_collection_with_id(path, "routes.txt", &ser_collections.routes).unwrap();
            write_collection_with_id(path, "networks.txt", &ser_collections.networks).unwrap();
            write::write_vehicle_journeys_and_stop_times(
                path,
                &ser_collections.vehicle_journeys,
                &ser_collections.stop_points,
                &ser_collections.stop_time_headsigns,
                &ser_collections.stop_time_ids,
            )
            .unwrap();
            write::write_comments(path, &ser_collections).unwrap();
            write::write_codes(path, &ser_collections).unwrap();
            write::write_object_properties(path, &ser_collections).unwrap();

            let mut des_collections = Collections::default();
            des_collections.lines = make_collection_with_id(path, "lines.txt").unwrap();
            des_collections.routes = make_collection_with_id(path, "routes.txt").unwrap();
            des_collections.vehicle_journeys = make_collection_with_id(path, "trips.txt").unwrap();
            des_collections.networks = make_collection_with_id(path, "networks.txt").unwrap();
            read::manage_stops(&mut des_collections, path).unwrap();
            read::manage_stop_times(&mut des_collections, path).unwrap();
            read::manage_comments(&mut des_collections, path).unwrap();
            read::manage_codes(&mut des_collections, path).unwrap();
            read::manage_object_properties(&mut des_collections, path).unwrap();

            assert_eq!(ser_collections.comments, des_collections.comments);

            // test comment links
            assert_eq!(
                ser_collections
                    .lines
                    .get("OIF:002002003:3OIF829")
                    .unwrap()
                    .comment_links,
                des_collections
                    .lines
                    .get("OIF:002002003:3OIF829")
                    .unwrap()
                    .comment_links
            );

            assert_eq!(
                ser_collections
                    .stop_points
                    .get("sp_1")
                    .unwrap()
                    .comment_links,
                des_collections
                    .stop_points
                    .get("sp_1")
                    .unwrap()
                    .comment_links
            );

            assert_eq!(
                ser_collections
                    .stop_points
                    .get("sp_1")
                    .unwrap()
                    .comment_links,
                des_collections
                    .stop_points
                    .get("sp_1")
                    .unwrap()
                    .comment_links
            );

            assert_eq!(
                ser_collections
                    .stop_areas
                    .get("sa_1")
                    .unwrap()
                    .comment_links,
                des_collections
                    .stop_areas
                    .get("sa_1")
                    .unwrap()
                    .comment_links
            );

            assert_eq!(
                ser_collections
                    .routes
                    .get("OIF:002002002:CEN")
                    .unwrap()
                    .comment_links,
                des_collections
                    .routes
                    .get("OIF:002002002:CEN")
                    .unwrap()
                    .comment_links
            );

            assert_eq!(
                ser_collections
                    .vehicle_journeys
                    .get("VJ:1")
                    .unwrap()
                    .comment_links,
                des_collections
                    .vehicle_journeys
                    .get("VJ:1")
                    .unwrap()
                    .comment_links
            );

            assert_eq!(
                ser_collections.stop_time_comments,
                des_collections.stop_time_comments
            );

            // test codes
            assert_eq!(
                ser_collections
                    .lines
                    .get("OIF:002002003:3OIF829")
                    .unwrap()
                    .codes,
                des_collections
                    .lines
                    .get("OIF:002002003:3OIF829")
                    .unwrap()
                    .codes
            );

            assert_eq!(
                ser_collections.stop_points.get("sp_1").unwrap().codes,
                des_collections.stop_points.get("sp_1").unwrap().codes
            );

            assert_eq!(
                ser_collections.stop_points.get("sp_1").unwrap().codes,
                des_collections.stop_points.get("sp_1").unwrap().codes
            );

            assert_eq!(
                ser_collections.stop_areas.get("sa_1").unwrap().codes,
                des_collections.stop_areas.get("sa_1").unwrap().codes
            );

            assert_eq!(
                ser_collections
                    .routes
                    .get("OIF:002002002:CEN")
                    .unwrap()
                    .codes,
                des_collections
                    .routes
                    .get("OIF:002002002:CEN")
                    .unwrap()
                    .codes
            );

            assert_eq!(
                ser_collections.vehicle_journeys.get("VJ:1").unwrap().codes,
                des_collections.vehicle_journeys.get("VJ:1").unwrap().codes
            );

            assert_eq!(
                ser_collections.networks.get("OIF:102").unwrap().codes,
                des_collections.networks.get("OIF:102").unwrap().codes
            );
        });
    }

    #[test]
    fn trip_properties_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            TripProperty {
                id: "1".to_string(),
                wheelchair_accessible: Availability::Available,
                bike_accepted: Availability::NotAvailable,
                air_conditioned: Availability::InformationNotAvailable,
                visual_announcement: Availability::Available,
                audible_announcement: Availability::Available,
                appropriate_escort: Availability::Available,
                appropriate_signage: Availability::Available,
                school_vehicle_type: TransportType::Regular,
            },
            TripProperty {
                id: "2".to_string(),
                wheelchair_accessible: Availability::Available,
                bike_accepted: Availability::NotAvailable,
                air_conditioned: Availability::InformationNotAvailable,
                visual_announcement: Availability::Available,
                audible_announcement: Availability::Available,
                appropriate_escort: Availability::Available,
                appropriate_signage: Availability::Available,
                school_vehicle_type: TransportType::RegularAndSchool,
            },
        ]);
    }

    #[test]
    fn geometries_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            Geometry {
                id: "geo-id-1".to_string(),
                geometry:
                    line_string![(x: 2.541_951, y: 49.013_402), (x: 2.571_294, y: 49.004_725)]
                        .into(),
            },
            Geometry {
                id: "geo-id-2".to_string(),
                geometry:
                    line_string![(x: 2.548_309, y: 49.009_182), (x: 2.549_309, y: 49.009_253)]
                        .into(),
            },
        ]);
    }

    #[test]
    fn admin_stations_serialization_deserialization() {
        test_serialize_deserialize_collection(vec![
            AdminStation {
                admin_id: "admin:1".to_string(),
                admin_name: "Paris 12".to_string(),
                stop_id: "OIF:SA:8768600".to_string(),
            },
            AdminStation {
                admin_id: "admin:1".to_string(),
                admin_name: "Paris 12".to_string(),
                stop_id: "OIF:SA:8768666".to_string(),
            },
            AdminStation {
                admin_id: "admin:2".to_string(),
                admin_name: "Paris Nord".to_string(),
                stop_id: "OIF:SA:8727100".to_string(),
            },
        ]);
    }

    #[test]
    fn prices_v1_serialization_deserialization() {
        test_serialize_deserialize_collection(vec![
            PriceV1 {
                id: "PV1-01".to_string(),
                start_date: chrono::NaiveDate::from_ymd(2019, 1, 1),
                end_date: chrono::NaiveDate::from_ymd(2019, 12, 31),
                price: 190,
                name: "Ticket PV1-01".to_string(),
                ignored: "".to_string(),
                comment: "Comment on PV1-01".to_string(),
                currency_type: Some("centime".to_string()),
            },
            PriceV1 {
                id: "PV1-02".to_string(),
                start_date: chrono::NaiveDate::from_ymd(2019, 1, 1),
                end_date: chrono::NaiveDate::from_ymd(2019, 12, 31),
                price: 280,
                name: "Ticket PV1-02".to_string(),
                ignored: "".to_string(),
                comment: "".to_string(),
                currency_type: None,
            },
        ]);
    }

    #[test]
    fn od_fares_v1_serialization_deserialization() {
        test_serialize_deserialize_collection(vec![
            ODFareV1 {
                origin_stop_area_id: "stop_area:0:SA:8727114".to_string(),
                origin_name: Some("EPINAY-S/SEINE".to_string()),
                origin_mode: "stop".to_string(),
                destination_stop_area_id: "stop_area:0:SA:8727116".to_string(),
                destination_name: Some("PIERREFITTE-ST.".to_string()),
                destination_mode: "stop".to_string(),
                ticket_id: "29".to_string(),
            },
            ODFareV1 {
                origin_stop_area_id: "stop_area:0:SA:8773006".to_string(),
                origin_name: None,
                origin_mode: "zone".to_string(),
                destination_stop_area_id: "stop_area:0:SA:8775812".to_string(),
                destination_name: None,
                destination_mode: "stop".to_string(),
                ticket_id: "99-93".to_string(),
            },
        ]);
    }

    #[test]
    fn fares_v1_serialization_deserialization() {
        test_serialize_deserialize_collection(vec![
            FareV1 {
                before_change: "*".to_string(),
                after_change: "mode=physical_mode:Bus".to_string(),
                start_trip: "duration<90".to_string(),
                end_trip: "".to_string(),
                global_condition: "".to_string(),
                ticket_id: "".to_string(),
            },
            FareV1 {
                before_change: "*".to_string(),
                after_change: "network=network:0:56".to_string(),
                start_trip: "zone=1".to_string(),
                end_trip: "zone=1".to_string(),
                global_condition: "exclusive".to_string(),
                ticket_id: "tickett".to_string(),
            },
        ]);
    }

    #[test]
    fn tickets_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            Ticket {
                id: "PF1:Ticket1".to_string(),
                name: "Ticket name 1".to_string(),
                comment: Some("Some comment on ticket".to_string()),
            },
            Ticket {
                id: "PF2:Ticket2".to_string(),
                name: "Ticket name 1".to_string(),
                comment: None,
            },
        ]);
    }

    #[test]
    fn ticket_uses_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            TicketUse {
                id: "PF1:TicketUse1".to_string(),
                ticket_id: "PF1:Ticket1".to_string(),
                max_transfers: Some(1),
                boarding_time_limit: Some(60),
                alighting_time_limit: Some(60),
            },
            TicketUse {
                id: "PF2:TicketUse2".to_string(),
                ticket_id: "PF2:Ticket2".to_string(),
                max_transfers: None,
                boarding_time_limit: None,
                alighting_time_limit: None,
            },
        ]);
    }

    #[test]
    fn ticket_prices_serialization_deserialization() {
        use rust_decimal_macros::dec;
        test_serialize_deserialize_collection(vec![
            TicketPrice {
                ticket_id: "PF1:Ticket1".to_string(),
                price: dec!(150.0),
                currency: "EUR".to_string(),
                ticket_validity_start: chrono::NaiveDate::from_ymd(2019, 1, 1),
                ticket_validity_end: chrono::NaiveDate::from_ymd(2019, 12, 31),
            },
            TicketPrice {
                ticket_id: "PF2:Ticket2".to_string(),
                price: dec!(900.0),
                currency: "GHS".to_string(),
                ticket_validity_start: chrono::NaiveDate::from_ymd(2019, 1, 1),
                ticket_validity_end: chrono::NaiveDate::from_ymd(2019, 12, 31),
            },
        ]);
    }

    #[test]
    fn ticket_use_perimeters_serialization_deserialization() {
        test_serialize_deserialize_collection(vec![
            TicketUsePerimeter {
                ticket_use_id: "PF1:TicketUse1".to_string(),
                object_type: ObjectType::Network,
                object_id: "PF1:Network1".to_string(),
                perimeter_action: PerimeterAction::Included,
            },
            TicketUsePerimeter {
                ticket_use_id: "PF1:TicketUse1".to_string(),
                object_type: ObjectType::Line,
                object_id: "PF2:Line2".to_string(),
                perimeter_action: PerimeterAction::Excluded,
            },
        ]);
    }

    #[test]
    fn ticket_use_restrictions_serialization_deserialization() {
        test_serialize_deserialize_collection(vec![
            TicketUseRestriction {
                ticket_use_id: "PF1:TicketUse1".to_string(),
                restriction_type: RestrictionType::OriginDestination,
                use_origin: "PF1:SA1".to_string(),
                use_destination: "PF1:SA2".to_string(),
            },
            TicketUseRestriction {
                ticket_use_id: "PF2:TicketUse2".to_string(),
                restriction_type: RestrictionType::Zone,
                use_origin: "PF2:ZO1".to_string(),
                use_destination: "PF2:ZO2".to_string(),
            },
        ]);
    }
}
