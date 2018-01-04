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
    let tmp_dir = TempDir::new("osm_transit_extractor").expect("create temp dir");
    let file_path = tmp_dir.path().join("agency.txt");
    let mut f = File::create(&file_path).unwrap();
    f.write_all(agency_content.as_bytes()).unwrap();

    let networks = gtfs::read_agency(tmp_dir.path());
    tmp_dir.close().expect("delete temp dir");
    assert_eq!(1, networks.len());
}

#[test]
fn load_standard_agency() {
    let agency_content = "agency_id,agency_name,agency_url,agency_timezone\n
    id_1,My agency,http://my-agency_url.com,Europe/London";
    let tmp_dir = TempDir::new("osm_transit_extractor").expect("create temp dir");
    let file_path = tmp_dir.path().join("agency.txt");
    let mut f = File::create(&file_path).unwrap();
    f.write_all(agency_content.as_bytes()).unwrap();

    let networks = gtfs::read_agency(tmp_dir.path());
    tmp_dir.close().expect("delete temp dir");
    assert_eq!(1, networks.len());
}
