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

mod read;
mod write;

use std::path;
use {Collections, PtObjects};
use utils::*;
use objects::*;
use Result;

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

#[derive(Serialize, Deserialize, Debug)]
struct CalendarDate {
    service_id: String,
    #[serde(deserialize_with = "de_from_date_string", serialize_with = "ser_from_naive_date")]
    date: Date,
    exception_type: ExceptionType,
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

pub fn read<P: AsRef<path::Path>>(path: P) -> Result<PtObjects> {
    let path = path.as_ref();
    info!("Loading NTFS from {:?}", path);
    let mut collections = Collections::default();
    collections.contributors = read::make_collection_with_id(path, "contributors.txt")?;
    collections.datasets = read::make_collection_with_id(path, "datasets.txt")?;
    collections.commercial_modes = read::make_collection_with_id(path, "commercial_modes.txt")?;
    collections.networks = read::make_collection_with_id(path, "networks.txt")?;
    collections.lines = read::make_collection_with_id(path, "lines.txt")?;
    collections.routes = read::make_collection_with_id(path, "routes.txt")?;
    collections.vehicle_journeys = read::make_collection_with_id(path, "trips.txt")?;
    collections.physical_modes = read::make_collection_with_id(path, "physical_modes.txt")?;
    collections.companies = read::make_collection_with_id(path, "companies.txt")?;
    collections.equipments = read::make_opt_collection_with_id(path, "equipments.txt")?;
    collections.trip_properties = read::make_opt_collection_with_id(path, "trip_properties.txt")?;
    collections.geometries = read::make_opt_collection_with_id(path, "geometries.txt")?;
    collections.transfers = read::make_opt_collection(path, "transfers.txt")?;
    collections.admin_stations = read::make_opt_collection(path, "admin_stations.txt")?;
    read::manage_calendars(&mut collections, path)?;
    read::manage_feed_infos(&mut collections, path)?;
    read::manage_stops(&mut collections, path)?;
    read::manage_stop_times(&mut collections, path)?;
    read::manage_codes(&mut collections, path)?;
    read::manage_comments(&mut collections, path)?;
    read::manage_object_properties(&mut collections, path)?;
    info!("Indexing");
    let res = PtObjects::new(collections)?;
    info!("Loading NTFS done");
    Ok(res)
}

pub fn write<P: AsRef<path::Path>>(path: P, pt_objects: &PtObjects) {
    let path = path.as_ref();
    info!("Writing NTFS to {:?}", path);

    write::write_feed_infos(path, &pt_objects.feed_infos);
    write::write_collection_with_id(path, "contributors.txt", &pt_objects.contributors);
    write::write_collection_with_id(path, "datasets.txt", &pt_objects.datasets);
    write::write_collection_with_id(path, "networks.txt", &pt_objects.networks);
    write::write_collection_with_id(path, "commercial_modes.txt", &pt_objects.networks);
    write::write_collection_with_id(path, "companies.txt", &pt_objects.companies);
    write::write_collection_with_id(path, "lines.txt", &pt_objects.lines);
    write::write_collection_with_id(path, "physical_modes.txt", &pt_objects.lines);
    write::write_collection_with_id(path, "equipments.txt", &pt_objects.equipments);
    write::write_collection_with_id(path, "routes.txt", &pt_objects.lines);
    write::write_collection_with_id(path, "trip_properties.txt", &pt_objects.trip_properties);
    write::write_collection_with_id(path, "geometries.txt", &pt_objects.geometries);
    write::write_collection(path, "transfers.txt", &pt_objects.transfers);
    write::write_collection(path, "admin_stations.txt", &pt_objects.admin_stations);
    write::write_vehicle_journeys_and_stop_times(
        path,
        &pt_objects.vehicle_journeys,
        &pt_objects.stop_points,
    );
    write::write_calendar_and_calendar_dates(path, &pt_objects.calendars);
    write::write_stops(path, &pt_objects.stop_points, &pt_objects.stop_areas);
    write::write_comments(path, pt_objects);
    write::write_codes(path, pt_objects);
    write::write_object_properties(path, pt_objects);
}

#[cfg(test)]
mod tests {
    extern crate tempdir;
    use self::tempdir::TempDir;
    use objects::*;
    use {Collection, CollectionWithId};
    use super::{read, write};
    use super::Collections;
    use collection::Id;
    use std::collections::HashMap;
    use serde;
    use std::fmt::Debug;
    use chrono;
    use std::path;

    fn ser_deser_in_tmp_dir<F>(func: F)
    where
        F: FnOnce(&path::Path),
    {
        let tmp_dir = TempDir::new("navitia_model_tests").expect("create temp dir");
        {
            let path = tmp_dir.as_ref();
            func(path);
        }
        tmp_dir.close().expect("delete temp dir");
    }

