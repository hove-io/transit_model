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

extern crate env_logger;
extern crate failure;
extern crate navitia_model;
extern crate serde_json;

use navitia_model::collection::{CollectionWithId, Id, Idx};
use navitia_model::model::GetCorresponding;
use navitia_model::relations::IdxSet;
use navitia_model::{Model, Result};

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
    let objects = navitia_model::ntfs::read(".")?;

    for (from, stop_area) in objects.stop_areas.iter() {
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
    env_logger::init();
    if let Err(err) = run() {
        for cause in err.causes() {
            eprintln!("{}", cause);
        }
        std::process::exit(1);
    }
}
