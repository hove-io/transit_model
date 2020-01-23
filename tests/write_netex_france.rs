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

use transit_model::{self, netex_france, test_utils::*};

#[test]
fn test_write_netex_france() {
    let model = transit_model::ntfs::read("tests/fixtures/ntfs").unwrap();
    test_in_tmp_dir(|output_dir| {
        let participant_ref = String::from("Participant");
        let netex_france_exporter =
            netex_france::Exporter::new(model, participant_ref, get_test_datetime());
        netex_france_exporter.write(output_dir).unwrap();
        compare_output_dir_with_expected_content(&output_dir, None, "tests/fixtures/netex_france");
    });
}
