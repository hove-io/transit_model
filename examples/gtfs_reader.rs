extern crate csv;
extern crate navitia_model;

fn main() {
    let objects = navitia_model::gtfs::read("./examples/data/");
    println!("Count of networks loaded : {}", objects.networks.len());
}
