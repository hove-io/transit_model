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

use approx::assert_relative_eq;
use pretty_assertions::assert_eq;
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::path::Path;
use transit_model;
use transit_model::model::Collections;
use transit_model::model::Model;
use transit_model::objects::{Comment, CommentLinks, StopPoint, VehicleJourney};
use transit_model::test_utils::*;
use transit_model::transfers;
use transit_model::transfers::TransfersMode;
use typed_index_collection::{CollectionWithId, Idx};

#[test]
fn merge_collections_with_collisions() {
    let mut collections = Collections::default();
    let input_collisions = ["tests/fixtures/ntfs", "tests/fixtures/ntfs"];

    let error_message = input_collisions
        .iter()
        .map(|input_directory| transit_model::ntfs::read(input_directory).unwrap())
        .map(|model| collections.try_merge(model.into_collections()))
        .collect::<Result<(), _>>()
        .unwrap_err()
        .to_string();
    assert_eq!("identifier TGC already exists", error_message);
}

#[test]
#[allow(clippy::cognitive_complexity)]
fn merge_collections_ok() {
    let mut collections = Collections::default();
    let input_dirs = ["tests/fixtures/ntfs", "tests/fixtures/merge-ntfs/input"];
    for input_directory in input_dirs.iter() {
        let to_append_model = transit_model::ntfs::read(input_directory).unwrap();

        collections
            .try_merge(to_append_model.into_collections())
            .unwrap();
    }
    assert_eq!(2, collections.contributors.len());
    assert_eq!(2, collections.datasets.len());
    assert_eq!(3, collections.networks.len());
    // check that commercial mode Bus appears once.
    let count_bus = collections
        .commercial_modes
        .values()
        .filter(|cm| cm.id == "Bus" && cm.name == "Bus")
        .count();
    assert_eq!(1, count_bus);
    // Check that the merge of CO2 emission keeps only the biggest value
    let bus_mode = collections.physical_modes.get("Bus").unwrap();
    assert_relative_eq!(bus_mode.co2_emission.unwrap(), 132f32);

    assert_eq!(5, collections.commercial_modes.len());
    // 4 + 3 automatically inserted 'Bike', 'BikeSharingService', and 'Car'
    assert_eq!(7, collections.physical_modes.len());
    assert_eq!(5, collections.lines.len());
    assert_eq!(8, collections.routes.len());
    assert_eq!(10, collections.vehicle_journeys.len());
    assert_eq!(2, collections.frequencies.len());
    assert_eq!(1, collections.stop_time_headsigns.len());
    assert_eq!(8, collections.stop_time_ids.len());
    assert_eq!(4, collections.levels.len());
    assert_eq!(3, collections.pathways.len());
    assert_eq!(1, collections.grid_calendars.len());
    assert_eq!(1, collections.grid_exception_dates.len());
    assert_eq!(1, collections.grid_periods.len());
    assert_eq!(1, collections.grid_rel_calendar_line.len());

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
        (
            collections
                .vehicle_journeys
                .get_idx("OIF:77100915-1_1424-1")
                .unwrap(),
            0,
        ),
        "StopTime:OIF:77100915-1_1424-1:0".into(),
    );
    stop_times_ids.insert(
        (
            collections
                .vehicle_journeys
                .get_idx("OIF:77100921-1_1420-1")
                .unwrap(),
            0,
        ),
        "StopTime:OIF:77100921-1_1420-1:0".into(),
    );
    stop_times_ids.insert(
        (
            collections
                .vehicle_journeys
                .get_idx("OIF:77100925-1_1424-1")
                .unwrap(),
            0,
        ),
        "StopTime:OIF:77100925-1_1424-1:0".into(),
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
        vec![
            collections.stop_points.get_idx("DEFR").unwrap(),
            collections.stop_points.get_idx("CDGR").unwrap(),
            collections.stop_points.get_idx("GDLR").unwrap(),
            collections.stop_points.get_idx("NATR").unwrap(),
        ],
        get_stop_point_idxs(&collections.vehicle_journeys, "RERAB1")
    );
    assert_eq!(
        vec![
            collections.stop_points.get_idx("OIF:SP:10:10").unwrap(),
            collections.stop_points.get_idx("OIF:SP:10:100").unwrap(),
            collections.stop_points.get_idx("OIF:SP:10:200").unwrap(),
        ],
        get_stop_point_idxs(&collections.vehicle_journeys, "OIF:77100911-1_1420-1")
    );
    assert_eq!(7, collections.stop_areas.len());
    assert_eq!(14, collections.stop_points.len());
    assert_eq!(0, collections.feed_infos.len());
    let calendar_vec = collections.calendars.into_vec();
    assert_eq!(261, calendar_vec[0].dates.len());
    assert_eq!(6, calendar_vec[1].dates.len());
    assert_eq!(3, collections.companies.len());
    assert_eq!(7, collections.comments.len());
    assert_eq!(0, collections.equipments.len());
    assert_eq!(0, collections.transfers.len());
    assert_eq!(0, collections.trip_properties.len());
    assert_eq!(0, collections.geometries.len());
    assert_eq!(0, collections.admin_stations.len());

    fn assert_comment_idx<T: CommentLinks>(
        collection: &CollectionWithId<T>,
        obj_id: &str,
        comments: &CollectionWithId<Comment>,
        comment_id: &str,
    ) {
        assert_eq!(
            &comments.get_idx(comment_id).unwrap(),
            collection
                .get(obj_id)
                .unwrap()
                .comment_links()
                .iter()
                .next()
                .unwrap()
        );
    }

    assert_comment_idx(
        &collections.stop_points,
        "OIF:SP:10:10",
        &collections.comments,
        "OIFCOM2",
    );
    assert_comment_idx(
        &collections.stop_areas,
        "OIF:SA:10:1002",
        &collections.comments,
        "OIFCOM3",
    );
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
fn merge_collections_fares_v2_with_collisions() {
    let mut collections = Collections::default();
    let input_dirs = [
        "tests/fixtures/ntfs",
        "tests/fixtures/merge-ntfs/input_farev2_conflicts",
    ];

    let error_message = input_dirs
        .iter()
        .map(|input_directory| transit_model::ntfs::read(input_directory).unwrap())
        .map(|model| collections.try_merge(model.into_collections()))
        .collect::<Result<(), _>>()
        .unwrap_err()
        .to_string();
    assert_eq!("identifier ticket.1 already exists", error_message);
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
