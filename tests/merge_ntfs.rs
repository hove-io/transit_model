// Copyright 2017 Kisio Digital and/or its affiliates.
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

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::path::Path;
use transit_model;
use transit_model::model::Collections;
use transit_model::model::Model;
use transit_model::objects::{Comment, StopPoint, VehicleJourney};
use transit_model::test_utils::*;
use transit_model::transfers;
use transit_model::transfers::TransfersMode;
use transit_model_collection::CollectionWithId;
use transit_model_collection::Idx;

#[test]
#[should_panic(expected = "TGC already found")] // first collision is on contributor id
fn merge_collections_with_collisions() {
    let mut collections = Collections::default();
    let input_collisions = ["tests/fixtures/ntfs", "tests/fixtures/ntfs"];
    for input_directory in input_collisions.iter() {
        let to_append_model = transit_model::ntfs::read(input_directory).unwrap();
        collections
            .try_merge(to_append_model.into_collections())
            .unwrap();
    }
}

#[test]
fn merge_collections_ok() {
    let mut collections = Collections::default();
    let input_dirs = ["tests/fixtures/ntfs", "tests/fixtures/merge-ntfs/input"];
    for input_directory in input_dirs.iter() {
        let to_append_model = transit_model::ntfs::read(input_directory).unwrap();

        collections
            .try_merge(to_append_model.into_collections())
            .unwrap();
    }
    assert_eq!(collections.contributors.len(), 2);
    assert_eq!(collections.datasets.len(), 2);
    assert_eq!(collections.networks.len(), 3);
    // check that commercial mode Bus appears once.
    let count_bus = collections
        .commercial_modes
        .values()
        .filter(|cm| cm.id == "Bus" && cm.name == "Bus")
        .count();
    assert_eq!(count_bus, 1);
    assert_eq!(collections.commercial_modes.len(), 6);
    assert_eq!(collections.lines.len(), 6);
    assert_eq!(collections.routes.len(), 8);
    assert_eq!(collections.vehicle_journeys.len(), 8);
    assert_eq!(collections.stop_time_headsigns.len(), 1);
    assert_eq!(collections.stop_time_ids.len(), 5);

    let mut headsigns = HashMap::<(Idx<VehicleJourney>, u32), String>::new();
    headsigns.insert(
        (
            collections
                .vehicle_journeys
                .get_idx("OIF:77100911-1_1420-1")
                .unwrap(),
            3,
        ),
        "somewhere".into(),
    );
    headsigns.insert(
        (
            collections
                .vehicle_journeys
                .get_idx("OIF:77100911-1_1420-1")
                .unwrap(),
            3,
        ),
        "somewhere".into(),
    );
    assert_eq!(headsigns, collections.stop_time_headsigns);

    let mut stop_times_ids = HashMap::<(Idx<VehicleJourney>, u32), String>::new();
    stop_times_ids.insert(
        (
            collections
                .vehicle_journeys
                .get_idx("OIF:77100911-1_1420-1")
                .unwrap(),
            3,
        ),
        "StopTime:OIF:77100911-1_1420-1:1".into(),
    );
    stop_times_ids.insert(
        (
            collections
                .vehicle_journeys
                .get_idx("OIF:77100911-1_1420-1")
                .unwrap(),
            0,
        ),
        "StopTime:OIF:77100911-1_1420-1:0".into(),
    );
    stop_times_ids.insert(
        (
            collections
                .vehicle_journeys
                .get_idx("OIF:77100911-1_1420-1")
                .unwrap(),
            4,
        ),
        "StopTime:OIF:77100911-1_1420-1:2".into(),
    );
    stop_times_ids.insert(
        (collections.vehicle_journeys.get_idx("RERAB1").unwrap(), 5),
        "StopTime:RERAB1-5:1".into(),
    );
    stop_times_ids.insert(
        (collections.vehicle_journeys.get_idx("RERAB1").unwrap(), 8),
        "StopTime:RERAB1-8:0".into(),
    );

    assert_eq!(stop_times_ids, collections.stop_time_ids);

    let mut stop_time_comments = HashMap::<(Idx<VehicleJourney>, u32), Idx<Comment>>::new();
    stop_time_comments.insert(
        (collections.vehicle_journeys.get_idx("RERAB1").unwrap(), 5),
        collections.comments.get_idx("RERACOM1").unwrap(),
    );
    stop_time_comments.insert(
        (
            collections
                .vehicle_journeys
                .get_idx("OIF:77100911-1_1420-1")
                .unwrap(),
            4,
        ),
        collections.comments.get_idx("OIFCOM1").unwrap(),
    );
    assert_eq!(stop_time_comments, collections.stop_time_comments);

    fn get_stop_point_idxs(
        col: &CollectionWithId<VehicleJourney>,
        id: &str,
    ) -> Vec<Idx<StopPoint>> {
        col.get(id)
            .unwrap()
            .stop_times
            .iter()
            .map(|st| st.stop_point_idx)
            .collect()
    }

    assert_eq!(
        get_stop_point_idxs(&collections.vehicle_journeys, "RERAB1"),
        vec![
            collections.stop_points.get_idx("DEFR").unwrap(),
            collections.stop_points.get_idx("CDGR").unwrap(),
            collections.stop_points.get_idx("GDLR").unwrap(),
            collections.stop_points.get_idx("NATR").unwrap(),
        ]
    );
    assert_eq!(
        get_stop_point_idxs(&collections.vehicle_journeys, "OIF:77100911-1_1420-1"),
        vec![
            collections.stop_points.get_idx("OIF:SP:10:10").unwrap(),
            collections.stop_points.get_idx("OIF:SP:10:100").unwrap(),
            collections.stop_points.get_idx("OIF:SP:10:200").unwrap(),
        ]
    );
    assert_eq!(collections.physical_modes.len(), 6);
    assert_eq!(collections.stop_areas.len(), 7);
    assert_eq!(collections.stop_points.len(), 14);
    assert_eq!(collections.feed_infos.len(), 0);
    let calendar_vec = collections.calendars.into_vec();
    assert_eq!(calendar_vec[0].dates.len(), 261);
    assert_eq!(calendar_vec[1].dates.len(), 6);
    assert_eq!(collections.companies.len(), 3);
    assert_eq!(collections.comments.len(), 6);
    assert_eq!(collections.equipments.len(), 0);
    assert_eq!(collections.transfers.len(), 0);
    assert_eq!(collections.trip_properties.len(), 0);
    assert_eq!(collections.geometries.len(), 0);
    assert_eq!(collections.admin_stations.len(), 0);
}

