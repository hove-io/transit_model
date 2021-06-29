use assert_cmd::prelude::*;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_ntfs2ntfs() {
    let output_dir = TempDir::new().expect("create temp dir failed");
    Command::cargo_bin("ntfs2ntfs")
        .expect("Failed to find binary 'ntfs2ntfs'")
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
    Command::cargo_bin("ntfs2ntfs")
        .expect("Failed to find binary 'ntfs2ntfs'")
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
    Command::cargo_bin("ntfs2ntfs")
        .expect("Failed to find binary 'ntfs2ntfs'")
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
