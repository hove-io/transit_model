extern crate csv;
extern crate navitia_model;

fn main() {
    let objects = navitia_model::gtfs::read(".");
    println!("Count of networks loaded : {}", objects.networks.len());
}
