// Copyright 2017 Kisio Digital and/or its affiliates.
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

use std::path::Path;
use transit_model::hellogo_fares;
use transit_model::model::Model;
use transit_model::test_utils::*;

#[test]
fn test_read_global_with_prefix() {
    test_in_tmp_dir(|path| {
        let objects = transit_model::ntfs::read(Path::new(
            "./tests/fixtures/enrich-with-hellogo-fares/input/ntfs_with_prefix",
        ))
        .unwrap();
        let mut collections = objects.into_collections();
        hellogo_fares::enrich_with_hellogo_fares(
            &mut collections,
            Path::new("./tests/fixtures/enrich-with-hellogo-fares/input/hellogo_fares_ok"),
        )
        .unwrap();
        let new_model = Model::new(collections).unwrap();
        transit_model::ntfs::write(&new_model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec![
                "tickets.txt",
                "ticket_uses.txt",
                "ticket_prices.txt",
                "ticket_use_perimeters.txt",
                "ticket_use_restrictions.txt",
            ]),
            "./tests/fixtures/enrich-with-hellogo-fares/output/ntfs_fares_with_prefix",
        );
    });
}

#[test]
fn test_read_global_without_prefix() {
    test_in_tmp_dir(|path| {
        let objects = transit_model::ntfs::read(Path::new(
            "./tests/fixtures/enrich-with-hellogo-fares/input/ntfs_without_prefix",
        ))
        .unwrap();
        let mut collections = objects.into_collections();
        hellogo_fares::enrich_with_hellogo_fares(
            &mut collections,
            Path::new("./tests/fixtures/enrich-with-hellogo-fares/input/hellogo_fares_ok"),
        )
        .unwrap();
        let new_model = Model::new(collections).unwrap();
        transit_model::ntfs::write(&new_model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec![
                "tickets.txt",
                "ticket_uses.txt",
                "ticket_prices.txt",
                "ticket_use_perimeters.txt",
                "ticket_use_restrictions.txt",
            ]),
            "./tests/fixtures/enrich-with-hellogo-fares/output/ntfs_fares_without_prefix",
        );
    });
}

#[test]
#[should_panic(expected = "Failed to find a \\'UnitPrice\\' fare frame in the Netex file")]
fn test_read_ko_no_unit_price() {
    let objects = transit_model::ntfs::read(Path::new(
        "./tests/fixtures/enrich-with-hellogo-fares/input/ntfs_with_prefix",
    ))
    .unwrap();
    let mut collections = objects.into_collections();
    hellogo_fares::enrich_with_hellogo_fares(
        &mut collections,
        Path::new("./tests/fixtures/enrich-with-hellogo-fares/input/hellogo_fares_ko_no_unit"),
    )
    .unwrap();
}

#[test]
#[should_panic(expected = "Failed to find a unique \\'UnitPrice\\' fare frame in the Netex file")]
fn test_read_ko_several_unit_prices() {
    let objects = transit_model::ntfs::read(Path::new(
        "./tests/fixtures/enrich-with-hellogo-fares/input/ntfs_with_prefix",
    ))
    .unwrap();
    let mut collections = objects.into_collections();
    hellogo_fares::enrich_with_hellogo_fares(
        &mut collections,
        Path::new("./tests/fixtures/enrich-with-hellogo-fares/input/hellogo_fares_ko_several_unit"),
    )
    .unwrap();
}
