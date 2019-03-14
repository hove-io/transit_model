// Copyright 2017 Kisio Digital and/or its affiliates.
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
//!          vj.route("1")
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

use crate::collection::Idx;
use crate::model::{Collections, Model};
use crate::objects::{Calendar, Route, StopPoint, StopTime, Time, VehicleJourney};

/// Builder used to easily create a `Model`
#[derive(Default)]
pub struct ModelBuilder {
    collections: Collections,
}

/// Builder used to create and modify a new VehicleJourney
pub struct VehicleJourneyBuilder<'a> {
    model: &'a mut ModelBuilder,
    vj_idx: Idx<VehicleJourney>,
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
        };

        vj_initer(vj_builder);
        self
    }

    /// Add a new Route to the model
    ///
    /// ```
    /// # fn main() {
    /// let model = model_builder::ModelBuilder::default()
    ///      .route("l1", |r| {
    ///             r.name = "ligne 1".to_owned();
    ///         })
    ///      .vj("toto", |vj| {
    ///          vj.route("l1")
    ///            .st("A", "10:00:00", "10:01:00")
    ///            .st("B", "11:00:00", "11:01:00");
    ///      })
    ///      .build();
    /// # }
    /// ```
    pub fn route<F>(mut self, id: &str, mut route_initer: F) -> Self
    where
        F: FnMut(&mut Route),
    {
        self.collections.routes.get_or_create_with(id, || {
            let mut r = Route::default();
            route_initer(&mut r);
            r
        });
        self
    }

    /// Add a new Calendar to the model
    ///
    /// ```
    /// # use navitia_model::objects::Date;
    ///
    /// # fn main() {
    /// let model = model_builder::ModelBuilder::default()
    ///      .calendar("c1", |c| {
    ///             c.dates.insert(Date::from_ymd(2019, 2, 6));
    ///         })
    ///      .vj("toto", |vj| {
    ///          vj.calendar("c1")
    ///            .st("A", "10:00:00", "10:01:00")
    ///            .st("B", "11:00:00", "11:01:00");
    ///      })
    ///      .build();
    /// # }
    /// ```
    pub fn calendar<F>(mut self, id: &str, mut calendar_initer: F) -> Self
    where
        F: FnMut(&mut Calendar),
    {
        self.collections.calendars.get_or_create_with(id, || {
            let mut c = Calendar::default();
            calendar_initer(&mut c);
            c
        });
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

                self.model.collections.stop_areas.get_or_create(&sa_id);

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

    /// Set the route of the vj
    ///
    /// ```
    /// # fn main() {
    /// let model = model_builder::ModelBuilder::default()
    ///        .route("1", |r| {
    ///            r.name = "bob".into();
    ///        })
    ///        .vj("toto", |vj_builder| {
    ///            vj_builder.route("1");
    ///        })
    ///        .build();
    /// # }
    /// ```
    pub fn route(self, id: &str) -> Self {
        {
            let vj = &mut self
                .model
                .collections
                .vehicle_journeys
                .index_mut(self.vj_idx);
            vj.route_id = id.to_owned();
        }

        self
    }

    /// Set the calendar (service_id) of the vj
    ///
    /// ```
    /// # use navitia_model::objects::Date;
    ///
    /// # fn main() {
    /// let model = model_builder::ModelBuilder::default()
    ///        .calendar("c1", |c| {
    ///             c.dates.insert(Date::from_ymd(2019, 2, 6));
    ///         })
    ///        .vj("toto", |vj_builder| {
    ///            vj_builder.calendar("c1");
    ///        })
    ///        .build();
    /// # }
    /// ```
    pub fn calendar(self, id: &str) -> Self {
        {
            let vj = &mut self
                .model
                .collections
                .vehicle_journeys
                .index_mut(self.vj_idx);
            vj.service_id = id.to_owned();
        }

        self
    }
}

impl<'a> Drop for VehicleJourneyBuilder<'a> {
    fn drop(&mut self) {
        let collections = &mut self.model.collections;
        // add the missing objects to the model (routes, lines, ...)
        let new_vj = &collections.vehicle_journeys[self.vj_idx];
        let dataset = collections.datasets.get_or_create(&new_vj.dataset_id);
        collections
            .contributors
            .get_or_create(&dataset.contributor_id);

        collections.companies.get_or_create(&new_vj.company_id);
        collections.calendars.get_or_create(&new_vj.service_id);
        collections
            .physical_modes
            .get_or_create(&new_vj.physical_mode_id);

        let route = collections.routes.get_or_create(&new_vj.route_id);
        let line = collections.lines.get_or_create(&route.line_id);
        collections
            .commercial_modes
            .get_or_create(&line.commercial_mode_id);
        collections.networks.get_or_create(&line.network_id);
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
            .route("1", |r| {
                r.name = "bob".into();
            })
            .vj("toto", |vj_builder| {
                vj_builder
                    .route("1")
                    .st("A", "10:00:00", "10:01:00")
                    .st("B", "11:00:00", "11:01:00");
            })
            .vj("tata", |vj_builder| {
                vj_builder
                    .route("2")
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
        // there should be only 3 routes, the route '1', '2' and the default one for 'tutu'
        assert_eq!(model.routes.len(), 3);
        assert_eq!(
            model.get_corresponding_from_idx(model.routes.get_idx("1").unwrap()),
            ["toto"]
                .into_iter()
                .map(|s| model.vehicle_journeys.get_idx(s).unwrap())
                .collect()
        );
        assert_eq!(
            model.get_corresponding_from_idx(model.routes.get_idx("2").unwrap()),
            ["tata"]
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
