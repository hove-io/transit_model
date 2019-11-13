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

use transit_model::model::GetCorresponding;
use transit_model::{Model, Result};
use transit_model_collection::{CollectionWithId, Id, Idx};
use transit_model_relations::IdxSet;

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
    let objects = transit_model::ntfs::read(".")?;

    for (from, stop_area) in &objects.stop_areas {
        let cms = get(from, &objects.commercial_modes, &objects);
        let pms = get(from, &objects.physical_modes, &objects);
        let ns = get(from, &objects.networks, &objects);
        let cs = get(from, &objects.contributors, &objects);
        println!(
            "{}: cms: {:?}, pms: {:?}, ns: {:?}, cs: {:?}, codes: {:?}",
            stop_area.id, cms, pms, ns, cs, stop_area.codes
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
