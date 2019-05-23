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

use std::path::Path;
use transit_model;
use transit_model::apply_rules;
use transit_model::test_utils::*;

#[test]
fn test_apply_complementary_codes() {
    test_in_tmp_dir(|path| {
        let input_dir = "fixtures/apply_rules/input";
        let cc_rules =
            vec![Path::new("./fixtures/apply_rules/complementary_codes_rules.txt").to_path_buf()];
        let p_rules = vec![Path::new("./fixtures/apply_rules/property_rules.txt").to_path_buf()];
        let n_consolidation = Path::new("").to_path_buf();
        let report_path = path.join("report.json");

        let model = apply_rules::apply_rules(
            transit_model::ntfs::read(input_dir).unwrap(),
            cc_rules,
            p_rules,
            n_consolidation,
            Path::new(&report_path).to_path_buf(),
        )
        .unwrap();
        transit_model::ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec![
                "object_codes.txt",
                "geometries.txt",
                "lines.txt",
                "routes.txt",
                "trips.txt",
                "stops.txt",
                "networks.txt",
                "report.json",
            ]),
            "./fixtures/apply_rules/output",
        );
    });
}
