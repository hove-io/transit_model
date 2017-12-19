use std::path;
use csv;
use serde;

use collection::{Collection, Id};
use {Collections, PtObjects};

fn make_collection<T>(path: &path::Path, file: &str) -> Collection<T>
where
    T: Id<T>,
    for<'de> T: serde::Deserialize<'de>,
{
    let mut lines_rdr = csv::Reader::from_path(path.join(file)).unwrap();
    let lines = lines_rdr.deserialize().map(Result::unwrap).collect();
    Collection::from_vec(lines)
}

pub fn read<P: AsRef<path::Path>>(path: P) -> PtObjects {
    let path = path.as_ref();
    let mut collections = Collections::default();
    collections.commercial_modes = make_collection(path, "commercial_modes.txt");
    collections.lines = make_collection(path, "lines.txt");
    collections.routes = make_collection(path, "routes.txt");
    PtObjects::new(collections)
}
