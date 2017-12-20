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
    let args: Vec<_> = std::env::args().collect();
    let objects = navitia_model::ntfs::read(".");
    let from = objects.lines.get_idx(&args[1]).unwrap();
    println!("commercial_modes: {:?}", get(from, &objects.commercial_modes, &objects));
    println!("physical_modes: {:?}", get(from, &objects.physical_modes, &objects));
    //let json = serde_json::to_string(&*pt_objects).unwrap();
    //println!("{}", json);
}