    fn test_serialize_deserialize_collection_with_id<T>(objects: Vec<T>)
    where
        T: Id<T> + PartialEq + Debug + serde::Serialize,
        for<'de> T: serde::Deserialize<'de>,
    {
        let collection = CollectionWithId::new(objects);
        ser_deser_in_tmp_dir(|path| {
            write::write_collection_with_id(path, "file.txt", &collection);
            let des_collection = read::make_collection_with_id(path, "file.txt").unwrap();
            assert_eq!(des_collection, collection);
        });
    }

    fn test_serialize_deserialize_collection<T>(objects: Vec<T>)
    where
        T: PartialEq + Debug + serde::Serialize,
        for<'de> T: serde::Deserialize<'de>,
    {
        let collection = Collection::new(objects);
        ser_deser_in_tmp_dir(|path| {
            write::write_collection(path, "file.txt", &collection);
            let des_collection = read::make_opt_collection(path, "file.txt").unwrap();
            assert_eq!(des_collection, collection);
        });
    }

    #[test]
    fn feed_infos_serialization_deserialization() {
        let mut feed_infos = HashMap::default();
        feed_infos.insert("ntfs_version".to_string(), "0.3".to_string());
        feed_infos.insert("feed_license".to_string(), "".to_string());
        let mut collections = Collections::default();

        ser_deser_in_tmp_dir(|path| {
            write::write_feed_infos(path, &feed_infos);
            read::manage_feed_infos(&mut collections, path).unwrap();
        });
        assert_eq!(collections.feed_infos.len(), 2);
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
        ]);
    }

    #[test]
    fn routes_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            Route {
                id: "IF:002002002:BDE".to_string(),
                name: "Hôtels - Hôtels".to_string(),
                direction_type: Some("foward".to_string()),
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
            },
        ]);
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
                        dropoff_type: 1,
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
                        dropoff_type: 0,
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
        ]);

        ser_deser_in_tmp_dir(|path| {
            write::write_vehicle_journeys_and_stop_times(path, &vehicle_journeys, &stop_points);

            let mut collections = Collections::default();
            collections.vehicle_journeys =
                read::make_collection_with_id::<VehicleJourney>(path, "trips.txt").unwrap();
            collections.stop_points = stop_points;

            read::manage_stop_times(&mut collections, path).unwrap();
            assert_eq!(collections.vehicle_journeys, vehicle_journeys);
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
        test_serialize_deserialize_collection_with_id(vec![
            Equipment {
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
            },
        ]);
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
        let calendars = CollectionWithId::new(vec![
            Calendar {
                id: "0".to_string(),
                monday: false,
                tuesday: false,
                wednesday: false,
                thursday: false,
                friday: false,
                saturday: true,
                sunday: true,
                start_date: chrono::NaiveDate::from_ymd(2018, 1, 7),
                end_date: chrono::NaiveDate::from_ymd(2018, 1, 28),
                calendar_dates: vec![
                    (
                        chrono::NaiveDate::from_ymd(2018, 1, 7),
                        ExceptionType::Remove,
                    ),
                    (chrono::NaiveDate::from_ymd(2018, 1, 15), ExceptionType::Add),
                ],
            },
            Calendar {
                id: "1".to_string(),
                monday: true,
                tuesday: true,
                wednesday: true,
                thursday: true,
                friday: true,
                saturday: false,
                sunday: false,
                start_date: chrono::NaiveDate::from_ymd(2018, 1, 6),
                end_date: chrono::NaiveDate::from_ymd(2018, 1, 27),
                calendar_dates: vec![],
            },
        ]);

        ser_deser_in_tmp_dir(|path| {
            write::write_calendar_and_calendar_dates(path, &calendars);

            let mut collections = Collections::default();
            read::manage_calendars(&mut collections, path).unwrap();

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
                stop_area_id: "sa_2".to_string(),
                fare_zone_id: None,
            },
        ]);

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
        ]);

        ser_deser_in_tmp_dir(|path| {
            write::write_stops(path, &stop_points, &stop_areas);

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
                value: "value:1".to_string(),
                url: Some("http://www.foo.bar".to_string()),
            },
            Comment {
                id: "c:2".to_string(),
                comment_type: CommentType::OnDemandTransport,
                label: Some("label:2".to_string()),
                value: "value:3".to_string(),
                url: Some("http://www.foo.bar".to_string()),
            },
            Comment {
                id: "c:3".to_string(),
                comment_type: CommentType::Information,
                label: None,
                value: "value:1".to_string(),
                url: None,
            },
        ]);

        let stop_points = CollectionWithId::new(vec![
            StopPoint {
                id: "sp_1".to_string(),
                name: "sp_name_1".to_string(),
                codes: vec![("object_system:1".to_string(), "object_code:1".to_string())],
                object_properties: vec![("prop_name:1".to_string(), "prop_value:1".to_string())],
                comment_links: vec!["c:1".to_string()],
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
            },
        ]);

        let stop_areas = CollectionWithId::new(vec![
            StopArea {
                id: "sa_1".to_string(),
                name: "sa_name_1".to_string(),
                codes: vec![("object_system:2".to_string(), "object_code:2".to_string())],
                object_properties: vec![("prop_name:2".to_string(), "prop_value:2".to_string())],
                comment_links: vec!["c:2".to_string()],
                visible: true,
                coord: Coord {
                    lon: 2.073034,
                    lat: 48.799115,
                },
                timezone: None,
                geometry_id: None,
                equipment_id: None,
            },
        ]);

        let lines = CollectionWithId::new(vec![
            Line {
                id: "OIF:002002003:3OIF829".to_string(),
                name: "3".to_string(),
                code: None,
                codes: vec![("object_system:3".to_string(), "object_code:3".to_string())],
                object_properties: vec![("prop_name:3".to_string(), "prop_value:3".to_string())],
                comment_links: vec!["c:1".to_string(), "c:2".to_string()],
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

        let routes = CollectionWithId::new(vec![
            Route {
                id: "OIF:002002002:CEN".to_string(),
                name: "Hôtels - Hôtels".to_string(),
                direction_type: None,
                codes: vec![
                    ("object_system:4".to_string(), "object_code:4".to_string()),
                    ("object_system:5".to_string(), "object_code:5".to_string()),
                ],
                object_properties: vec![("prop_name:4".to_string(), "prop_value:4".to_string())],
                comment_links: vec!["c:3".to_string()],
                line_id: "OIF:002002002:BDEOIF829".to_string(),
                geometry_id: None,
                destination_id: None,
            },
        ]);

        let vehicle_journeys = CollectionWithId::new(vec![
            VehicleJourney {
                id: "OIF:90014407-1_425283-1".to_string(),
                codes: vec![("object_system:6".to_string(), "object_code:6".to_string())],
                object_properties: vec![("prop_name:6".to_string(), "prop_value:6".to_string())],
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
        ]);

        let networks = CollectionWithId::new(vec![
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

        ser_collections.comments = comments;
        ser_collections.stop_areas = stop_areas;
        ser_collections.stop_points = stop_points;
        ser_collections.lines = lines;
        ser_collections.routes = routes;
        ser_collections.vehicle_journeys = vehicle_journeys;
        ser_collections.networks = networks;

        ser_deser_in_tmp_dir(|path| {
            write::write_collection_with_id(path, "lines.txt", &ser_collections.lines);
            write::write_stops(
                path,
                &ser_collections.stop_points,
                &ser_collections.stop_areas,
            );
            write::write_collection_with_id(path, "routes.txt", &ser_collections.routes);
            write::write_collection_with_id(path, "trips.txt", &ser_collections.vehicle_journeys);
            write::write_collection_with_id(path, "networks.txt", &ser_collections.networks);
            write::write_comments(path, &ser_collections);
            write::write_codes(path, &ser_collections);
            write::write_object_properties(path, &ser_collections);

            let mut des_collections = Collections::default();
            des_collections.lines = read::make_collection_with_id(path, "lines.txt").unwrap();
            des_collections.routes = read::make_collection_with_id(path, "routes.txt").unwrap();
            des_collections.vehicle_journeys =
                read::make_collection_with_id(path, "trips.txt").unwrap();
            des_collections.networks = read::make_collection_with_id(path, "networks.txt").unwrap();
            read::manage_stops(&mut des_collections, path).unwrap();
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
                    .get("OIF:90014407-1_425283-1")
                    .unwrap()
                    .comment_links,
                des_collections
                    .vehicle_journeys
                    .get("OIF:90014407-1_425283-1")
                    .unwrap()
                    .comment_links
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
                ser_collections
                    .vehicle_journeys
                    .get("OIF:90014407-1_425283-1")
                    .unwrap()
                    .codes,
                des_collections
                    .vehicle_journeys
                    .get("OIF:90014407-1_425283-1")
                    .unwrap()
                    .codes
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
                wkt: "LINESTRING(2.541951 49.013402,2.571294 49.004725)".to_string(),
            },
            Geometry {
                id: "geo-id-2".to_string(),
                wkt: "LINESTRING(2.548309 49.009182,2.549309 49.009253)".to_string(),
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
