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

use chrono::{DateTime, FixedOffset};
use pretty_assertions::assert_eq;
use std::collections::BTreeSet;
use std::fs;
use std::fs::File;
use std::io::{prelude::*, BufReader};
use std::path;
use std::path::Path;
use tempfile::tempdir;

pub fn get_file_content<P: AsRef<Path>>(path: P) -> Vec<String> {
    let path = path.as_ref();
    let file = File::open(path).unwrap_or_else(|_| panic!("file {:?} not found", path));
    let reader = BufReader::new(file);
    let mut vec = Vec::new();
    for result_line in reader.lines() {
        let line =
            result_line.unwrap_or_else(|_| panic!("Cannot parse as a line in file {:?}", path));
        vec.push(line);
    }
    vec
}
pub fn get_lines_content<P: AsRef<Path>>(path: P) -> BTreeSet<String> {
    let path = path.as_ref();
    let file = File::open(path).unwrap_or_else(|_| panic!("file {:?} not found", path));
    let reader = BufReader::new(file);
    let mut set = BTreeSet::new();
    for result_line in reader.lines() {
        let line =
            result_line.unwrap_or_else(|_| panic!("Cannot parse as a line in file {:?}", path));
        set.insert(line);
    }
    set
}

fn get_files_to_compare<P>(output_dir: &P, files_to_check: Option<Vec<&str>>) -> Vec<String>
where
    P: AsRef<Path>,
{
    match files_to_check {
        None => fs::read_dir(output_dir.as_ref())
            .unwrap()
            .map(|file| file.unwrap())
            .filter(|file| file.path().is_file())
            .map(|file| file.file_name().into_string().unwrap())
            .collect(),
        Some(v) => v.iter().map(|&f| f.to_string()).collect(),
    }
}

pub fn compare_output_dir_with_expected<P: AsRef<Path>>(
    output_dir: &P,
    files_to_check: Option<Vec<&str>>,
    work_dir_expected: &str,
) {
    compare_output_dir_with_expected_lines(output_dir, files_to_check, work_dir_expected);
}

pub fn compare_output_dir_with_expected_lines<P: AsRef<Path>>(
    output_dir: &P,
    files_to_check: Option<Vec<&str>>,
    work_dir_expected: &str,
) {
    let files = get_files_to_compare(output_dir, files_to_check);
    for filename in files {
        let output_file_path = output_dir.as_ref().join(filename.clone());
        let output_contents = get_lines_content(output_file_path);

        let expected_file_path = format!("{}/{}", work_dir_expected, filename);
        let expected_contents = get_lines_content(expected_file_path);

        assert_eq!(expected_contents, output_contents);
    }
}

pub fn compare_output_dir_with_expected_content<P: AsRef<Path>>(
    output_dir: &P,
    files_to_check: Option<Vec<&str>>,
    work_dir_expected: &str,
) {
    let files = get_files_to_compare(output_dir, files_to_check);
    for filename in files {
        let output_file_path = output_dir.as_ref().join(filename.clone());
        let output_contents = get_file_content(output_file_path);

        let expected_file_path = format!("{}/{}", work_dir_expected, filename);
        let expected_contents = get_file_content(expected_file_path);

        assert_eq!(expected_contents, output_contents);
    }
}

pub fn create_file_with_content(path: &path::Path, file_name: &str, content: &str) {
    let file_path = path.join(file_name);
    let mut f = File::create(&file_path).unwrap();
    f.write_all(content.as_bytes()).unwrap();
}

pub fn test_in_tmp_dir<F>(func: F)
where
    F: FnOnce(&path::Path),
{
    let tmp_dir = tempdir().expect("create temp dir");
    {
        let path = tmp_dir.as_ref();
        func(path);
    }
    tmp_dir.close().expect("delete temp dir");
}

pub fn get_test_datetime() -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339("2019-04-03T17:19:00Z").unwrap()
}
