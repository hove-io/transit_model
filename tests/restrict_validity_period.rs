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

use chrono::NaiveDate;
use navitia_model::model::Model;
use navitia_model::test_utils::*;
use std::path::Path;

#[test]
fn test_restrict_global() {
    test_in_tmp_dir(|path| {
        let objects =
            navitia_model::ntfs::read(Path::new("./fixtures/restrict-validity-period/input"))
                .unwrap();
        let mut collections = objects.into_collections();
        collections
            .restrict_period(
                &NaiveDate::from_ymd(2018, 5, 1),
                &NaiveDate::from_ymd(2018, 8, 5),
            )
            .unwrap();
        collections.sanitize().unwrap();
        let new_model = Model::new(collections).unwrap();
        navitia_model::ntfs::write(&new_model, path).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec![
                "calendar_dates.txt",
                "companies.txt",
                "datasets.txt",
                "equipments.txt",
                "geometries.txt",
                "lines.txt",
                "networks.txt",
                "routes.txt",
                "stop_times.txt",
                "stops.txt",
                "trip_properties.txt",
                "trips.txt",
                "object_codes.txt",
                "object_properties.txt",
            ]),
            "./fixtures/restrict-validity-period/output/",
        );
    });
}
