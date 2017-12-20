extern crate csv;
#[macro_use]
extern crate derivative;
#[macro_use]
extern crate get_corresponding_derive;
extern crate serde;
#[macro_use]
extern crate serde_derive;

macro_rules! impl_forward_serde {
    ($obj:ident, $ty:ident < $($ty_param:ident),* > where $($where:tt)*) => {
        impl<$($ty_param),*> ::serde::Serialize for $ty<$($ty_param),*>
        where
            $($ty_param: ::serde::Serialize,)*
            $($where)*
        {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: ::serde::Serializer,
            {
                self.$obj.serialize(serializer)
            }
        }
        impl<'de $(, $ty_param)*> ::serde::Deserialize<'de> for $ty<$($ty_param),*>
        where
            $($ty_param: ::serde::Deserialize<'de>,)*
            $($where)*
        {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where D: ::serde::Deserializer<'de>
            {
                ::serde::Deserialize::deserialize(deserializer).map($ty::new)
            }
        }
    };
    ($obj:ident, $ty:ident) => {
        impl ::serde::Serialize for $ty {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where S: ::serde::Serializer
            {
                self.$obj.serialize(serializer)
            }
        }
        impl<'de> ::serde::Deserialize<'de> for $ty {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where D: ::serde::Deserializer<'de>
            {
                ::serde::Deserialize::deserialize(deserializer).map($ty::new)
            }
        }
    };
}

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
}

#[derive(GetCorresponding)]
pub struct PtObjects {
    collections: Collections,
    network_to_lines: OneToMany<Network, Line>,
    commercial_modes_to_lines: OneToMany<CommercialMode, Line>,
    lines_to_routes: OneToMany<Line, Route>,
    routes_to_vehicle_journeys: OneToMany<Route, VehicleJourney>,
    physical_modes_to_vehicle_journeys: OneToMany<PhysicalMode, VehicleJourney>,
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
            collections: c,
        }
    }
}
impl_forward_serde!(collections, PtObjects);
impl ops::Deref for PtObjects {
    type Target = Collections;
    fn deref(&self) -> &Self::Target {
        &self.collections
    }
}
