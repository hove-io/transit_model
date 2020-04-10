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

#![feature(test)]

extern crate test;

use chrono::NaiveDate;
use test::Bencher;
use transit_model::transxchange;

#[bench]
fn read_transxchange(bencher: &mut Bencher) {
    bencher.iter(|| {
        transxchange::read(
            "./tests/fixtures/transxchange2ntfs/input/transxchange",
            "./tests/fixtures/transxchange2ntfs/input/naptan",
            None,
            None,
            None,
            NaiveDate::from_ymd(2020, 4, 10),
        )
        .unwrap()
    });
}
