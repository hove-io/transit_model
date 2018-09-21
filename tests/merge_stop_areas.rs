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

#[cfg(test)]
mod tests {
    extern crate navitia_model;

    use std::path::Path;
    use tests::navitia_model::merge_stop_areas::*;

    #[test]
    fn test_read_rules() {
        let paths = vec![
            Path::new("./fixtures/merge-stop-areas/rule1.csv"),
            Path::new("./fixtures/merge-stop-areas/rule2.csv"),
        ];
        let mut rules = read_rules(paths);
        assert_eq!(rules.len(), 4);
        rules.sort();
        println!("{:?}", rules);
        assert_eq!(
            rules[0],
            StopAreaGroupRule {
                master_stop_area_id: "SA:01".to_string(),
                to_merge_stop_area_ids: vec!["SA:02".to_string(), "SA:04".to_string()]
            }
        );
        assert_eq!(
            rules[1],
            StopAreaGroupRule {
                master_stop_area_id: "SA:05".to_string(),
                to_merge_stop_area_ids: vec!["SA:06".to_string()]
            }
        );
        assert_eq!(
            rules[2],
            StopAreaGroupRule {
                master_stop_area_id: "SA:11".to_string(),
                to_merge_stop_area_ids: vec!["SA:10".to_string()]
            }
        );
        assert_eq!(
            rules[3],
            StopAreaGroupRule {
                master_stop_area_id: "SA:12".to_string(),
                to_merge_stop_area_ids: vec![]
            }
        );
    }
}
