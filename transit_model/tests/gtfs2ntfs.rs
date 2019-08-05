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

use transit_model;
use transit_model::test_utils::*;

#[test]
fn test_frequencies_generate_trips() {
    test_in_tmp_dir(|path| {
        let input_dir = "./fixtures/gtfs2ntfs/frequencies/input";
        let model = transit_model::gtfs::read_from_path(input_dir, None, None).unwrap();
        transit_model::ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec![
                "calendar_dates.txt",
                "calendar.txt",
                "trips.txt",
                "stop_times.txt",
                "object_codes.txt",
            ]),
            "./fixtures/gtfs2ntfs/frequencies/output",
        );
    });
}

#[test]
fn test_minimal_gtfs() {
    test_in_tmp_dir(|path| {
        let input_dir = "./fixtures/gtfs2ntfs/minimal/input";
        let model = transit_model::gtfs::read_from_path(input_dir, None, None).unwrap();
        transit_model::ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(&path, None, "./fixtures/gtfs2ntfs/minimal/output");
    });
}

#[test]
fn test_minimal_gtfs_with_feed_infos() {
    test_in_tmp_dir(|path| {
        let input_dir = "./fixtures/gtfs2ntfs/minimal_with_config/input";
        let config = "./fixtures/gtfs2ntfs/minimal_with_config/config.json";
        let model = transit_model::gtfs::read_from_path(input_dir, Some(config), None).unwrap();
        transit_model::ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec![
                "contributors.txt",
                "trips.txt",
                "datasets.txt",
                "feed_infos.txt",
            ]),
            "./fixtures/gtfs2ntfs/minimal_with_config/output",
        );
    });
}

#[test]
fn test_gtfs_physical_modes() {
    test_in_tmp_dir(|path| {
        let input_dir = "./fixtures/gtfs2ntfs/physical_modes/input";
        let model = transit_model::gtfs::read_from_path(input_dir, None, None).unwrap();
        transit_model::ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec![
                "commercial_modes.txt",
                "lines.txt",
                "physical_modes.txt",
                "trips.txt",
            ]),
            "./fixtures/gtfs2ntfs/physical_modes/output",
        );
    });
}

#[test]
fn test_gtfs_remove_vjs_with_no_traffic() {
    test_in_tmp_dir(|path| {
        let input_dir = "./fixtures/gtfs2ntfs/no_traffic/input";
        let model = transit_model::gtfs::read_from_path(input_dir, None, None).unwrap();
        transit_model::ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec![
                "trips.txt",
                "calendar.txt",
                "stops.txt",
                "routes.txt",
                "stop_times.txt",
            ]),
            "./fixtures/gtfs2ntfs/no_traffic/output",
        );
    });
}

#[test]
fn test_minimal_ziped_gtfs() {
    test_in_tmp_dir(|path| {
        let input = "./fixtures/ziped_gtfs/gtfs.zip";
        let model = transit_model::gtfs::read_from_zip(input, None, None).unwrap();
        transit_model::ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(&path, None, "./fixtures/gtfs2ntfs/minimal/output");
    });
}

#[test]
fn test_minimal_ziped_sub_dir_gtfs() {
    test_in_tmp_dir(|path| {
        let input = "./fixtures/ziped_gtfs/sub_dir_gtfs.zip";
        let model = transit_model::gtfs::read_from_zip(input, None, None).unwrap();
        transit_model::ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(&path, None, "./fixtures/gtfs2ntfs/minimal/output");
    });
}

#[test]
fn test_gtfs_with_platforms() {
    test_in_tmp_dir(|path| {
        let input_dir = "./fixtures/gtfs2ntfs/platforms/input";
        let model = transit_model::gtfs::read_from_path(input_dir, None, None).unwrap();
        transit_model::ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec!["stops.txt"]),
            "./fixtures/gtfs2ntfs/platforms/output",
        );
    });
}
