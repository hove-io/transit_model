extern crate navitia_model;
extern crate serde_json;

use navitia_model::collection::{Collection, Idx, Id};
use navitia_model::relations::{GetCorresponding, IdxSet};
use navitia_model::PtObjects;

fn get<T, U: Id<U>>(idx: Idx<T>, collection: &Collection<U>, objects: &PtObjects) -> Vec<String>
    where
    IdxSet<T>: GetCorresponding<U>
{
    let from: IdxSet<T> = [idx].iter().cloned().collect();
    let to: IdxSet<U> = from.get_corresponding(objects);
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
