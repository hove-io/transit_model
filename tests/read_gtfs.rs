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

use pretty_assertions::assert_eq;

#[test]
fn simple_gtfs_reading() {
    let ntm = transit_model::gtfs::read("tests/fixtures/gtfs").unwrap();
    assert_eq!(ntm.stop_areas.len(), 2);
}

#[test]
fn ziped_gtfs_reading() {
    let ntm = transit_model::gtfs::read("tests/fixtures/ziped_gtfs/gtfs.zip").unwrap();
    assert_eq!(ntm.stop_areas.len(), 1);
}

#[test]
fn gtfs_with_config_reading() {
    let mut feed = std::collections::BTreeMap::<_, _>::default();
    feed.insert("pouet".to_owned(), "toto".to_owned());
    let c = transit_model::gtfs::Configuration {
        feed_infos: feed.clone(),
        ..Default::default()
    };
    let model = transit_model::gtfs::Reader::new(c)
        .parse("tests/fixtures/gtfs")
        .unwrap();
    assert_eq!(model.stop_areas.len(), 2);
    // we should find our custom feed info in the loaded model
    assert_eq!(model.feed_infos, feed);
}

#[test]
#[should_panic(
    expected = "ErrorMessage { msg: \"file \\\"tests/fixtures/i_m_not_here\\\" is neither a file nor a directory, cannot read a gtfs from it\" }"
)]
fn unexistent_file() {
    // reading a file that does not exists will lead to an error
    let _ = transit_model::gtfs::read("tests/fixtures/i_m_not_here").unwrap();
}

#[test]
#[should_panic(
    expected = "InvalidArchive(\"Could not find central directory end\")\n\nimpossible to read ziped gtfs \"tests/fixtures/gtfs/stops.txt\""
)]
fn file_not_a_gtfs() {
    // reading a file that is not either a directory with the gtfs files nor a zip archive will lead to an error
    // here we read the stops.txt
    let _ = transit_model::gtfs::read("tests/fixtures/gtfs/stops.txt").unwrap();
}

#[test]
#[should_panic(
    expected = "ErrorMessage { msg: \"calendar_dates.txt or calendar.txt not found\" }\n\nimpossible to read gtfs directory from \"tests/fixtures/netex_france\""
)]
fn directory_not_a_gtfs() {
    // reading a directory that does not contain the gtfs files will lead to an error
    let _ = transit_model::gtfs::read("tests/fixtures/netex_france").unwrap();
}