#[test]
fn merge_collections_with_transfers_ok() {
    let mut collections = Collections::default();
    test_in_tmp_dir(|path| {
        let report_path = path.join("report.json");
        let input_dirs = [
            "tests/fixtures/minimal_ntfs",
            "tests/fixtures/merge-ntfs/input",
        ];
        let rule_paths =
            vec![Path::new("./tests/fixtures/merge-ntfs/transfer_rules.csv").to_path_buf()];
        for input_directory in input_dirs.iter() {
            let to_append_model = transit_model::ntfs::read(input_directory).unwrap();

            collections
                .try_merge(to_append_model.into_collections())
                .unwrap();
        }
        let model = Model::new(collections).unwrap();
        let model = transfers::generates_transfers(
            model,
            100.0,
            0.785,
            60,
            rule_paths,
            &TransfersMode::InterContributor,
            Some(Path::new(&report_path).to_path_buf()),
        )
        .unwrap();
        transit_model::ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec!["transfers.txt", "report.json"]),
            "./tests/fixtures/merge-ntfs/output",
        );
    });
}

#[test]
fn merge_collections_with_feed_infos() {
    let mut collections = Collections::default();
    test_in_tmp_dir(|path| {
        let feed_infos_file = File::open("tests/fixtures/merge-ntfs/feed_infos.json").unwrap();
        let mut feed_infos: BTreeMap<String, String> =
            serde_json::from_reader(feed_infos_file).unwrap();
        for input_directory in &[
            "tests/fixtures/minimal_ntfs",
            "tests/fixtures/merge-ntfs/input",
        ] {
            let to_append_model = transit_model::ntfs::read(input_directory).unwrap();
            collections
                .try_merge(to_append_model.into_collections())
                .unwrap();
        }
        collections.feed_infos.append(&mut feed_infos);
        let model = Model::new(collections).unwrap();
        transit_model::ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec!["feed_infos.txt"]),
            "./tests/fixtures/merge-ntfs/output_feedinfos",
        );
    });
}

