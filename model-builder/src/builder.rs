// Copyright 2017-2019 Kisio Digital and/or its affiliates.
//
// This program is free software: you can redistribute it and/or
// modify it under the terms of the GNU General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see
// <http://www.gnu.org/licenses/>.

//! Provides an easy way to create a `crate::Model`
//!
//! ```
//! # fn main() {
//!  let model = model_builder::ModelBuilder::default()
//!      .vj("toto", |vj| {
//!          vj.route("1", |_| {})
//!            .st("A", "10:00:00", "10:01:00")
//!            .st("B", "11:00:00", "11:01:00");
//!      })
//!      .vj("tata", |vj| {
//!          vj.st("A", "10:00:00", "10:01:00")
//!            .st("D", "11:00:00", "11:01:00");
//!      })
//!      .build();
//! # }
//! ```

use crate::collection::{CollectionWithId, Id, Idx, RefMut};
use crate::model::{Collections, Model};
use crate::objects::{Route, StopPoint, StopTime, Time, VehicleJourney, WithId};

/// Builder used to easily create a `Model`
#[derive(Default)]
pub struct ModelBuilder {
    collections: Collections,
}

struct ObjectModifier<T> {
    pub id: String,
    modifier: Box<Fn(&mut T)>,
}

/// Builder used to create and modify a new VehicleJourney
pub struct VehicleJourneyBuilder<'a> {
    model: &'a mut ModelBuilder,
    vj_idx: Idx<VehicleJourney>,
    route_modifier: Option<ObjectModifier<Route>>,
}

fn get_or_create<'a, T: Id<T> + WithId>(
    col: &'a mut CollectionWithId<T>,
    id: &str,
) -> RefMut<'a, T> {
    let elt = col
        .get_idx(id)
        .unwrap_or_else(|| col.push(T::with_id(id)).unwrap());
    col.index_mut(elt)
}

fn get_or_create_with<'a, T: Id<T> + WithId, F>(
    col: &'a mut CollectionWithId<T>,
    id: &str,
    f: Option<F>,
) -> RefMut<'a, T>
where
    F: FnMut(&mut T),
{
    let elt = col.get_idx(id).unwrap_or_else(|| {
        let mut o = T::with_id(id);
        if let Some(mut f) = f {
            f(&mut o);
        }
        col.push(o).unwrap()
    });
    col.index_mut(elt)
}

impl<'a> ModelBuilder {
    /// Add a new VehicleJourney to the model
    ///
    /// ```
    /// # fn main() {
    /// let model = model_builder::ModelBuilder::default()
    ///        .vj("toto", |vj_builder| {
    ///            vj_builder
    ///                .st("A", "10:00:00", "10:01:00")
    ///                .st("B", "11:00:00", "11:01:00");
    ///        })
    ///        .vj("tata", |vj_builder| {
    ///            vj_builder
    ///                .st("C", "08:00:00", "08:01:00")
    ///                .st("B", "09:00:00", "09:01:00");
    ///        })
    ///        .build();
    /// # }
    /// ```
    pub fn vj<F>(mut self, name: &str, mut vj_initer: F) -> Self
    where
        F: FnMut(VehicleJourneyBuilder),
    {
        let mut new_vj = VehicleJourney::default();
        new_vj.id = name.into();
        let vj_idx = self
            .collections
            .vehicle_journeys
            .push(new_vj)
            .expect(&format!("vj {} already exists", name));
        let vj_builder = VehicleJourneyBuilder {
            model: &mut self,
            vj_idx,
            route_modifier: None,
        };

        vj_initer(vj_builder);
        self
    }

    /// Consume the builder to create a navitia model
    pub fn build(self) -> Model {
        Model::new(self.collections).unwrap()
    }
}

pub trait IntoTime {
    fn into_time(self) -> Time;
}

impl IntoTime for Time {
    fn into_time(self) -> Time {
        self
    }
}

impl IntoTime for &Time {
    fn into_time(self) -> Time {
        *self
    }
}

impl IntoTime for &str {
    // Note: if the string is not in the right format, this conversion will fail
    fn into_time(self) -> Time {
        self.parse().unwrap()
    }
}

