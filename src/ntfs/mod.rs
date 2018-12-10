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

//! [NTFS](https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/ntfs_fr.md)
//! format management.

mod read;
mod write;

use common_format;
use model::{Collections, Model};
use objects::*;
use std::path;
use utils::*;
use Result;
extern crate tempdir;
use self::tempdir::TempDir;
use read_utils::open_file;

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
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Stop {
    #[serde(rename = "stop_id")]
    id: String,
    #[serde(rename = "stop_name")]
    name: String,
    #[serde(
        default = "default_visible",
        deserialize_with = "de_from_u8",
        serialize_with = "ser_from_bool"
    )]
    visible: bool,
    fare_zone_id: Option<String>,
    #[serde(rename = "stop_lon")]
    lon: f64,
    #[serde(rename = "stop_lat")]
    lat: f64,
    #[serde(deserialize_with = "de_with_empty_default")]
    location_type: i32,
    parent_station: Option<String>,
    #[serde(rename = "stop_timezone")]
    timezone: Option<String>,
    geometry_id: Option<String>,
    equipment_id: Option<String>,
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

/// Imports a `Model` from the
/// [NTFS](https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/ntfs_fr.md)
/// files in the given directory.
pub fn read<P: AsRef<path::Path>>(path: P) -> Result<Model> {
    let path = path.as_ref();
    info!("Loading NTFS from {:?}", path);
    let mut collections = Collections::default();
    collections.contributors = make_collection_with_id(path, "contributors.txt")?;
    collections.datasets = make_collection_with_id(path, "datasets.txt")?;
    collections.commercial_modes = make_collection_with_id(path, "commercial_modes.txt")?;
    collections.networks = make_collection_with_id(path, "networks.txt")?;
    collections.lines = make_collection_with_id(path, "lines.txt")?;
    collections.routes = make_collection_with_id(path, "routes.txt")?;
    collections.vehicle_journeys = make_collection_with_id(path, "trips.txt")?;
    collections.physical_modes = make_collection_with_id(path, "physical_modes.txt")?;
    collections.companies = make_collection_with_id(path, "companies.txt")?;
    collections.equipments = make_opt_collection_with_id(path, "equipments.txt")?;
    collections.trip_properties = make_opt_collection_with_id(path, "trip_properties.txt")?;
    collections.transfers = make_opt_collection(path, "transfers.txt")?;
    collections.admin_stations = make_opt_collection(path, "admin_stations.txt")?;
    //TODO
    // common_format::manage_calendars(
    //     open_file(path, "calendar.txt").ok(),
    //     open_file(path, "calendar_dates.txt").ok(),
    //     &mut collections,
    // )?;
    read::manage_geometries(&mut collections, path)?;
    read::manage_feed_infos(&mut collections, path)?;
    read::manage_stops(&mut collections, path)?;
    read::manage_stop_times(&mut collections, path)?;
    read::manage_codes(&mut collections, path)?;
    read::manage_comments(&mut collections, path)?;
    read::manage_object_properties(&mut collections, path)?;
    info!("Indexing");
    let res = Model::new(collections)?;
    info!("Loading NTFS done");
    Ok(res)
}

/// Exports a `Model` to the
/// [NTFS](https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/ntfs_fr.md)
/// files in the given directory.
pub fn write<P: AsRef<path::Path>>(model: &Model, path: P) -> Result<()> {
    let path = path.as_ref();
    info!("Writing NTFS to {:?}", path);

    write::write_feed_infos(path, &model.feed_infos)?;
    write::write_collection_with_id(path, "contributors.txt", &model.contributors)?;
    write::write_collection_with_id(path, "datasets.txt", &model.datasets)?;
    write::write_collection_with_id(path, "networks.txt", &model.networks)?;
    write::write_collection_with_id(path, "commercial_modes.txt", &model.commercial_modes)?;
    write::write_collection_with_id(path, "companies.txt", &model.companies)?;
    write::write_collection_with_id(path, "lines.txt", &model.lines)?;
    write::write_collection_with_id(path, "physical_modes.txt", &model.physical_modes)?;
    write::write_collection_with_id(path, "equipments.txt", &model.equipments)?;
    write::write_collection_with_id(path, "routes.txt", &model.routes)?;
    write::write_collection_with_id(path, "trip_properties.txt", &model.trip_properties)?;
    write::write_collection_with_id(path, "geometries.txt", &model.geometries)?;
    write::write_collection(path, "transfers.txt", &model.transfers)?;
    write::write_collection(path, "admin_stations.txt", &model.admin_stations)?;
    write::write_vehicle_journeys_and_stop_times(
        path,
        &model.vehicle_journeys,
        &model.stop_points,
        &model.stop_time_headsigns,
        &model.stop_time_ids,
    )?;
    common_format::write_calendar_dates(path, &model.calendars)?;
    write::write_stops(path, &model.stop_points, &model.stop_areas)?;
    write::write_comments(path, model)?;
    write::write_codes(path, model)?;
    write::write_object_properties(path, model)?;

    Ok(())
}

