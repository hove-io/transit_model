extern crate navitia_model;
extern crate serde_json;

use navitia_model::objects::*;
use navitia_model::relations::{GetCorresponding, IdxSet};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let pt_objects = navitia_model::ntfs::read(".");
    let first_route: IdxSet<Route> = pt_objects.routes.get_idx(&args[1]).into_iter().collect();
    let cm_from_route: IdxSet<CommercialMode> = first_route.get_corresponding(&pt_objects);
    let cm_from_route: Vec<_> = cm_from_route.iter().map(|idx| &pt_objects.commercial_modes[*idx].id).collect();
    println!("commercial_modes: {:?}", cm_from_route);
    //let json = serde_json::to_string(&*pt_objects).unwrap();
    //println!("{}", json);
}