#[test]
fn merge_collections_fares_v2() {
    let mut collections = Collections::default();
    test_in_tmp_dir(|path| {
        let input_dirs = ["tests/fixtures/ntfs", "tests/fixtures/merge-ntfs/input"];
        for input_directory in input_dirs.iter() {
            let to_append_model = transit_model::ntfs::read(input_directory).unwrap();

            collections
                .try_merge(to_append_model.into_collections())
                .unwrap();
        }
        let model = Model::new(collections).unwrap();
        transit_model::ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec![
                "tickets.txt",
                "ticket_prices.txt",
                "ticket_uses.txt",
                "ticket_use_perimeters.txt",
                "ticket_use_restrictions.txt",
                "prices.csv",
                "fares.csv",
                "od_fares.csv",
            ]),
            "./tests/fixtures/merge-ntfs/output_merge_fares",
        );
    });
}

#[test]
#[should_panic(expected = "ticket.1 already found")]
fn merge_collections_fares_v2_with_collisions() {
    let mut collections = Collections::default();
    let input_dirs = [
        "tests/fixtures/ntfs",
        "tests/fixtures/merge-ntfs/input_farev2_conflicts",
    ];
    for input_directory in input_dirs.iter() {
        let to_append_model = transit_model::ntfs::read(input_directory).unwrap();

        collections
            .try_merge(to_append_model.into_collections())
            .unwrap();
    }
}

#[test]
#[should_panic(expected = "Cannot convert Fares V2 to V1. Prices or fares are empty.")]
fn merge_collections_fares_v2_not_convertible_in_v1() {
    let mut collections = Collections::default();
    test_in_tmp_dir(|path| {
        let input_dirs = [
            "tests/fixtures/minimal_ntfs",
            "tests/fixtures/merge-ntfs/input_faresv2_without_euro_currency",
        ];
        for input_directory in input_dirs.iter() {
            let to_append_model = transit_model::ntfs::read(input_directory).unwrap();
            collections
                .try_merge(to_append_model.into_collections())
                .unwrap();
        }
        let model = Model::new(collections).unwrap();
        transit_model::ntfs::write(&model, path, get_test_datetime()).unwrap();
    });
}

#[test]
fn merge_collections_fares_v2_with_ntfs_only_farev1() {
    let mut collections = Collections::default();
    test_in_tmp_dir(|path| {
        let input_dirs = [
            "tests/fixtures/ntfs",
            "tests/fixtures/merge-ntfs/input_only_farev1",
        ];
        for input_directory in input_dirs.iter() {
            let to_append_model = transit_model::ntfs::read(input_directory).unwrap();

            collections
                .try_merge(to_append_model.into_collections())
                .unwrap();
        }
        let model = Model::new(collections).unwrap();
        transit_model::ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec![
                "tickets.txt",
                "ticket_prices.txt",
                "ticket_uses.txt",
                "ticket_use_perimeters.txt",
                "ticket_use_restrictions.txt",
                "prices.csv",
                "fares.csv",
                "od_fares.csv",
            ]),
            "./tests/fixtures/merge-ntfs/output_merge_fares_only_one_farev2",
        );
    });
}