impl<'a> VehicleJourneyBuilder<'a> {
    fn find_or_create_sp(&mut self, sp: &str) -> Idx<StopPoint> {
        self.model
            .collections
            .stop_points
            .get_idx(sp)
            .unwrap_or_else(|| {
                let sa_id = format!("sa:{}", sp);
                let new_sp = StopPoint {
                    id: sp.to_owned(),
                    name: sp.to_owned(),
                    stop_area_id: sa_id.clone(),
                    ..Default::default()
                };

                get_or_create_with(
                    &mut self.model.collections.stop_areas,
                    &sa_id,
                    Some(|mut sa: &mut crate::objects::StopArea| sa.name = format!("sa {}", sp)),
                );

                self.model
                    .collections
                    .stop_points
                    .push(new_sp)
                    .expect(&format!("stoppoint {} already exists", sp))
            })
    }

    /// add a StopTime to the vehicle journey
    ///
    /// ```
    /// # fn main() {
    /// let model = model_builder::ModelBuilder::default()
    ///        .vj("toto", |vj_builder| {
    ///            vj_builder
    ///                .st("A", "10:00:00", "10:01:00")
    ///                .st("B", "11:00:00", "11:01:00");
    ///        })
    ///        .build();
    /// # }
    /// ```
    pub fn st(mut self, name: &str, arrival: impl IntoTime, departure: impl IntoTime) -> Self {
        let stop_point_idx = self.find_or_create_sp(name);
        {
            let vj = &mut self
                .model
                .collections
                .vehicle_journeys
                .index_mut(self.vj_idx);
            let sequence = vj.stop_times.len() as u32;
            let stop_time = StopTime {
                stop_point_idx,
                sequence,
                arrival_time: arrival.into_time(),
                departure_time: departure.into_time(),
                boarding_duration: 0u16,
                alighting_duration: 0u16,
                pickup_type: 0u8,
                drop_off_type: 0u8,
                datetime_estimated: false,
                local_zone_id: None,
            };

            vj.stop_times.push(stop_time);
        }

        self
    }

    /// Set the route of the vj, and apply a lamba if the route had to be created
    ///
    /// ```
    /// # fn main() {
    /// let model = model_builder::ModelBuilder::default()
    ///        .vj("toto", |vj_builder| {
    ///            vj_builder
    ///                .route("1", |r| {
    ///                    r.name = "bob".into();
    ///                });
    ///        })
    ///        .build();
    /// # }
    /// ```
    pub fn route<F>(mut self, name: &str, f: F) -> Self
    where
        F: Fn(&mut Route) + 'static,
    {
        self.route_modifier = Some(ObjectModifier {
            id: name.to_owned(),
            modifier: Box::new(f),
        });

        self
    }
}

impl<'a> Drop for VehicleJourneyBuilder<'a> {
    fn drop(&mut self) {
        let collections = &mut self.model.collections;
        // add the missing objects to the model (routes, lines, ...)
        let mut new_vj = collections.vehicle_journeys.index_mut(self.vj_idx);
        let dataset = get_or_create(&mut collections.datasets, &new_vj.dataset_id);
        get_or_create(&mut collections.contributors, &dataset.contributor_id);

        get_or_create(&mut collections.companies, &new_vj.company_id);
        get_or_create(&mut collections.physical_modes, &new_vj.physical_mode_id);

        if let Some(route_modifier) = &self.route_modifier {
            new_vj.route_id = route_modifier.id.clone();
        }
        let route = get_or_create_with(
            &mut collections.routes,
            &new_vj.route_id,
            self.route_modifier.as_ref().map(|m| &*m.modifier),
        );
        let line = get_or_create(&mut collections.lines, &route.line_id);
        get_or_create(&mut collections.commercial_modes, &line.commercial_mode_id);
        get_or_create(&mut collections.networks, &line.network_id);
    }
}

#[cfg(test)]
mod test {
    use super::ModelBuilder;

