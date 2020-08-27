// Copyright (C) 2020 Kisio Digital and/or its affiliates.
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

use relational_types::IdxSet;
use transit_model::model::GetCorresponding;
use transit_model::{Model, Result};
use typed_index_collection::{CollectionWithId, Id, Idx};

fn get<T, U>(idx: Idx<T>, collection: &CollectionWithId<U>, objects: &Model) -> Vec<String>
where
    U: Id<U>,
    IdxSet<T>: GetCorresponding<U>,
{
    objects
        .get_corresponding_from_idx(idx)
        .iter()
        .map(|idx| collection[*idx].id().to_string())
        .collect()
}

fn run() -> Result<()> {
    // load ntfs from current directory
    let transit_objects = transit_model::ntfs::read(".")?;

    // stop_area by stop_area, print PT objects related to it
    for (idx, stop_area) in &transit_objects.stop_areas {
        // retrieve idx from id
        assert_eq!(
            transit_objects.stop_areas.get_idx(&stop_area.id).unwrap(),
            idx
        );

        // lines passing by stop
        let lines = get(idx, &transit_objects.lines, &transit_objects);
        // physical_modes stopping at stop
        let pms = get(idx, &transit_objects.physical_modes, &transit_objects);
        // networks using stop
        let ns = get(idx, &transit_objects.networks, &transit_objects);
        // contributors providing the data for stop
        let cs = get(idx, &transit_objects.contributors, &transit_objects);
        // access stop_area through its idx to get name
        let stop_name = &transit_objects.stop_areas[idx].name;
        println!(
            "stop_area {} ({}): lines: {:?}, physical_modes: {:?}, networks: {:?}, contributors: {:?}, codes: {:?}",
            stop_area.id, stop_name, lines, pms, ns, cs, stop_area.codes
        );
    }
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        for cause in err.iter_chain() {
            eprintln!("{}", cause);
        }
        std::process::exit(1);
    }
}
