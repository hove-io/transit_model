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

use navitia_model;
use navitia_model::collection::CollectionWithId;
use navitia_model::collection::Idx;
use navitia_model::model::Collections;
use navitia_model::model::Model;
use navitia_model::objects::{Comment, StopPoint, VehicleJourney};
use navitia_model::test_utils::*;
use navitia_model::transfers;
use navitia_model::transfers::TransfersMode;
use std::collections::HashMap;
use std::path::Path;

#[test]
#[should_panic(expected = "TGC already found")] // first collision is on contributor id
fn merge_collections_with_collisions() {
    let mut collections = Collections::default();
    let input_collisions = ["fixtures/ntfs", "fixtures/ntfs"];
    for input_directory in input_collisions.iter() {
        let to_append_model = navitia_model::ntfs::read(input_directory).unwrap();
        collections
            .merge(to_append_model.into_collections())
            .unwrap();
    }
}

#[test]
fn merge_collections_ok() {
    let mut collections = Collections::default();
    let input_dirs = ["fixtures/ntfs", "fixtures/merge-ntfs/input"];
    for input_directory in input_dirs.iter() {
        let to_append_model = navitia_model::ntfs::read(input_directory).unwrap();

        collections
            .merge(to_append_model.into_collections())
            .unwrap();
    }
    assert_eq!(collections.contributors.len(), 2);
    assert_eq!(collections.datasets.len(), 2);
    assert_eq!(collections.networks.len(), 3);
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
    assert_eq!(collections.feed_infos.len(), 10);
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
        let input_dirs = ["fixtures/minimal_ntfs", "fixtures/merge-ntfs/input"];
        let rule_paths = vec![Path::new("./fixtures/merge-ntfs/transfer_rules.csv").to_path_buf()];
        for input_directory in input_dirs.iter() {
            let to_append_model = navitia_model::ntfs::read(input_directory).unwrap();

            collections
                .merge(to_append_model.into_collections())
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
        navitia_model::ntfs::write(&model, path).unwrap();
        compare_output_dir_with_expected(
            &path,
            vec!["transfers.txt", "report.json"],
            "./fixtures/merge-ntfs/output",
        );
    });
}