    #[test]
    fn simple_model_creation() {
        let model = ModelBuilder::default()
            .vj("toto", |vj_builder| {
                vj_builder
                    .st("A", "10:00:00", "10:01:00")
                    .st("B", "11:00:00", "11:01:00");
            })
            .vj("tata", |vj_builder| {
                vj_builder
                    .st("C", "10:00:00", "10:01:00")
                    .st("D", "11:00:00", "11:01:00");
            })
            .build();

        assert_eq!(
            model.get_corresponding_from_idx(model.vehicle_journeys.get_idx("toto").unwrap()),
            ["A", "B"]
                .into_iter()
                .map(|s| model.stop_points.get_idx(s).unwrap())
                .collect()
        );
        assert_eq!(
            model.get_corresponding_from_idx(model.vehicle_journeys.get_idx("tata").unwrap()),
            ["C", "D"]
                .into_iter()
                .map(|s| model.stop_points.get_idx(s).unwrap())
                .collect()
        );
    }

    #[test]
    fn same_sp_model_creation() {
        let model = ModelBuilder::default()
            .vj("toto", |vj| {
                vj.st("A", "10:00:00", "10:01:00")
                    .st("B", "11:00:00", "11:01:00");
            })
            .vj("tata", |vj| {
                vj.st("A", "10:00:00", "10:01:00")
                    .st("D", "11:00:00", "11:01:00");
            })
            .build();

        assert_eq!(
            model.get_corresponding_from_idx(model.vehicle_journeys.get_idx("toto").unwrap()),
            ["A", "B"]
                .into_iter()
                .map(|s| model.stop_points.get_idx(s).unwrap())
                .collect()
        );
        assert_eq!(
            model.get_corresponding_from_idx(model.stop_points.get_idx("A").unwrap()),
            ["toto", "tata"]
                .into_iter()
                .map(|s| model.vehicle_journeys.get_idx(s).unwrap())
                .collect()
        );

        assert_eq!(model.stop_points.len(), 3);
        assert_eq!(model.stop_areas.len(), 3);
    }

    #[test]
    fn model_creation_with_lines() {
        let model = ModelBuilder::default()
            .vj("toto", |vj_builder| {
                vj_builder
                    .route("1", |r| {
                        r.name = "bob".into();
                    })
                    .st("A", "10:00:00", "10:01:00")
                    .st("B", "11:00:00", "11:01:00");
            })
            .vj("tata", |vj_builder| {
                vj_builder
                    .route("1", |r| {
                        r.name = "bobette".into(); //useless, the route will be changed only at its creation
                    })
                    .st("C", "10:00:00", "10:01:00")
                    .st("D", "11:00:00", "11:01:00");
            })
            .vj("tutu", |vj_builder| {
                vj_builder
                    .st("C", "10:00:00", "10:01:00")
                    .st("E", "11:00:00", "11:01:00");
            })
            .build();

        assert_eq!(
            model.get_corresponding_from_idx(model.vehicle_journeys.get_idx("toto").unwrap()),
            ["A", "B"]
                .into_iter()
                .map(|s| model.stop_points.get_idx(s).unwrap())
                .collect()
        );
        assert_eq!(
            model.get_corresponding_from_idx(model.vehicle_journeys.get_idx("tata").unwrap()),
            ["C", "D"]
                .into_iter()
                .map(|s| model.stop_points.get_idx(s).unwrap())
                .collect()
        );
        // there should be only 2 routes, the route '1' and the default one for 'tutu'
        assert_eq!(model.routes.len(), 2);
        assert_eq!(
            model.get_corresponding_from_idx(model.routes.get_idx("1").unwrap()),
            ["toto", "tata"]
                .into_iter()
                .map(|s| model.vehicle_journeys.get_idx(s).unwrap())
                .collect()
        );
        assert_eq!(model.routes.get("1").unwrap().name, "bob");
        assert_eq!(
            model.get_corresponding_from_idx(model.routes.get_idx("default_route").unwrap()),
            ["tutu"]
                .into_iter()
                .map(|s| model.vehicle_journeys.get_idx(s).unwrap())
                .collect()
        );
    }

}
