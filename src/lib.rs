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
use relations::{IdxSet, OneToMany};

#[derive(Derivative, Serialize, Deserialize, Debug)]
#[derivative(Default)]
pub struct Collections {
    pub networks: Collection<Network>,
    pub commercial_modes: Collection<CommercialMode>,
    pub lines: Collection<Line>,
    pub routes: Collection<Route>,
    pub vehicle_journeys: Collection<VehicleJourney>,
    pub physical_modes: Collection<PhysicalMode>,
    pub stop_areas: Collection<StopArea>,
    pub stop_points: Collection<StopPoint>,
}

#[derive(GetCorresponding)]
pub struct PtObjects {
    collections: Collections,
    network_to_lines: OneToMany<Network, Line>,
    commercial_modes_to_lines: OneToMany<CommercialMode, Line>,
    lines_to_routes: OneToMany<Line, Route>,
    routes_to_vehicle_journeys: OneToMany<Route, VehicleJourney>,
    physical_modes_to_vehicle_journeys: OneToMany<PhysicalMode, VehicleJourney>,
    stop_areas_to_stop_points: OneToMany<StopArea, StopPoint>,
}
impl PtObjects {
    pub fn new(c: Collections) -> Self {
        PtObjects {
            network_to_lines: OneToMany::new(&c.networks, &c.lines),
            commercial_modes_to_lines: OneToMany::new(&c.commercial_modes, &c.lines),
            lines_to_routes: OneToMany::new(&c.lines, &c.routes),
            routes_to_vehicle_journeys: OneToMany::new(&c.routes, &c.vehicle_journeys),
            physical_modes_to_vehicle_journeys: OneToMany::new(
                &c.physical_modes,
                &c.vehicle_journeys,
            ),
            stop_areas_to_stop_points: OneToMany::new(&c.stop_areas, &c.stop_points),
            collections: c,
        }
    }
}
impl ::serde::Serialize for PtObjects {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: ::serde::Serializer
    {
        self.collections.serialize(serializer)
    }
}
impl<'de> ::serde::Deserialize<'de> for PtObjects {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: ::serde::Deserializer<'de>
    {
        ::serde::Deserialize::deserialize(deserializer).map(PtObjects::new)
    }
}
impl ops::Deref for PtObjects {
    type Target = Collections;
    fn deref(&self) -> &Self::Target {
        &self.collections
    }
}
