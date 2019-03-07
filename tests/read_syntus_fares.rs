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

use navitia_model::model::Model;
use navitia_model::syntus_fares;
use navitia_model::test_utils::*;
use std::path::Path;

#[test]
fn test_read_global() {
    test_in_tmp_dir(|path| {
        let objects =
            navitia_model::ntfs::read(Path::new("./fixtures/read-syntus-fares/input/ntfs"))
                .unwrap();
        let (tickets, od_rules, fares) = syntus_fares::read(
            Path::new("./fixtures/read-syntus-fares/input/syntus_fares_ok"),
            &objects.stop_points,
        )
        .unwrap();
        let mut collections = objects.into_collections();
        collections.tickets = tickets;
        collections.od_rules = od_rules;
        collections.fares = fares;
        let new_model = Model::new(collections).unwrap();
        navitia_model::ntfs::write(&new_model, path).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec!["fares.csv", "od_fares.csv", "prices.csv"]),
            "./fixtures/read-syntus-fares/output/",
        );
    });
}

#[test]
#[should_panic(expected = "no UnitPrice FareFrame found for the DistanceMatrix FareFrame")]
fn test_read_ko_no_unit_price() {
    let objects =
        navitia_model::ntfs::read(Path::new("./fixtures/read-syntus-fares/input/ntfs")).unwrap();
    syntus_fares::read(
        Path::new("./fixtures/read-syntus-fares/input/syntus_fares_ko_no_unit"),
        &objects.stop_points,
    )
    .unwrap();
}

#[test]
#[should_panic(
    expected = "unable to pick a reference UnitPrice FareFrame for the DistanceMatrix FareFrame"
)]
fn test_read_ko_several_unit_prices() {
    let objects =
        navitia_model::ntfs::read(Path::new("./fixtures/read-syntus-fares/input/ntfs")).unwrap();
    syntus_fares::read(
        Path::new("./fixtures/read-syntus-fares/input/syntus_fares_ko_several_unit"),
        &objects.stop_points,
    )
    .unwrap();
}
