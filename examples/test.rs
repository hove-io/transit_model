extern crate navitia_model;
extern crate serde_json;

use navitia_model::collection::{Collection, Idx, Id};
use navitia_model::relations::IdxSet;
use navitia_model::{GetCorresponding, PtObjects};

fn get<T, U>(idx: Idx<T>, collection: &Collection<U>, objects: &PtObjects) -> Vec<String>
where
    U: Id<U>,
    IdxSet<T>: GetCorresponding<U>,
{
    let from = [idx].iter().cloned().collect();
    let to = objects.get_corresponding(&from);
    to.iter().map(|idx| collection[*idx].id().to_string()).collect()
}

fn main() {
    let objects = navitia_model::ntfs::read(".");

    for (from, stop_area) in objects.stop_areas.iter() {
        let cms = get(from, &objects.commercial_modes, &objects);
        let pms = get(from, &objects.physical_modes, &objects);
        let ns = get(from, &objects.networks, &objects);
        let cs = get(from, &objects.contributors, &objects);
        println!("{}: cms: {:?}, pms: {:?}, ns: {:?}, cs: {:?}", stop_area.id, cms, pms, ns, cs);
    }
}
