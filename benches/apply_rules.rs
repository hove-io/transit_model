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

#![feature(test)]

extern crate test;

use std::path::Path;
use test::Bencher;
use transit_model::{apply_rules, ntfs};

#[bench]
fn apply_rules_none(bencher: &mut Bencher) {
    bencher.iter(|| {
        apply_rules::apply_rules(
            ntfs::read("./tests/fixtures/apply_rules/input").unwrap(),
            vec![],
            vec![],
            None,
            Path::new("./tests/fixtures/apply_rules/output_report/report.json").to_path_buf(),
        )
        .unwrap()
    });
}

#[bench]
fn apply_rules_complementary_codes(bencher: &mut Bencher) {
    bencher.iter(|| {
        apply_rules::apply_rules(
            ntfs::read("./tests/fixtures/apply_rules/input").unwrap(),
            vec![
                Path::new("./tests/fixtures/apply_rules/complementary_codes_rules.txt")
                    .to_path_buf(),
            ],
            vec![],
            None,
            Path::new(
                "./tests/fixtures/apply_rules/output_report/report_apply_complementary_codes.json",
            )
            .to_path_buf(),
        )
        .unwrap()
    });
}

#[bench]
fn apply_rules_property(bencher: &mut Bencher) {
    bencher.iter(|| {
        apply_rules::apply_rules(
            ntfs::read("./tests/fixtures/apply_rules/input").unwrap(),
            vec![
                Path::new("./tests/fixtures/apply_rules/complementary_codes_rules.txt")
                    .to_path_buf(),
            ],
            vec![Path::new("./tests/fixtures/apply_rules/property_rules.txt").to_path_buf()],
            None,
            Path::new("./tests/fixtures/apply_rules/output_report/report_apply_property.json")
                .to_path_buf(),
        )
        .unwrap()
    });
}

#[bench]
fn apply_rules_network_consolidation(bencher: &mut Bencher) {
    bencher.iter(|| {
        apply_rules::apply_rules(
            ntfs::read("./tests/fixtures/apply_rules/input").unwrap(),
            vec![],
            vec![],
            Some(Path::new("./tests/fixtures/apply_rules/ntw_consolidation.json").to_path_buf()),
            Path::new("./tests/fixtures/apply_rules/output_report/report.json").to_path_buf(),
        )
        .unwrap()
    });
}
