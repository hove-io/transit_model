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

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use transit_model;
    use transit_model::apply_rules;
    use transit_model::test_utils::*;

    fn compare_report(report_path: PathBuf, fixture_report_output: PathBuf) {
        let output_contents = get_file_content(report_path);
        let expected_contents = get_file_content(fixture_report_output);
        assert_eq!(output_contents, expected_contents);
    }

    fn test_apply_rules(
        cc_rules_dir: &str,
        p_rules_dir: &str,
        n_consolidation: &str,
        fixture_output_dir: &str,
        fixture_report_output: &str,
    ) {
        test_in_tmp_dir(|path| {
            let mut file_to_compare = vec![
                "geometries.txt",
                "lines.txt",
                "routes.txt",
                "trips.txt",
                "stops.txt",
                "networks.txt",
            ];

            let input_dir = "fixtures/apply_rules/input";

            let mut cc_rules: Vec<PathBuf> = vec![];
            if !cc_rules_dir.is_empty() {
                cc_rules.push(Path::new(cc_rules_dir).to_path_buf());
                file_to_compare.push("object_codes.txt")
            }

            let mut p_rules: Vec<PathBuf> = vec![];
            if !p_rules_dir.is_empty() {
                p_rules.push(Path::new(p_rules_dir).to_path_buf());
            }

            let n_consolidation = Path::new(n_consolidation).to_path_buf();

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
            compare_output_dir_with_expected(&path, Some(file_to_compare), fixture_output_dir);
            compare_report(report_path, Path::new(fixture_report_output).to_path_buf());
        });
    }
    #[test]
    fn test_no_property_rules() {
        test_apply_rules(
            "",
            "",
            "",
            "./fixtures/apply_rules/output",
            "./fixtures/apply_rules/output_report/report.json",
        );
    }

    #[test]
    fn test_apply_complementary_codes() {
        test_apply_rules(
            "./fixtures/apply_rules/complementary_codes_rules.txt",
            "",
            "",
            "./fixtures/apply_rules/output_apply_complementary_codes",
            "./fixtures/apply_rules/output_report/report_apply_complementary_codes.json",
        );
    }

    #[test]
    fn test_apply_property() {
        test_apply_rules(
            "./fixtures/apply_rules/complementary_codes_rules.txt",
            "./fixtures/apply_rules/property_rules.txt",
            "",
            "./fixtures/apply_rules/output_apply_property",
            "./fixtures/apply_rules/output_report/report_apply_property.json",
        );
    }

    #[test]
    fn test_ntw_consolidation() {
        test_apply_rules(
            "",
            "",
            "./fixtures/apply_rules/ntw_consolidation.json",
            "./fixtures/apply_rules/output_consolidation",
            "./fixtures/apply_rules/output_report/report.json",
        );
    }

    #[test]
    #[should_panic]
    fn test_ntw_consolidation_unvalid() {
        test_apply_rules(
            "",
            "",
            "./fixtures/apply_rules/ntw_consolidation_unvalid.json",
            "./fixtures/apply_rules/output_consolidation_unvalid",
            "./fixtures/apply_rules/output_report/report.json",
        );
    }

    #[test]
    fn test_ntw_consolidation_with_object_code() {
        test_apply_rules(
            "./fixtures/apply_rules/complementary_codes_rules.txt",
            "./fixtures/apply_rules/property_rules.txt",
            "./fixtures/apply_rules/ntw_consolidation.json",
            "./fixtures/apply_rules/output_consolidation_with_object_code",
            "./fixtures/apply_rules/output_report/report_consolidation_with_object_code.json",
        );
    }

    #[test]
    fn test_ntw_consolidation_2_ntw() {
        test_apply_rules(
            "",
            "",
            "./fixtures/apply_rules/ntw_consolidation_2_ntw.json",
            "./fixtures/apply_rules/output_consolidation_2_ntw",
            "./fixtures/apply_rules/output_report/report.json",
        );
    }

    #[test]
    fn test_ntw_consolidation_2_diff_ntw() {
        test_apply_rules(
            "",
            "",
            "./fixtures/apply_rules/ntw_consolidation_2_diff_ntw.json",
            "./fixtures/apply_rules/output_consolidation_2_diff_ntw",
            "./fixtures/apply_rules/output_report/report.json",
        );
    }

    #[test]
    fn test_ntw_consolidation_unknown_id() {
        test_apply_rules(
            "",
            "",
            "./fixtures/apply_rules/ntw_consolidation_unknown_id.json",
            "./fixtures/apply_rules/output",
            "./fixtures/apply_rules/output_report/report_consolidation_unknown_id.json",
        );
    }

    #[test]
    #[should_panic]
    fn test_ntw_consolidation_duplicate_id() {
        test_apply_rules(
            "",
            "",
            "./fixtures/apply_rules/ntw_consolidation_duplicate_id.json",
            "./fixtures/apply_rules/output",
            "./fixtures/apply_rules/output_report/report_consolidation_duplicate_id.json",
        );
    }

    #[test]
    #[should_panic]
    fn test_ntw_consolidation_unvalid_network() {
        test_apply_rules(
            "",
            "",
            "./fixtures/apply_rules/ntw_consolidation_unvalid_network.json",
            "./fixtures/apply_rules/output",
            "./fixtures/apply_rules/output_report/report_consolidation_duplicate_id.json",
        );
    }

    #[test]
    fn test_ntw_consolidation_no_grouped_from() {
        test_apply_rules(
            "",
            "",
            "./fixtures/apply_rules/ntw_consolidation_no_grouped_from.json",
            "./fixtures/apply_rules/output",
            "./fixtures/apply_rules/output_report/report_consolidation_no_grouped_from.json",
        );
    }

    #[test]
    fn test_ntw_consolidation_empty_grouped_from() {
        test_apply_rules(
            "",
            "",
            "./fixtures/apply_rules/ntw_consolidation_empty_grouped_from.json",
            "./fixtures/apply_rules/output",
            "./fixtures/apply_rules/output_report/report_consolidation_empty_grouped_from.json",
        );
    }
}
