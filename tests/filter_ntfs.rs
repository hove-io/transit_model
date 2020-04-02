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

use transit_model;
use transit_model::ntfs::filter;
use transit_model::test_utils::*;

#[test]
fn test_extract_network() {
    test_in_tmp_dir(|path| {
        let input_dir = "tests/fixtures/filter_ntfs/input";

        let mut filter = filter::Filter::new(filter::Action::Extract);
        filter.add(filter::ObjectType::Network, "network_id", "network1");

        let model = filter::filter(transit_model::ntfs::read(input_dir).unwrap(), &filter).unwrap();
        transit_model::ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            None,
            "./tests/fixtures/filter_ntfs/output_extract",
        );
    });
}

#[test]
fn test_remove_network() {
    test_in_tmp_dir(|path| {
        let input_dir = "tests/fixtures/filter_ntfs/input";

        let mut filter = filter::Filter::new(filter::Action::Remove);
        filter.add(filter::ObjectType::Network, "network_id", "network1");

        let model = filter::filter(transit_model::ntfs::read(input_dir).unwrap(), &filter).unwrap();
        transit_model::ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(&path, None, "./tests/fixtures/filter_ntfs/output_remove");
    });
}

#[test]
#[should_panic(expected = "Network \\'unknown\\' not found.")]
fn test_extract_with_unknown_network() {
    let input_dir = "tests/fixtures/filter_ntfs/input";
    let mut filter = filter::Filter::new(filter::Action::Extract);
    filter.add(filter::ObjectType::Network, "network_id", "unknown");

    filter::filter(transit_model::ntfs::read(input_dir).unwrap(), &filter).unwrap();
}

#[test]
#[should_panic(expected = "the data does not contain vehicle journeys anymore.")]
fn test_remove_all_networks() {
    let input_dir = "tests/fixtures/filter_ntfs/input";
    let mut filter = filter::Filter::new(filter::Action::Remove);
    filter.add(filter::ObjectType::Network, "network_id", "network1");
    filter.add(filter::ObjectType::Network, "network_id", "network2");
    filter.add(filter::ObjectType::Network, "network_id", "network3");
    filter::filter(transit_model::ntfs::read(input_dir).unwrap(), &filter).unwrap();
}

#[test]
fn test_remove_line_by_line_code() {
    test_in_tmp_dir(|path| {
        let input_dir = "tests/fixtures/filter_ntfs/input";

        let mut filter = filter::Filter::new(filter::Action::Remove);
        filter.add(filter::ObjectType::Line, "line_code", "route3");

        let model = filter::filter(transit_model::ntfs::read(input_dir).unwrap(), &filter).unwrap();
        transit_model::ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            None,
            "./tests/fixtures/filter_ntfs/output_remove_line",
        );
    });
}

#[test]
fn test_remove_line_by_line_id() {
    test_in_tmp_dir(|path| {
        let input_dir = "tests/fixtures/filter_ntfs/input";

        let mut filter = filter::Filter::new(filter::Action::Remove);
        filter.add(filter::ObjectType::Line, "line_id", "line3");

        let model = filter::filter(transit_model::ntfs::read(input_dir).unwrap(), &filter).unwrap();
        transit_model::ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            None,
            "./tests/fixtures/filter_ntfs/output_remove_line",
        );
    });
}

#[test]
fn test_extract_multiple_line_by_line_code() {
    test_in_tmp_dir(|path| {
        let input_dir = "tests/fixtures/filter_ntfs/input";

        let mut filter = filter::Filter::new(filter::Action::Extract);
        filter.add(filter::ObjectType::Line, "line_code", "route1");
        filter.add(filter::ObjectType::Line, "line_code", "route3");

        let model = filter::filter(transit_model::ntfs::read(input_dir).unwrap(), &filter).unwrap();
        transit_model::ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            None,
            "./tests/fixtures/filter_ntfs/output_extract_multiple_lines",
        );
    });
}
