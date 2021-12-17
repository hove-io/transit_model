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

use std::path::Path;
use time::{Date, Month};
use transit_model::model::Model;
use transit_model::test_utils::*;

#[test]
fn test_restrict_global() {
    test_in_tmp_dir(|path| {
        let objects =
            transit_model::ntfs::read(Path::new("./tests/fixtures/restrict-validity-period/input"))
                .unwrap();
        let mut collections = objects.into_collections();
        collections
            .restrict_period(
                Date::from_calendar_date(2018, Month::May, 1).unwrap(),
                Date::from_calendar_date(2018, Month::August, 5).unwrap(),
            )
            .unwrap();
        let new_model = Model::new(collections).unwrap();
        transit_model::ntfs::write(&new_model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            None,
            "./tests/fixtures/restrict-validity-period/output/",
        );
    });
}

#[test]
fn test_restrict_no_panic() {
    test_in_tmp_dir(|path| {
        let objects =
            transit_model::ntfs::read(Path::new("./tests/fixtures/restrict-validity-period/input"))
                .unwrap();
        let mut collections = objects.into_collections();
        collections
            .restrict_period(
                Date::from_calendar_date(2018, Month::August, 2).unwrap(),
                Date::from_calendar_date(2019, Month::July, 31).unwrap(),
            )
            .unwrap();
        let new_model = Model::new(collections).unwrap();
        transit_model::ntfs::write(&new_model, path, get_test_datetime()).unwrap();
    });
}
