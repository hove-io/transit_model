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
use transit_model::ntfs::filter;
use transit_model::test_utils::*;

#[test]
fn test_extract_network() {
    test_in_tmp_dir(|path| {
        let input_dir = "tests/fixtures/filter_ntfs/input";

        let model = filter::filter(
            transit_model::ntfs::read(input_dir).unwrap(),
            filter::Action::Extract,
            vec!["network1".into()],
        )
        .unwrap();
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

        let model = filter::filter(
            transit_model::ntfs::read(input_dir).unwrap(),
            filter::Action::Remove,
            vec!["network1".into()],
        )
        .unwrap();
        transit_model::ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(&path, None, "./tests/fixtures/filter_ntfs/output_remove");
    });
}

#[test]
#[should_panic(expected = "network unknown not found.")]
fn test_extract_with_unknown_network() {
    let input_dir = "tests/fixtures/filter_ntfs/input";
    filter::filter(
        transit_model::ntfs::read(input_dir).unwrap(),
        filter::Action::Extract,
        vec!["unknown".into()],
    )
    .unwrap();
}

#[test]
#[should_panic(expected = "the data does not contain services anymore.")]
fn test_remove_all_networks() {
    let input_dir = "tests/fixtures/filter_ntfs/input";
    filter::filter(
        transit_model::ntfs::read(input_dir).unwrap(),
        filter::Action::Remove,
        vec!["network1".into(), "network2".into(), "network3".into()],
    )
    .unwrap();
}
