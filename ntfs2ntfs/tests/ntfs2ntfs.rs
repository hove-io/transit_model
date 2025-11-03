use assert_cmd::{cargo_bin, prelude::*};
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_ntfs2ntfs() {
    let output_dir = TempDir::new().expect("create temp dir failed");
    Command::new(cargo_bin!("ntfs2ntfs"))
        .arg("--input")
        .arg("../tests/fixtures/minimal_ntfs/")
        .arg("--output")
        .arg(output_dir.path().to_str().unwrap())
        .arg("--current-datetime")
        .arg("2019-04-03T17:19:00Z")
        .assert()
        .success();
    assert!(output_dir.path().join("feed_infos.txt").is_file())
}

#[test]
fn test_ntfs2ntfs_create_output_directory() {
    let output_dir = TempDir::new().expect("create temp dir failed");
    let unexisting_dir = output_dir.path().join("unexisting-folder");
    Command::new(cargo_bin!("ntfs2ntfs"))
        .arg("--input")
        .arg("../tests/fixtures/minimal_ntfs/")
        .arg("--output")
        .arg(unexisting_dir.to_str().unwrap())
        .arg("--current-datetime")
        .arg("2019-04-03T17:19:00Z")
        .assert()
        .success();
    assert!(unexisting_dir.join("feed_infos.txt").is_file())
}

#[test]
fn test_ntfs2ntfs_create_zip() {
    let output_dir = TempDir::new().expect("create temp dir failed");
    let ntfs_zip = output_dir.path().join("ntfs.zip");
    assert!(!ntfs_zip.exists());
    Command::new(cargo_bin!("ntfs2ntfs"))
        .arg("--input")
        .arg("../tests/fixtures/minimal_ntfs/")
        .arg("--output")
        .arg(ntfs_zip.to_str().unwrap())
        .arg("--current-datetime")
        .arg("2019-04-03T17:19:00Z")
        .assert()
        .success();
    assert!(ntfs_zip.is_file());
}

#[test]
fn test_ntfs2ntfs_create_foobar() {
    let output_dir = TempDir::new().expect("create temp dir failed");
    let ntfs_foobar = output_dir.path().join("ntfs.foobar");
    assert!(!ntfs_foobar.exists());
    Command::new(cargo_bin!("ntfs2ntfs"))
        .arg("--input")
        .arg("../tests/fixtures/minimal_ntfs/")
        .arg("--output")
        .arg(ntfs_foobar.to_str().unwrap())
        .arg("--current-datetime")
        .arg("2019-04-03T17:19:00Z")
        .assert()
        .success();
    assert!(ntfs_foobar.join("feed_infos.txt").is_file());
}

#[test]
fn test_ntfs2ntfs_without_transfers() {
    let output_dir = TempDir::new().expect("create temp dir failed");
    Command::new(cargo_bin!("ntfs2ntfs"))
        .arg("--input")
        .arg("../tests/fixtures/minimal_ntfs/")
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
