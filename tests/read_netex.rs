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
extern crate zip;

use std::path::Path;

#[test]
fn ratp_line7bis() {
    let input_data = "fixtures/netex/RATP_Line7bis-extract-2009-NeTEx/input.zip";
    let read_result = navitia_model::netex::read(input_data, None, None);
    assert!(read_result.is_ok(), "{:?}", read_result.err().unwrap());
    navitia_model::ntfs::write_to_zip(
        &read_result.unwrap(),
        &Path::new("fixtures/netex/RATP_Line7bis-extract-2009-NeTEx/result_to_check.zip"),
    ).unwrap()
}

#[test]
fn read_netex_oslo() {
    let input_data = "fixtures/netex/Full_PublicationDelivery_109_Oslo_morningbus_example.xml";
    let read_result = navitia_model::netex::read(Path::new(input_data), None, None);
    assert!(read_result.is_ok(), "{:?}", read_result.err().unwrap());
    navitia_model::ntfs::write_to_zip(
        &read_result.unwrap(),
        &Path::new("fixtures/netex/Full_PublicationDelivery_109_Oslo_morningbus_example_result_to_check.zip"),
    ).unwrap()
}


