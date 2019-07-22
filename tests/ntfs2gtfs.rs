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
fn test_stop_zones_not_exported_and_cleaned() {
    test_in_tmp_dir(|path| {
        let input = "./fixtures/ntfs2gtfs/input";
        let model = transit_model::ntfs::read(input).unwrap();
        transit_model::gtfs::write(model, path).unwrap();
        compare_output_dir_with_expected(&path, None, "./fixtures/ntfs2gtfs/output");
    });
}

#[test]
fn test_platforms_preserving() {
    test_in_tmp_dir(|path| {
        let input = "./fixtures/ntfs2gtfs/platforms/input";
        let model = transit_model::ntfs::read(input).unwrap();
        transit_model::gtfs::write(model, path).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec!["stops.txt"]),
            "./fixtures/ntfs2gtfs/platforms/output",
        );
    });
}
