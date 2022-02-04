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

use assert_cmd::prelude::*;
use ntfs2gtfs::add_mode_to_line_code;
use std::process::Command;
use tempfile::TempDir;
use transit_model::{test_utils::*, Model};

#[test]
fn test_stop_zones_not_exported_and_cleaned() {
    test_in_tmp_dir(|path| {
        let input = "./tests/fixtures/input";
        let mut collections = transit_model::ntfs::read_collections(input).unwrap();
        collections.remove_stop_zones();
        collections.remove_route_points();
        let model = Model::new(collections).unwrap();
        transit_model::gtfs::write(model, path).unwrap();
        compare_output_dir_with_expected(&path, None, "./tests/fixtures/output");
    });
}

#[test]
fn test_mode_in_route_shortname() {
    test_in_tmp_dir(|path| {
        let input = "./tests/fixtures/input";
        let model = transit_model::ntfs::read(input).unwrap();
        let model = add_mode_to_line_code(model).unwrap();
        transit_model::gtfs::write(model, path).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec!["routes.txt"]),
            "./tests/fixtures/output_route_shortname_with_mode",
        );
    });
}

#[test]
fn test_platforms_preserving() {
    test_in_tmp_dir(|path| {
        let input = "./tests/fixtures/platforms/input";
        let model = transit_model::ntfs::read(input).unwrap();
        transit_model::gtfs::write(model, path).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec!["stops.txt"]),
            "./tests/fixtures/platforms/output",
        );
    });
}

#[test]
fn test_ntfs2gtfs() {
    let output_dir = TempDir::new().expect("create temp dir failed");
    Command::cargo_bin("ntfs2gtfs")
        .expect("Failed to find binary 'ntfs2gtfs'")
        .arg("--input")
        .arg("tests/fixtures/input/")
        .arg("--output")
        .arg(output_dir.path().to_str().unwrap())
        .assert()
        .success();
    assert!(output_dir.path().join("agency.txt").is_file())
}

#[test]
fn test_ntfs2gtfs_create_output_directory() {
    let output_dir = TempDir::new().expect("create temp dir failed");
    let unexisting_dir = output_dir.path().join("unexisting-folder");
    Command::cargo_bin("ntfs2gtfs")
        .expect("Failed to find binary 'ntfs2gtfs'")
        .arg("--input")
        .arg("tests/fixtures/input/")
        .arg("--output")
        .arg(unexisting_dir.to_str().unwrap())
        .assert()
        .success();
    assert!(unexisting_dir.join("agency.txt").is_file())
}

#[test]
fn test_ntfs2gtfs_create_zip() {
    let output_dir = TempDir::new().expect("create temp dir failed");
    let ntfs_zip = output_dir.path().join("ntfs.zip");
    assert!(!ntfs_zip.is_file());
    Command::cargo_bin("ntfs2gtfs")
        .expect("Failed to find binary 'ntfs2gtfs'")
        .arg("--input")
        .arg("tests/fixtures/input/")
        .arg("--output")
        .arg(ntfs_zip.to_str().unwrap())
        .assert()
        .success();
    assert!(ntfs_zip.is_file());
}

#[test]
fn test_ntfs2gtfs_create_foobar() {
    let output_dir = TempDir::new().expect("create temp dir failed");
    let ntfs_foobar = output_dir.path().join("ntfs.foobar");
    assert!(!ntfs_foobar.is_file());
    Command::cargo_bin("ntfs2gtfs")
        .expect("Failed to find binary 'ntfs2gtfs'")
        .arg("--input")
        .arg("tests/fixtures/input/")
        .arg("--output")
        .arg(ntfs_foobar.to_str().unwrap())
        .assert()
        .success();
    assert!(ntfs_foobar.join("agency.txt").is_file());
}

#[test]
fn test_ntfs2gtfs_split_route_by_mode() {
    let output_dir = TempDir::new().expect("create temp dir failed");
    Command::cargo_bin("ntfs2gtfs")
        .expect("Failed to find binary 'ntfs2gtfs'")
        .arg("--input")
        .arg("tests/fixtures/input_split_route_by_mode")
        .arg("--output")
        .arg(output_dir.path().to_str().unwrap())
        .assert()
        .success();
    compare_output_dir_with_expected(
        &output_dir,
        Some(vec!["routes.txt"]),
        "./tests/fixtures/output_split_route_by_mode",
    );
}
