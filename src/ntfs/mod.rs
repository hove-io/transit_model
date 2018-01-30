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

pub fn read<P: AsRef<path::Path>>(path: P) -> PtObjects {
    let path = path.as_ref();
    info!("Loading NTFS from {:?}", path);
    let mut collections = Collections::default();
    collections.contributors = read::make_collection_with_id(path, "contributors.txt");
    collections.datasets = read::make_collection_with_id(path, "datasets.txt");
    collections.commercial_modes = read::make_collection_with_id(path, "commercial_modes.txt");
    collections.networks = read::make_collection_with_id(path, "networks.txt");
    collections.lines = read::make_collection_with_id(path, "lines.txt");
    collections.routes = read::make_collection_with_id(path, "routes.txt");
    collections.vehicle_journeys = read::make_collection_with_id(path, "trips.txt");
    collections.physical_modes = read::make_collection_with_id(path, "physical_modes.txt");
    read::manage_calendars(&mut collections, path);
    collections.companies = read::make_collection_with_id(path, "companies.txt");
    read::manage_feed_infos(&mut collections, path);
    read::manage_stops(&mut collections, path);
    read::manage_stop_times(&mut collections, path);
    read::manage_codes(&mut collections, path);
    read::manage_comments(&mut collections, path);
    collections.equipments = read::make_collection_with_id(path, "equipments.txt");
    collections.transfers = read::make_collection(path, "transfers.txt");
    info!("Indexing");
    let res = PtObjects::new(collections);
    info!("Loading NTFS done");
    res
}

pub fn write<P: AsRef<path::Path>>(path: P, pt_objects: &PtObjects) {
    let path = path.as_ref();
    info!("Writing NTFS to {:?}", path);

    write::write_feed_infos(path, &pt_objects.feed_infos);
    write::write_collection_with_id(path, "networks.txt", &pt_objects.networks);
    write::write_collection_with_id(path, "commercial_modes.txt", &pt_objects.networks);
    write::write_collection_with_id(path, "companies.txt", &pt_objects.companies);
    write::write_collection_with_id(path, "lines.txt", &pt_objects.lines);
    write::write_collection_with_id(path, "physical_modes.txt", &pt_objects.lines);
    write::write_collection_with_id(path, "routes.txt", &pt_objects.lines);
    write::write_vehicle_journeys_and_stop_times(
        path,
        &pt_objects.vehicle_journeys,
        &pt_objects.stop_points,
    );
}

#[cfg(test)]
mod tests {
    extern crate tempdir;
    use self::tempdir::TempDir;
    use objects::*;
    use CollectionWithId;
    use super::{read, write};
    use super::Collections;
    use collection::Id;
    use std::collections::HashMap;
    use serde;
    use std::fmt::Debug;

    #[test]
    fn feed_infos_serialization_deserialization() {
        let mut feed_infos = HashMap::default();
        feed_infos.insert("ntfs_version".to_string(), "0.3".to_string());
        feed_infos.insert("feed_license".to_string(), "".to_string());
        let mut collections = Collections::default();

        let tmp_dir = TempDir::new("navitia_model_tests").expect("create temp dir");
        {
            let path = tmp_dir.as_ref();
            write::write_feed_infos(path, &feed_infos);
            read::manage_feed_infos(&mut collections, path);
        }
        tmp_dir.close().expect("delete temp dir");

        assert_eq!(collections.feed_infos.len(), 2);
        assert_eq!(collections.feed_infos, feed_infos);
    }

    fn test_serialize_deserialize_collection_with_id<T>(objects: Vec<T>)
    where
        T: Id<T> + PartialEq + Debug + serde::Serialize,
        for<'de> T: serde::Deserialize<'de>,
    {
        let collection = CollectionWithId::new(objects);
        let tmp_dir = TempDir::new("navitia_model_tests").expect("create temp dir");

        {
            let path = tmp_dir.as_ref();
            write::write_collection_with_id(path, "file.txt", &collection);
            let des_collection = read::make_collection_with_id(path, "file.txt");
            assert_eq!(des_collection, collection);
        }
        tmp_dir.close().expect("delete temp dir");
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
                codes: CodesT::default(),
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
                codes: CodesT::default(),
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
                codes: CodesT::default(),
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
                codes: CodesT::default(),
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
                codes: CodesT::default(),
                comment_links: CommentLinksT::default(),
                line_id: "OIF:002002002:BDEOIF829".to_string(),
                geometry_id: Some("Geometry:Line:Relation:6883353".to_string()),
                destination_id: Some("OIF,OIF:SA:4:126".to_string()),
            },
            Route {
                id: "OIF:002002002:CEN".to_string(),
                name: "Hôtels - Hôtels".to_string(),
                direction_type: None,
                codes: CodesT::default(),
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
                codes: CodesT::default(),
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
            },
            StopPoint {
                id: "OIF:SP:36:2127".to_string(),
                name: "Division Leclerc".to_string(),
                codes: CodesT::default(),
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
            },
        ]);
        let vehicle_journeys = CollectionWithId::new(vec![
            VehicleJourney {
                id: "OIF:87604986-1_11595-1".to_string(),
                codes: CodesT::default(),
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
                codes: CodesT::default(),
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

        let tmp_dir = TempDir::new("navitia_model_tests").expect("create temp dir");

        {
            let path = tmp_dir.as_ref();
            write::write_vehicle_journeys_and_stop_times(path, &vehicle_journeys, &stop_points);

            let mut collections = Collections::default();
            collections.vehicle_journeys =
                read::make_collection_with_id::<VehicleJourney>(path, "trips.txt");
            collections.stop_points = stop_points;

            read::manage_stop_times(&mut collections, path);
            assert_eq!(collections.vehicle_journeys, vehicle_journeys);
        }
        tmp_dir.close().expect("delete temp dir");
    }
}
