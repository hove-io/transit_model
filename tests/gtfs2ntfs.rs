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

extern crate navitia_model;
use navitia_model::test_utils::*;

#[test]
fn test_frequencies_generate_trips() {
    test_in_tmp_dir(|path| {
        let input_dir = "./fixtures/gtfs2ntfs/frequencies/input";
        let model = navitia_model::gtfs::read(input_dir, None, None).unwrap();
        navitia_model::ntfs::write(&model, path).unwrap();
        compare_output_dir_with_expected(
            &path,
            vec![
                "calendar_dates.txt",
                "trips.txt",
                "stop_times.txt",
                "object_codes.txt",
            ],
            "./fixtures/gtfs2ntfs/frequencies/output",
        );
    }
}
fn test_minimal_gtfs() {
    test_in_tmp_dir(|path| {
        let input_dir = "./fixtures/gtfs2ntfs/minimal/input";
        let model = navitia_model::gtfs::read(input_dir, None, None).unwrap();
        navitia_model::ntfs::write(&model, path).unwrap();
        compare_output_dir_with_expected(&path, vec![], "./fixtures/gtfs2ntfs/minimal/output");
    });
}
