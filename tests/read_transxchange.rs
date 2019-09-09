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
fn test_read_transxchange() {
    let ntm = transit_model::transxchange::read(
        "tests/fixtures/transxchange2ntfs/input/transxchange",
        "tests/fixtures/transxchange2ntfs/input/naptan",
        Some("tests/fixtures/transxchange2ntfs/input/config.json"),
        Some("prefix".into()),
        chrono::NaiveDate::from_ymd(2021, 12, 31),
    )
    .unwrap();
    test_in_tmp_dir(|output_dir| {
        transit_model::ntfs::write(&ntm, output_dir, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &output_dir,
            Some(vec![
                "calendar.txt",
                "companies.txt",
                "contributors.txt",
                "commercial_modes.txt",
                "datasets.txt",
                "feed_infos.txt",
                "lines.txt",
                "networks.txt",
                "object_codes.txt",
                "physical_modes.txt",
                "stops.txt",
                "stop_times.txt",
                "routes.txt",
                "trips.txt",
            ]),
            "tests/fixtures/transxchange2ntfs/output/ntfs",
        );
    });
}
