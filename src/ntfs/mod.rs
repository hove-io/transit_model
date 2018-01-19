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

pub fn read<P: AsRef<path::Path>>(path: P) -> PtObjects {
    let path = path.as_ref();
    info!("Loading NTFS from {:?}", path);
    let mut collections = Collections::default();
    collections.contributors = read::make_collection_with_id(&path, "contributors.txt");
    collections.datasets = read::make_collection_with_id(&path, "datasets.txt");
    collections.commercial_modes = read::make_collection_with_id(&path, "commercial_modes.txt");
    collections.networks = read::make_collection_with_id(&path, "networks.txt");
    collections.lines = read::make_collection_with_id(&path, "lines.txt");
    collections.routes = read::make_collection_with_id(&path, "routes.txt");
    collections.vehicle_journeys = read::make_collection_with_id(&path, "trips.txt");
    collections.physical_modes = read::make_collection_with_id(&path, "physical_modes.txt");
    read::manage_calendars(&mut collections, path);
    collections.companies = read::make_collection_with_id(&path, "companies.txt");
    read::manage_feed_infos(&mut collections, &path);
    read::manage_stops(&mut collections, path);
    read::manage_stop_times(&mut collections, path);
    read::manage_codes(&mut collections, path);
    read::manage_comments(&mut collections, path);
    collections.equipments = read::make_collection_with_id(&path, "equipments.txt");
    collections.transfers = read::make_collection(path, "transfers.txt");
    info!("Indexing");
    let res = PtObjects::new(collections);
    info!("Loading NTFS done");
    res
}

pub fn write<P: AsRef<path::Path>>(path: P, pt_objects: &PtObjects) {
    let path = path.as_ref();
    info!("Writing NTFS to {:?}", path);

    write::write_feed_infos(&path, &pt_objects.feed_infos);
    write::write_collection_with_id(&path, "networks.txt", &pt_objects.networks);
}

#[cfg(test)]
mod tests {
    extern crate tempdir;
    use self::tempdir::TempDir;
    use objects::*;
    use CollectionWithId;
    use super::{read, write};
    use super::Collections;
    use std::collections::HashMap;

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

    #[test]
    fn networks_serialization_deserialization() {
        let expected_networks = vec![
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
        ];

        let collection_with_id = CollectionWithId::new(expected_networks);

        let tmp_dir = TempDir::new("navitia_model_tests").expect("create temp dir");

        {
            let path = tmp_dir.as_ref();
            write::write_collection_with_id(path, "networks.txt", &collection_with_id);
            let networks = read::make_collection_with_id(path, "networks.txt");
            assert_eq!(networks, collection_with_id);
        }
        tmp_dir.close().expect("delete temp dir");
    }
}
