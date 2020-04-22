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

use std::fs;
#[cfg(feature = "xmllint")]
use std::{ffi::OsStr, path::Path, process::Command};
use transit_model::{self, model::Model, netex_france, test_utils::*};

fn test_write_netex_france(model: Model) {
    test_in_tmp_dir(|output_dir| {
        let participant_ref = String::from("Participant");
        let stop_provider_code = Some(String::from("ProviderCode"));
        let netex_france_exporter = netex_france::Exporter::new(
            &model,
            participant_ref,
            stop_provider_code,
            get_test_datetime(),
        );
        netex_france_exporter.write(output_dir).unwrap();
        compare_output_dir_with_expected_content(
            &output_dir,
            None,
            "tests/fixtures/netex_france/output",
        );
        let network_folders = fs::read_dir(output_dir)
            .unwrap()
            .map(|dir_entry| dir_entry.unwrap())
            .map(|dir_entry| dir_entry.path())
            .filter(|path| path.is_dir());
        for network_folder in network_folders {
            let folder_name = network_folder.file_name().unwrap();
            let expected_folder = format!(
                "tests/fixtures/netex_france/output/{}",
                folder_name.to_str().unwrap()
            );
            compare_output_dir_with_expected_content(&network_folder, None, &expected_folder);
        }
    });
}

#[test]
fn test_write_netex_france_from_ntfs() {
    let model = transit_model::ntfs::read("tests/fixtures/netex_france/input_ntfs").unwrap();
    test_write_netex_france(model);
}

#[test]
fn test_write_netex_france_from_gtfs() {
    let model = transit_model::gtfs::read_from_path(
        "tests/fixtures/netex_france/input_gtfs",
        None,
        None,
        false,
        None,
    )
    .unwrap();
    test_write_netex_france(model);
}

#[test]
#[cfg(feature = "xmllint")]
fn validate_xml_schemas() {
    fn check_xml_in_folder<P>(path: P)
    where
        P: AsRef<Path>,
    {
        let paths = fs::read_dir(path)
            .unwrap()
            .map(|result| result.unwrap())
            .map(|dir_entry| dir_entry.path())
            .filter(|path| path.extension() == Some(&OsStr::new("xml")));
        for path in paths {
            let status = Command::new("xmllint")
                .arg("--noout")
                .arg("--nonet")
                .arg("--huge")
                .args(&["--schema", "tests/NeTEx/xsd/NeTEx_publication.xsd"])
                .arg(path)
                .status()
                .unwrap();
            assert!(status.success());
        }
    }

    check_xml_in_folder("tests/fixtures/netex_france/output/");
    let network_paths = fs::read_dir("tests/fixtures/netex_france/output/")
        .unwrap()
        .map(|result| result.unwrap())
        .map(|dir_entry| dir_entry.path())
        .filter(|path| path.is_dir());
    for network_path in network_paths {
        check_xml_in_folder(network_path);
    }
}