/// Exports a `Model` to a
/// [NTFS](https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/ntfs_fr.md)
/// ZIP archive at the given full path.
pub fn write_to_zip<P: AsRef<path::Path>>(model: &Model, path: P) -> Result<()> {
    let path = path.as_ref();
    info!("Writing NTFS to ZIP File {:?}", path);
    let input_tmp_dir = TempDir::new("write_ntfs_for_zip")?;
    write(model, input_tmp_dir.path())?;
    zip_to(input_tmp_dir.path(), path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate tempdir;
    use super::Collections;
    use super::{read, write};
    use chrono;
    use collection::*;
    use collection::{Collection, CollectionWithId};
    use common_format;
    use geo_types::{Geometry as GeoGeometry, LineString, Point};
    use objects::*;
    use serde;
    use std::collections::{BTreeMap, BTreeSet, HashMap};
    use std::fmt::Debug;
    use std::fs::File;
    use test_utils::*;
    use utils::*;

    fn test_serialize_deserialize_collection_with_id<T>(objects: Vec<T>)
    where
        T: Id<T> + PartialEq + Debug + serde::Serialize,
        for<'de> T: serde::Deserialize<'de>,
    {
        let collection = CollectionWithId::new(objects).unwrap();
        test_in_tmp_dir(|path| {
            write::write_collection_with_id(path, "file.txt", &collection).unwrap();
            let des_collection = make_collection_with_id(path, "file.txt").unwrap();
            assert_eq!(des_collection, collection);
        });
    }

    fn test_serialize_deserialize_collection<T>(objects: Vec<T>)
    where
        T: PartialEq + Debug + serde::Serialize,
        for<'de> T: serde::Deserialize<'de>,
    {
        let collection = Collection::new(objects);
        test_in_tmp_dir(|path| {
            write::write_collection(path, "file.txt", &collection).unwrap();
            let des_collection = make_opt_collection(path, "file.txt").unwrap();
            assert_eq!(des_collection, collection);
        });
    }

    fn btree_set_from_vec<T: Ord>(input: Vec<T>) -> BTreeSet<T> {
        input.into_iter().collect()
    }

    #[test]
    fn feed_infos_serialization_deserialization() {
        let mut feed_infos = BTreeMap::default();
        feed_infos.insert("feed_license".to_string(), "".to_string());
        feed_infos.insert("ntfs_version".to_string(), "0.3".to_string());
        feed_infos.insert("feed_creation_date".to_string(), "20181004".to_string());
        let mut collections = Collections::default();

        test_in_tmp_dir(|path| {
            write::write_feed_infos(path, &feed_infos).unwrap();
            read::manage_feed_infos(&mut collections, path).unwrap();
            let info_params: Vec<_> = collections.feed_infos.keys().collect();
            // test that feed infos are ordered by info_param
            assert_eq!(
                info_params,
                ["feed_creation_date", "feed_license", "ntfs_version"]
            );
        });
        assert_eq!(collections.feed_infos.len(), 3);
        assert_eq!(collections.feed_infos, feed_infos);
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
                forward_direction: Some("OIF:SA:4:126".to_string()),
                backward_name: Some("Hôtels - Hôtels".to_string()),
                backward_direction: Some("OIF:SA:4:126".to_string()),
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
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                visible: true,
                coord: Coord {
                    lon: 2.073034,
                    lat: 48.799115,
                },
                stop_area_id: "OIF:SA:8739322".to_string(),
                timezone: Some("Europe/Paris".to_string()),
                geometry_id: None,
                equipment_id: None,
                fare_zone_id: Some("1".to_string()),
                stop_type: StopType::Point,
            },
            StopPoint {
                id: "OIF:SP:36:2127".to_string(),
                name: "Division Leclerc".to_string(),
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                visible: true,
                coord: Coord {
                    lon: 2.073407,
                    lat: 48.800598,
                },
                stop_area_id: "OIF:SA:2:1468".to_string(),
                timezone: Some("Europe/Paris".to_string()),
                geometry_id: None,
                equipment_id: None,
                fare_zone_id: None,
                stop_type: StopType::Point,
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
                block_id: Some("PLOI".to_string()),
                company_id: "OIF:743".to_string(),
                trip_property_id: Some("0".to_string()),
                geometry_id: Some("Geometry:Line:Relation:6883353".to_string()),
                stop_times: vec![
                    StopTime {
                        stop_point_idx: stop_points.get_idx("OIF:SP:36:2085").unwrap(),
                        sequence: 0,
                        arrival_time: Time::new(14, 40, 0),
                        departure_time: Time::new(14, 40, 0),
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 0,
                        drop_off_type: 1,
                        datetime_estimated: false,
                        local_zone_id: None,
                    },
                    StopTime {
                        stop_point_idx: stop_points.get_idx("OIF:SP:36:2127").unwrap(),
                        sequence: 1,
                        arrival_time: Time::new(14, 42, 0),
                        departure_time: Time::new(14, 42, 0),
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 0,
                        drop_off_type: 0,
                        datetime_estimated: false,
                        local_zone_id: None,
                    },
                ],
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
                block_id: None,
                company_id: "OIF:743".to_string(),
                trip_property_id: None,
                geometry_id: None,
                stop_times: vec![],
            },
        ])
        .unwrap();

        let mut headsigns = HashMap::new();
        headsigns.insert(
            (
                vehicle_journeys.get_idx("OIF:87604986-1_11595-1").unwrap(),
                1,
            ),
            "somewhere".to_string(),
        );
        let mut stop_time_ids = HashMap::new();
        stop_time_ids.insert(
            (
                vehicle_journeys.get_idx("OIF:87604986-1_11595-1").unwrap(),
                0,
            ),
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
            assert_eq!(collections.vehicle_journeys, vehicle_journeys);
            assert_eq!(headsigns, collections.stop_time_headsigns);
            assert_eq!(stop_time_ids, collections.stop_time_ids);
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
                start_date: chrono::NaiveDate::from_ymd(2018, 01, 30),
                end_date: chrono::NaiveDate::from_ymd(2018, 01, 31),
                dataset_type: Some(DatasetType::Theorical),
                extrapolation: false,
                desc: Some("description".to_string()),
                system: Some("GTFS V2".to_string()),
            },
            Dataset {
                id: "Bar:0".to_string(),
                contributor_id: "Bar".to_string(),
                start_date: chrono::NaiveDate::from_ymd(2018, 01, 30),
                end_date: chrono::NaiveDate::from_ymd(2018, 01, 31),
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
            wheelchair_boarding: common_format::Availability::Available,
            sheltered: common_format::Availability::InformationNotAvailable,
            elevator: common_format::Availability::Available,
            escalator: common_format::Availability::Available,
            bike_accepted: common_format::Availability::Available,
            bike_depot: common_format::Availability::Available,
            visual_announcement: common_format::Availability::Available,
            audible_announcement: common_format::Availability::Available,
            appropriate_escort: common_format::Availability::Available,
            appropriate_signage: common_format::Availability::Available,
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
            common_format::write_calendar_dates(path, &calendars).unwrap();

            let calendar_file = File::open(path.join("calendar_dates.txt")).unwrap();
            let mut collections = Collections::default();
            common_format::manage_calendars(None::<File>, Some(calendar_file), &mut collections)
                .unwrap();

            assert_eq!(collections.calendars, calendars);
        });
    }

    #[test]
    fn stops_serialization_deserialization() {
        let stop_points = CollectionWithId::new(vec![
            StopPoint {
                id: "sp_1".to_string(),
                name: "sp_name_1".to_string(),
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                visible: true,
                coord: Coord {
                    lon: 2.073034,
                    lat: 48.799115,
                },
                timezone: Some("Europe/Paris".to_string()),
                geometry_id: Some("geometry_1".to_string()),
                equipment_id: Some("equipment_1".to_string()),
                stop_area_id: "sa_1".to_string(),
                fare_zone_id: Some("1".to_string()),
                stop_type: StopType::Point,
            },
            // stop point with no parent station
            StopPoint {
                id: "sa_2".to_string(),
                name: "sa_name_2".to_string(),
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                visible: true,
                coord: Coord {
                    lon: 2.173034,
                    lat: 47.899115,
                },
                timezone: None,
                geometry_id: None,
                equipment_id: None,
                stop_area_id: "Navitia:sa_2".to_string(),
                fare_zone_id: None,
                stop_type: StopType::Point,
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
                    lon: 2.173034,
                    lat: 47.899115,
                },
                timezone: None,
                geometry_id: None,
                equipment_id: None,
            },
            StopArea {
                id: "sa_1".to_string(),
                name: "sa_name_1".to_string(),
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                visible: true,
                coord: Coord {
                    lon: 2.073034,
                    lat: 48.799115,
                },
                timezone: Some("Europe/Paris".to_string()),
                geometry_id: Some("geometry_3".to_string()),
                equipment_id: Some("equipment_1".to_string()),
            },
        ])
        .unwrap();

        test_in_tmp_dir(|path| {
            write::write_stops(path, &stop_points, &stop_areas).unwrap();

            let mut collections = Collections::default();
            read::manage_stops(&mut collections, path).unwrap();

            assert_eq!(collections.stop_points, stop_points);
            assert_eq!(collections.stop_areas, stop_areas);
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

        let stop_points = CollectionWithId::new(vec![StopPoint {
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
                lon: 2.073034,
                lat: 48.799115,
            },
            timezone: None,
            geometry_id: None,
            equipment_id: None,
            stop_area_id: "sa_1".to_string(),
            fare_zone_id: None,
            stop_type: StopType::Point,
        }])
        .unwrap();

        let stop_areas = CollectionWithId::new(vec![StopArea {
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
                lon: 2.073034,
                lat: 48.799115,
            },
            timezone: None,
            geometry_id: None,
            equipment_id: None,
        }])
        .unwrap();

        let lines = CollectionWithId::new(vec![Line {
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
        }])
        .unwrap();

        let routes = CollectionWithId::new(vec![Route {
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
        }])
        .unwrap();

        let vehicle_journeys = CollectionWithId::new(vec![VehicleJourney {
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
            block_id: None,
            company_id: "OIF:743".to_string(),
            trip_property_id: None,
            geometry_id: None,
            stop_times: vec![StopTime {
                stop_point_idx: stop_points.get_idx("sp_1").unwrap(),
                sequence: 0,
                arrival_time: Time::new(9, 0, 0),
                departure_time: Time::new(9, 2, 0),
                boarding_duration: 2,
                alighting_duration: 3,
                pickup_type: 1,
                drop_off_type: 2,
                datetime_estimated: false,
                local_zone_id: None,
            }],
        }])
        .unwrap();

        let networks = CollectionWithId::new(vec![Network {
            id: "OIF:102".to_string(),
            name: "SAVAC".to_string(),
            url: None,
            timezone: None,
            lang: None,
            phone: None,
            address: None,
            sort_order: None,
            codes: KeysValues::default(),
        }])
        .unwrap();

        let mut stop_time_ids = HashMap::new();
        stop_time_ids.insert(
            (vehicle_journeys.get_idx("VJ:1").unwrap(), 0),
            "StopTime:VJ:1:0".to_string(),
        );
        let mut stop_time_comments = HashMap::new();
        stop_time_comments.insert(
            (vehicle_journeys.get_idx("VJ:1").unwrap(), 0),
            comments.get_idx("c:2").unwrap(),
        );

        ser_collections.comments = comments;
        ser_collections.stop_areas = stop_areas;
        ser_collections.stop_points = stop_points;
        ser_collections.lines = lines;
        ser_collections.routes = routes;
        ser_collections.vehicle_journeys = vehicle_journeys;
        ser_collections.networks = networks;
        ser_collections.stop_time_ids = stop_time_ids;
        ser_collections.stop_time_comments = stop_time_comments;

        test_in_tmp_dir(|path| {
            write::write_collection_with_id(path, "lines.txt", &ser_collections.lines).unwrap();
            write::write_stops(
                path,
                &ser_collections.stop_points,
                &ser_collections.stop_areas,
            )
            .unwrap();
            write::write_collection_with_id(path, "routes.txt", &ser_collections.routes).unwrap();
            write::write_collection_with_id(path, "networks.txt", &ser_collections.networks)
                .unwrap();
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
                wheelchair_accessible: common_format::Availability::Available,
                bike_accepted: common_format::Availability::NotAvailable,
                air_conditioned: common_format::Availability::InformationNotAvailable,
                visual_announcement: common_format::Availability::Available,
                audible_announcement: common_format::Availability::Available,
                appropriate_escort: common_format::Availability::Available,
                appropriate_signage: common_format::Availability::Available,
                school_vehicle_type: TransportType::Regular,
            },
            TripProperty {
                id: "2".to_string(),
                wheelchair_accessible: common_format::Availability::Available,
                bike_accepted: common_format::Availability::NotAvailable,
                air_conditioned: common_format::Availability::InformationNotAvailable,
                visual_announcement: common_format::Availability::Available,
                audible_announcement: common_format::Availability::Available,
                appropriate_escort: common_format::Availability::Available,
                appropriate_signage: common_format::Availability::Available,
                school_vehicle_type: TransportType::RegularAndSchool,
            },
        ]);
    }

    #[test]
    fn geometries_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            Geometry {
                id: "geo-id-1".to_string(),
                geometry: GeoGeometry::LineString(LineString(vec![
                    Point::new(2.541951, 49.013402),
                    Point::new(2.571294, 49.004725),
                ])),
            },
            Geometry {
                id: "geo-id-2".to_string(),
                geometry: GeoGeometry::LineString(LineString(vec![
                    Point::new(2.548309, 49.009182),
                    Point::new(2.549309, 49.009253),
                ])),
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
}
