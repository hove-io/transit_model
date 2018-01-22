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

extern crate navitia_model;
extern crate tempdir;

use navitia_model::gtfs;
use tempdir::TempDir;
use std::fs::File;
use std::io::prelude::*;

#[test]
fn load_minimal_agency() {
    let agency_content = "agency_name,agency_url,agency_timezone\n
    My agency,http://my-agency_url.com,Europe/London";
    let tmp_dir = TempDir::new("navitia_model_tests").expect("create temp dir");
    let file_path = tmp_dir.path().join("agency.txt");
    let mut f = File::create(&file_path).unwrap();
    f.write_all(agency_content.as_bytes()).unwrap();

    let (networks, companies) = gtfs::read_agency(tmp_dir.path());
    tmp_dir.close().expect("delete temp dir");
    assert_eq!(1, networks.len());
    let agency = networks.iter().next().unwrap().1;
    assert_eq!("default_agency_id", agency.id);
    assert_eq!(1, companies.len());
}

#[test]
fn load_standard_agency() {
    let agency_content = "agency_id,agency_name,agency_url,agency_timezone\n
id_1,My agency,http://my-agency_url.com,Europe/London";
    let tmp_dir = TempDir::new("navitia_model_tests").expect("create temp dir");
    let file_path = tmp_dir.path().join("agency.txt");
    let mut f = File::create(&file_path).unwrap();
    f.write_all(agency_content.as_bytes()).unwrap();

    let (networks, companies) = gtfs::read_agency(tmp_dir.path());
    tmp_dir.close().expect("delete temp dir");
    assert_eq!(1, networks.len());
    assert_eq!(1, companies.len());
}

#[test]
fn load_complete_agency() {
    let agency_content = "agency_id,agency_name,agency_url,agency_timezone,agency_lang,agency_phone,agency_fare_url,agency_email\n
id_1,My agency,http://my-agency_url.com,Europe/London,EN,0123456789,http://my-agency_fare_url.com,my-mail@example.com";
    let tmp_dir = TempDir::new("navitia_model_tests").expect("create temp dir");
    let file_path = tmp_dir.path().join("agency.txt");
    let mut f = File::create(&file_path).unwrap();
    f.write_all(agency_content.as_bytes()).unwrap();

    let (networks, companies) = gtfs::read_agency(tmp_dir.path());
    tmp_dir.close().expect("delete temp dir");
    assert_eq!(1, networks.len());
    let network = networks.iter().next().unwrap().1;
    assert_eq!("id_1", network.id);
    assert_eq!(1, companies.len());
}

#[test]
#[should_panic]
fn load_2_agencies_with_no_id() {
    let agency_content = "agency_name,agency_url,agency_timezone\n
My agency 1,http://my-agency_url.com,Europe/London
My agency 2,http://my-agency_url.com,Europe/London";
    let tmp_dir = TempDir::new("navitia_model_tests").expect("create temp dir");
    let file_path = tmp_dir.path().join("agency.txt");
    let mut f = File::create(&file_path).unwrap();
    f.write_all(agency_content.as_bytes()).unwrap();
    gtfs::read_agency(tmp_dir.path());
    tmp_dir.close().expect("delete temp dir");
}
