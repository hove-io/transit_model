extern crate csv;
#[macro_use]
extern crate derivative;
#[macro_use]
extern crate get_corresponding_derive;
extern crate serde;
#[macro_use]
extern crate serde_derive;

pub mod collection;
pub mod objects;
pub mod relations;
pub mod ntfs;

use std::ops;

use collection::Collection;
use objects::*;
use relations::{GetCorresponding, IdxSet, OneToMany};

#[derive(Derivative, Serialize, Deserialize, Debug)]
#[derivative(Default)]
pub struct Collections {
    pub commercial_modes: Collection<CommercialMode>,
    pub lines: Collection<Line>,
    pub routes: Collection<Route>,
}

#[derive(GetCorresponding)]
pub struct PtObjects {
    collections: Collections,
    commercial_modes_to_lines: OneToMany<CommercialMode, Line>,
    lines_to_routes: OneToMany<Line, Route>,
}
impl PtObjects {
    pub fn new(collections: Collections) -> Self {
        PtObjects {
            commercial_modes_to_lines: OneToMany::new(
                &collections.commercial_modes,
                &collections.lines,
            ),
            lines_to_routes: OneToMany::new(&collections.lines, &collections.routes),
            collections: collections,
        }
    }
}
impl ops::Deref for PtObjects {
    type Target = Collections;
    fn deref(&self) -> &Self::Target {
        &self.collections
    }
}
