use assert_cmd::{cargo_bin, prelude::*};
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_gtfs2netexfr() {
    let output_dir = TempDir::new().expect("create temp dir failed");
    Command::new(cargo_bin!("gtfs2netexfr"))
        .arg("--input")
        .arg("../tests/fixtures/netex_france/input_gtfs")
        .arg("--output")
        .arg(output_dir.path().to_str().unwrap())
        .arg("--participant")
        .arg("Participant")
        .arg("--current-datetime")
        .arg("2019-04-03T17:19:00Z")
        .assert()
        .success();
    assert!(output_dir.path().join("arrets.xml").is_file());
}

#[test]
fn test_gtfs2netexfr_without_dir() {
    let output_dir = TempDir::new().expect("create temp dir failed");
    let unexisting_dir = output_dir.path().join("unexisting-dir");
    Command::new(cargo_bin!("gtfs2netexfr"))
        .arg("--input")
        .arg("../tests/fixtures/netex_france/input_gtfs")
        .arg("--output")
        .arg(unexisting_dir.to_str().unwrap())
        .arg("--participant")
        .arg("Participant")
        .arg("--current-datetime")
        .arg("2019-04-03T17:19:00Z")
        .assert()
        .success();
    assert!(unexisting_dir.join("arrets.xml").is_file());
}

#[test]
fn test_gtfs2netexfr_create_zip() {
    let output_dir = TempDir::new().expect("create temp dir failed");
    let netexfr_zip = output_dir.path().join("netexfr.zip");
    assert!(!netexfr_zip.exists());
    Command::new(cargo_bin!("gtfs2netexfr"))
        .arg("--input")
        .arg("../tests/fixtures/netex_france/input_gtfs")
        .arg("--output")
        .arg(netexfr_zip.to_str().unwrap())
        .arg("--participant")
        .arg("Participant")
        .arg("--current-datetime")
        .arg("2019-04-03T17:19:00Z")
        .assert()
        .success();
    assert!(netexfr_zip.is_file());
}

#[test]
fn test_gtfs2netexfr_create_not_zip_extension() {
    let output_dir = TempDir::new().expect("create temp dir failed");
    let netexfr_foobar = output_dir.path().join("netexfr.foobar");
    Command::new(cargo_bin!("gtfs2netexfr"))
        .arg("--input")
        .arg("../tests/fixtures/netex_france/input_gtfs")
        .arg("--output")
        .arg(netexfr_foobar.to_str().unwrap())
        .arg("--participant")
        .arg("Participant")
        .arg("--current-datetime")
        .arg("2019-04-03T17:19:00Z")
        .assert()
        .success();
    assert!(netexfr_foobar.join("arrets.xml").is_file());
}
