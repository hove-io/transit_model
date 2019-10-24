// SPDX-License-Identifier: AGPL-3.0-only
//
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
use transit_model::model::Model;
use transit_model::test_utils::*;

#[test]
fn test_merge_stop_areas_multi_steps() {
    test_in_tmp_dir(|path| {
        let paths = vec![
            Path::new("./tests/fixtures/merge-stop-areas/rule1.csv").to_path_buf(),
            Path::new("./tests/fixtures/merge-stop-areas/rule2.csv").to_path_buf(),
        ];
        let objects =
            transit_model::ntfs::read(Path::new("./tests/fixtures/merge-stop-areas/ntfs-to-merge"))
                .unwrap();
        let report_path = path.join("report.json");
        let collections = transit_model::merge_stop_areas::merge_stop_areas(
            objects.into_collections(),
            paths,
            200,
            Path::new(&report_path).to_path_buf(),
        )
        .unwrap();
        let new_model = Model::new(collections).unwrap();
        transit_model::ntfs::write(&new_model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec![
                "comment_links.txt",
                "comments.txt",
                "geometries.txt",
                "feed_infos.txt",
                "lines.txt",
                "object_codes.txt",
                "object_properties.txt",
                "stops.txt",
                "ticket_use_restrictions.txt",
                "report.json",
                "routes.txt",
            ]),
            "./tests/fixtures/merge-stop-areas/output/",
        );
    });
}
