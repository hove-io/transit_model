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

extern crate failure;
extern crate navitia_model;
extern crate tempdir;
extern crate zip;

use std::fs;
use std::io::Read;
use std::path::Path;
use tempdir::TempDir;
pub type Error = failure::Error;
pub type Result<T> = std::result::Result<T, Error>;

fn compare_ntfs_zips<P, T>(ntfs_zipfile1: P, ntfs_zipfile2: T) -> Result<()>
where
    P: AsRef<Path>,
    T: AsRef<Path>,
{
    let file1 = fs::File::open(ntfs_zipfile1.as_ref())?;
    let file2 = fs::File::open(ntfs_zipfile2.as_ref())?;
    let mut zip1 = zip::ZipArchive::new(file1)?;
    let mut zip2 = zip::ZipArchive::new(file2)?;
    assert_eq!(
        zip1.len(),
        zip2.len(),
        "Number of files in ZIP are different."
    );
    for i in 0..zip1.len() {
        let mut file1 = zip1.by_index(i)?;
        let mut file2 = zip2.by_name(file1.name())?;
        assert_eq!(
            file1.size(),
            file2.size(),
            "size of file {} is different.",
            file1.name()
        );
        let mut file1_content = vec![];
        let mut file2_content = vec![];
        file1.read_to_end(&mut file1_content)?;
        file2.read_to_end(&mut file2_content)?;
        assert!(
            file1_content == file2_content,
            "content of file {} is different",
            file1.name()
        );
    }
    Ok(())
}

#[test]
#[should_panic(expected = "No valid calendar in Netex Data")] // for the moment, reading calendars is not implemented
fn ratp_line7bis() {
    let input_data = "fixtures/netex/RATP_Line7bis-extract-2009-NeTEx.zip";
    let expected_result_file = "fixtures/netex/expected_result/ratp_result.zip";

    let read_result = navitia_model::netex::read(Path::new(input_data), None, None);
    assert!(read_result.is_ok(), "{:?}", read_result.err().unwrap());
    let tmp_dir = TempDir::new("netex_computed_result").unwrap();
    let file_path = tmp_dir.path().join("netex_computed_result_ratp.zip");
    navitia_model::ntfs::write_to_zip(&read_result.unwrap(), file_path.clone()).unwrap();
    compare_ntfs_zips(expected_result_file, file_path.as_path()).unwrap();
}

#[test]
#[should_panic(expected = "No valid calendar in Netex Data")] // for the moment, reading calendars is not implemented
fn read_netex_oslo() {
    let input_data = "fixtures/netex/Full_PublicationDelivery_109_Oslo_morningbus_example.xml";
    let expected_result_file = "fixtures/netex/expected_result/oslo_result.zip";

    let read_result = navitia_model::netex::read(Path::new(input_data), None, None);
    assert!(read_result.is_ok(), "{:?}", read_result.err().unwrap());
    let tmp_dir = TempDir::new("netex_computed_result").unwrap();
    let file_path = tmp_dir.path().join("netex_computed_result_oslo.zip");
    navitia_model::ntfs::write_to_zip(&read_result.unwrap(), file_path.clone()).unwrap();
    compare_ntfs_zips(expected_result_file, file_path.as_path()).unwrap();
}
