use assert_cmd::prelude::*;
use std::process::Command;
use tempfile::TempDir;
use transit_model::test_utils::*;

#[test]
fn test_ntfs2netexfr() {
    let output_dir = TempDir::new().expect("create temp dir failed");
    Command::cargo_bin("ntfs2netexfr")
        .expect("Failed to find binary 'ntfs2netexfr'")
        .arg("--input")
        .arg("../tests/fixtures/netex_france/input_ntfs")
        .arg("--output")
        .arg(output_dir.path().to_str().unwrap())
        .arg("--participant")
        .arg("Participant")
        .arg("--current-datetime")
        .arg("2019-04-03T17:19:00Z")
        .assert()
        .success();
    compare_output_dir_with_expected(&output_dir, None, "../tests/fixtures/netex_france/output/");
    let network_folders = std::fs::read_dir(output_dir)
        .unwrap()
        .map(|dir_entry| dir_entry.unwrap())
        .map(|dir_entry| dir_entry.path())
        .filter(|path| path.is_dir());
    for network_folder in network_folders {
        let folder_name = network_folder.file_name().unwrap();
        let expected_folder = format!(
            "../tests/fixtures/netex_france/output/{}",
            folder_name.to_str().unwrap()
        );
        compare_output_dir_with_expected_content(&network_folder, None, &expected_folder);
    }
}

#[test]
fn test_ntfs2netexfr_create_zip() {
    let output_dir = TempDir::new().expect("create temp dir failed");
    let netexfr_zip = output_dir.path().join("netexfr.zip");
    assert!(!netexfr_zip.exists());
    Command::cargo_bin("ntfs2netexfr")
        .expect("Failed to find binary 'ntfs2netexfr'")
        .arg("--input")
        .arg("../tests/fixtures/netex_france/input_ntfs")
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
