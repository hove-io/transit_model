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

use transit_model::{test_utils::*, transfers};

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
    test_in_tmp_dir(|path| {
        let input_dir = "tests/fixtures/transfers/mono_contributor/input";
        let model = transit_model::ntfs::read(input_dir).unwrap();
        let model = transfers::generates_transfers(model, 100.0, 0.785, 120, None).unwrap();
        transit_model::ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec!["transfers.txt"]),
            "./tests/fixtures/transfers/mono_contributor/output",
        );
    });
}

#[test]
fn test_generates_all_multi_contributors_transfers() {
    test_in_tmp_dir(|path| {
        let input_dir = "tests/fixtures/transfers/multi_contributors/input";
        let model = transit_model::ntfs::read(input_dir).unwrap();
        let model = transfers::generates_transfers(model, 100.0, 0.785, 120, None).unwrap();
        transit_model::ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec!["transfers.txt"]),
            "./tests/fixtures/transfers/multi_contributors/output",
        );
    });
}

#[test]
fn test_generates_transfers_with_closure_inter_contributors() {
    test_in_tmp_dir(|path| {
        use std::collections::BTreeSet;
        use transit_model::{
            objects::{Contributor, StopPoint},
            Model,
        };
        use typed_index_collection::Idx;
        let inter_contrib_tranfers = Box::new(
            |model: &Model, from_idx: Idx<StopPoint>, to_idx: Idx<StopPoint>| -> bool {
                let from_contributor: BTreeSet<Idx<Contributor>> =
                    model.get_corresponding_from_idx(from_idx);
                let to_contributor: BTreeSet<Idx<Contributor>> =
                    model.get_corresponding_from_idx(to_idx);

                if from_contributor.is_empty() || to_contributor.is_empty() {
                    return false;
                }

                from_contributor != to_contributor
            },
        );

        let input_dir = "tests/fixtures/transfers/multi_contributors/input";
        let model = transit_model::ntfs::read(input_dir).unwrap();
        let model =
            transfers::generates_transfers(model, 100.0, 0.785, 120, Some(inter_contrib_tranfers))
                .unwrap();
        transit_model::ntfs::write(&model, path, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &path,
            Some(vec!["transfers.txt"]),
            "./tests/fixtures/transfers/multi_contributors/output_closure_inter_contributor",
        );
    });
}
