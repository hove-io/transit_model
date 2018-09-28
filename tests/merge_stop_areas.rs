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

    use std::fs::File;
    use std::io::prelude::*;
    use std::path::Path;
    use tests::navitia_model::merge_stop_areas::*;
    use tests::navitia_model::model::Model;

    fn compare_output_dir_with_expected(output_dir: String) {
        for filename in vec![
            "comment_links.txt",
            "comments.txt",
            "geometries.txt",
            "lines.txt",
            "object_codes.txt",
            "object_properties.txt",
            "stops.txt",
            "report.json",
        ] {
            let output_file_path = format!("{}/{}", output_dir, filename);
            let mut output_file = File::open(output_file_path.clone())
                .expect(&format!("file {} not found", output_file_path));
            let mut output_contents = String::new();
            output_file.read_to_string(&mut output_contents).unwrap();
            let expected_file_path = format!("./fixtures/merge-stop-areas/output/{}", filename);
            let mut expected_file = File::open(expected_file_path.clone())
                .expect(&format!("file {} not found", expected_file_path));
            let mut expected_contents = String::new();
            expected_file
                .read_to_string(&mut expected_contents)
                .unwrap();
            assert_eq!(output_contents, expected_contents);
        }
    }

    #[test]
    fn test_merge_stop_areas_multi_steps() {
        let paths = vec![
            Path::new("./fixtures/merge-stop-areas/rule1.csv").to_path_buf(),
            Path::new("./fixtures/merge-stop-areas/rule2.csv").to_path_buf(),
        ];
        let objects =
            navitia_model::ntfs::read(Path::new("./fixtures/merge-stop-areas/ntfs-to-merge"))
                .unwrap();
        let collections = navitia_model::merge_stop_areas::merge_stop_areas(
            objects.into_collections(),
            paths,
            200,
            Path::new("./fixtures/output/report.json").to_path_buf(),
        );
        let new_model = Model::new(collections).unwrap();
        navitia_model::ntfs::write(&new_model, "./fixtures/output").unwrap();
        compare_output_dir_with_expected("./fixtures/output".to_string());
    }
}
