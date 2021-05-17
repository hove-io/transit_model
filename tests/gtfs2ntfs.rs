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

use std::collections::BTreeMap;
use transit_model::{
    gtfs, ntfs,
    objects::{Contributor, Dataset},
    read_utils::read_config,
    test_utils::*,
    PrefixConfiguration,
};

#[test]
fn test_gtfs() {
    test_in_tmp_dir(|path| {
        let input_dir = "./tests/fixtures/gtfs";
        let (contributor, dataset, feed_infos) =
            read_config(Some("./tests/fixtures/gtfs2ntfs/config.json")).unwrap();
        let mut prefix_conf = PrefixConfiguration::default();
        prefix_conf.set_data_prefix("ME");
        prefix_conf.set_schedule_subprefix("WINTER");
        let configuration = transit_model::gtfs::Configuration {
            dataset,
            contributor,
            feed_infos,
            prefix_conf: Some(prefix_conf),
            on_demand_transport: false,
            on_demand_transport_comment: None,
            read_as_line: false,
        };
        let model = transit_model::gtfs::Reader::new(configuration)
            .parse(input_dir)
            .unwrap();
        transit_model::ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(&path, None, "./tests/fixtures/gtfs2ntfs/full_output");
    });
}

#[test]
fn test_minimal_gtfs() {
    test_in_tmp_dir(|path| {
        let input_dir = "./tests/fixtures/gtfs2ntfs/minimal/input";
        let model = transit_model::gtfs::read(input_dir).unwrap();
        ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(&path, None, "./tests/fixtures/gtfs2ntfs/minimal/output");
    });
}

#[test]
fn test_gtfs_physical_modes() {
    test_in_tmp_dir(|path| {
        let input_dir = "./tests/fixtures/gtfs2ntfs/physical_modes/input";
        let model = transit_model::gtfs::read(input_dir).unwrap();
        ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec![
                "commercial_modes.txt",
                "lines.txt",
                "physical_modes.txt",
                "trips.txt",
            ]),
            "./tests/fixtures/gtfs2ntfs/physical_modes/output",
        );
    });
}

#[test]
fn test_gtfs_remove_vjs_with_no_traffic() {
    test_in_tmp_dir(|path| {
        let input_dir = "./tests/fixtures/gtfs2ntfs/no_traffic/input";
        let model = transit_model::gtfs::read(input_dir).unwrap();
        ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec![
                "trips.txt",
                "calendar.txt",
                "stops.txt",
                "routes.txt",
                "stop_times.txt",
                "levels.txt",
            ]),
            "./tests/fixtures/gtfs2ntfs/no_traffic/output",
        );
    });
}

#[test]
fn test_minimal_zipped_gtfs() {
    test_in_tmp_dir(|path| {
        let input = "./tests/fixtures/zipped_gtfs/gtfs.zip";
        let model = transit_model::gtfs::read(input).unwrap();
        ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(&path, None, "./tests/fixtures/gtfs2ntfs/minimal/output");
    });
}

#[test]
fn test_minimal_zipped_sub_dir_gtfs() {
    test_in_tmp_dir(|path| {
        let input = "./tests/fixtures/zipped_gtfs/sub_dir_gtfs.zip";
        let model = transit_model::gtfs::read(input).unwrap();
        ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(&path, None, "./tests/fixtures/gtfs2ntfs/minimal/output");
    });
}

#[test]
fn test_minimal_zipped_sub_dir_gtfs_with_hidden_files() {
    test_in_tmp_dir(|path| {
        let input = "./tests/fixtures/zipped_gtfs/sub_dir_gtfs_with_hidden_files.zip";
        let model = transit_model::gtfs::read(input).unwrap();
        ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(&path, None, "./tests/fixtures/gtfs2ntfs/minimal/output");
    });
}

#[test]
fn test_minimal_gtfs_with_odt_comment() {
    test_in_tmp_dir(|path| {
        let input_dir = "./tests/fixtures/gtfs2ntfs/minimal/input";
        let mut prefix_conf = PrefixConfiguration::default();
        prefix_conf.set_data_prefix("test");
        let configuration = gtfs::Configuration {
            contributor: Contributor::default(),
            dataset: Dataset::default(),
            feed_infos: BTreeMap::new(),
            prefix_conf: Some(prefix_conf),
            on_demand_transport: false,
            on_demand_transport_comment: Some(
                "Service à réservation {agency_name} {agency_phone}".to_string(),
            ),
            read_as_line: false,
        };
        let model = transit_model::gtfs::Reader::new(configuration)
            .parse(input_dir)
            .unwrap();
        ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec!["comment_links.txt", "comments.txt", "stop_times.txt"]),
            "./tests/fixtures/gtfs2ntfs/odt_comment/output_without_frequencies",
        );
    });
}

#[test]
fn test_minimal_gtfs_frequencies_with_odt_comment() {
    test_in_tmp_dir(|path| {
        let input_dir = "./tests/fixtures/gtfs2ntfs/frequencies/input";
        let mut prefix_conf = PrefixConfiguration::default();
        prefix_conf.set_data_prefix("test");
        let configuration = gtfs::Configuration {
            contributor: Contributor::default(),
            dataset: Dataset::default(),
            feed_infos: BTreeMap::new(),
            prefix_conf: Some(prefix_conf),
            on_demand_transport: false,
            on_demand_transport_comment: Some(
                "Service à réservation {agency_name} {agency_phone}".to_string(),
            ),
            read_as_line: false,
        };

        let model = transit_model::gtfs::Reader::new(configuration)
            .parse(input_dir)
            .unwrap();
        ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec!["comment_links.txt", "comments.txt", "stop_times.txt"]),
            "./tests/fixtures/gtfs2ntfs/odt_comment/output_with_frequencies",
        );
    });
}

#[test]
fn test_minimal_gtfs_with_routes_comments() {
    test_in_tmp_dir(|path| {
        let input_dir = "./tests/fixtures/gtfs2ntfs/routes_comments/input";
        let model = transit_model::gtfs::read(input_dir).unwrap();
        ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            None,
            "./tests/fixtures/gtfs2ntfs/routes_comments/output",
        );
    });
}

#[test]
fn test_minimal_gtfs_with_routes_as_lines_comments() {
    test_in_tmp_dir(|path| {
        let input_dir = "./tests/fixtures/gtfs2ntfs/routes_comments/input";
        let configuration = gtfs::Configuration {
            read_as_line: true,
            ..Default::default()
        };
        let model = transit_model::gtfs::Reader::new(configuration)
            .parse(input_dir)
            .unwrap();
        ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            None,
            "./tests/fixtures/gtfs2ntfs/routes_comments/output_as_lines",
        );
    });
}
