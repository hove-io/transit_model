use assert_cmd::prelude::*;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_gtfs2ntfs_flex() {
    std::env::set_var("NO_COLOR", "true");
    let output_dir = TempDir::new_in("./tests/fixtures/output").expect("create temp dir failed");
    Command::cargo_bin("gtfs2ntfs")
        .expect("Failed to find binary 'gtfs2ntfs'")
        .arg("--input")
        .arg("./tests/fixtures/gtfs_flex_v3_bois_le_roi")
        .arg("--output")
        .arg(output_dir.path().to_str().unwrap())
        .arg("--current-datetime")
        .arg("2019-04-03T17:19:00Z")
        .assert()
        .success();
    assert!(output_dir.path().join("feed_infos.txt").is_file());
    let collections = transit_model::ntfs::read(output_dir).unwrap();
    assert_eq!(53, collections.transfers.len());
}

#[test]
fn test_gtfs2ntfs() {
    std::env::set_var("RUST_LOG", "debug ");
    let output_dir = TempDir::new().expect("create temp dir failed");
    Command::cargo_bin("gtfs2ntfs")
        .expect("Failed to find binary 'gtfs2ntfs'")
        .arg("--input")
        .arg("../tests/fixtures/gtfs2ntfs/minimal/input")
        .arg("--output")
        .arg(output_dir.path().to_str().unwrap())
        .arg("--current-datetime")
        .arg("2019-04-03T17:19:00Z")
        .assert()
        .success();
    assert!(output_dir.path().join("feed_infos.txt").is_file());
    let collections = transit_model::ntfs::read(output_dir).unwrap();
    assert_eq!(81, collections.transfers.len());
}

#[test]
fn test_gtfs2ntfs_create_output_directory() {
    let output_dir = TempDir::new().expect("create temp dir failed");
    let unexisting_dir = output_dir.path().join("unexisting-folder");
    Command::cargo_bin("gtfs2ntfs")
        .expect("Failed to find binary 'gtfs2ntfs'")
        .arg("--input")
        .arg("../tests/fixtures/gtfs2ntfs/minimal/input")
        .arg("--output")
        .arg(unexisting_dir.to_str().unwrap())
        .arg("--current-datetime")
        .arg("2019-04-03T17:19:00Z")
        .assert()
        .success();
    assert!(unexisting_dir.join("feed_infos.txt").is_file());
}

#[test]
fn test_gtfs2ntfs_create_zip() {
    let output_dir = TempDir::new().expect("create temp dir failed");
    let ntfs_zip = output_dir.path().join("ntfs.zip");
    assert!(!ntfs_zip.exists());
    Command::cargo_bin("gtfs2ntfs")
        .expect("Failed to find binary 'gtfs2ntfs'")
        .arg("--input")
        .arg("../tests/fixtures/gtfs2ntfs/minimal/input")
        .arg("--output")
        .arg(ntfs_zip.to_str().unwrap())
        .arg("--current-datetime")
        .arg("2019-04-03T17:19:00Z")
        .assert()
        .success();
    assert!(ntfs_zip.is_file());
}

#[test]
fn test_gtfs2ntfs_create_not_zip_extension() {
    let output_dir = TempDir::new().expect("create temp dir failed");
    let ntfs_foobar = output_dir.path().join("ntfs.foobar");
    assert!(!ntfs_foobar.exists());
    Command::cargo_bin("gtfs2ntfs")
        .expect("Failed to find binary 'gtfs2ntfs'")
        .arg("--input")
        .arg("../tests/fixtures/gtfs2ntfs/minimal/input")
        .arg("--output")
        .arg(ntfs_foobar.to_str().unwrap())
        .arg("--current-datetime")
        .arg("2019-04-03T17:19:00Z")
        .assert()
        .success();
    assert!(ntfs_foobar.join("feed_infos.txt").is_file());
}

#[test]
fn test_gtfs2ntfs_without_transfers() {
    let output_dir = TempDir::new().expect("create temp dir failed");
    Command::cargo_bin("gtfs2ntfs")
        .expect("Failed to find binary 'gtfs2ntfs'")
        .arg("--input")
        .arg("../tests/fixtures/gtfs2ntfs/minimal/input")
        .arg("--output")
        .arg(output_dir.path().to_str().unwrap())
        .arg("--current-datetime")
        .arg("2019-04-03T17:19:00Z")
        .arg("--ignore-transfers")
        .assert()
        .success();
    assert!(output_dir.path().join("feed_infos.txt").is_file());
    let collections = transit_model::ntfs::read(output_dir).unwrap();
    assert_eq!(0, collections.transfers.len());
}
