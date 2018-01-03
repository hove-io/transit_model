extern crate navitia_model;
extern crate csv;

fn main() {
    let objects = navitia_model::gtfs::read("./examples/data/");
    println!("Count of networks loaded : {}", objects.networks.len())

}
