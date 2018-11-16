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
use navitia_model::transfers;
use navitia_model::transfers::TransfersMode;
use std::path::Path;
extern crate tempdir;
use self::tempdir::TempDir;
#[path = "utils.rs"]
mod utils;

use utils::compare_output_dir_with_expected;

#[test]
//                    206m
// sp_1 *--------------------------------* sp_3
//       \                        ______/
//        \                  ____/
//   65m   \           _____/   146m
//          \    _____/
//           \__/
//           sp_2
//
fn test_generates_transfers() {
    let tmp_dir = TempDir::new("navitia_model_tests").expect("create temp dir");
    let input_dir = "fixtures/transfers/input";
    let mut model = navitia_model::ntfs::read(input_dir).unwrap();
    let rules: Vec<Box<Path>> = vec![];
    transfers::generates_transfers(
        &mut model,
        100.0,
        0.785,
        120,
        rules,
        &TransfersMode::IntraContributor,
        None,
    ).unwrap();
    navitia_model::ntfs::write(&model, tmp_dir.path()).unwrap();
    compare_output_dir_with_expected(
        tmp_dir.path(),
        &vec!["transfers.txt".to_string()],
        "./fixtures/transfers/output".to_string(),
    );
}

#[test]
fn test_generates_transfers_with_modification_rules() {
    let tmp_dir = TempDir::new("navitia_model_tests").expect("create temp dir");
    let input_dir = "fixtures/transfers/input";
    let mut model = navitia_model::ntfs::read(input_dir).unwrap();
    let rules = vec![Path::new("./fixtures/transfers/rules.txt").to_path_buf()];
    transfers::generates_transfers(
        &mut model,
        100.0,
        0.785,
        120,
        rules,
        &TransfersMode::IntraContributor,
        None,
    ).unwrap();
    navitia_model::ntfs::write(&model, tmp_dir.path()).unwrap();
    compare_output_dir_with_expected(
        tmp_dir.path(),
        &vec!["transfers.txt".to_string()],
        "./fixtures/transfers/output_rules".to_string(),
    );
}
