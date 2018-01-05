extern crate csv;
extern crate navitia_model;
#[macro_use]
extern crate serde_json;

fn main() {
    let objects = navitia_model::gtfs::read(".");
    let json_objs = json!(objects);
    println!("{:?}", json_objs.to_string());
}
