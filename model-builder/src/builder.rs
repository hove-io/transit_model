// Copyright (C) 2017 Kisio Digital and/or its affiliates.
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by the
// Free Software Foundation, version 3.

// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.

// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>

//! Provides an easy way to create a `crate::Model`
//!
//! ```
//! # fn main() {
//!  let model = transit_model_builder::ModelBuilder::default()
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

use crate::model::{Collections, Model};
use crate::objects::{Calendar, Date, Route, StopPoint, StopTime, Time, VehicleJourney};
use typed_index_collection::Idx;

const DEFAULT_CALENDAR_ID: &str = "default_service";

/// Builder used to easily create a `Model`
/// Note: if not explicitly set all the vehicule journeys
/// will be attached to a default calendar starting 2020-01-01
#[derive(Default)]
pub struct ModelBuilder {
    collections: Collections,
}

/// Builder used to create and modify a new VehicleJourney
/// Note: if not explicitly set, the vehicule journey
/// will be attached to a default calendar starting 2020-01-01
pub struct VehicleJourneyBuilder<'a> {
    model: &'a mut ModelBuilder,
    vj_idx: Idx<VehicleJourney>,
}

impl<'a> ModelBuilder {
    /// Add a new VehicleJourney to the model
    ///
    /// ```
    /// # fn main() {
    /// let model = transit_model_builder::ModelBuilder::default()
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
        let new_vj = VehicleJourney {
            id: name.into(),
            ..Default::default()
        };
        let vj_idx = self
            .collections
            .vehicle_journeys
            .push(new_vj)
            .unwrap_or_else(|_| panic!("vj {} already exists", name));
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
    /// let model = transit_model_builder::ModelBuilder::default()
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

    /// Add a new Calendar or change an existing one
    ///
    /// ```
    /// # use transit_model::objects::Date;
    ///
    /// # fn main() {
    /// let model = transit_model_builder::ModelBuilder::default()
    ///      .calendar("c1", &["2020-01-01", "2020-01-02"])
    ///      .calendar("default_service", &[Date::from_ymd(2019, 2, 6)])
    ///      .vj("toto", |vj| {
    ///          vj.calendar("c1")
    ///            .st("A", "10:00:00", "10:01:00")
    ///            .st("B", "11:00:00", "11:01:00");
    ///      })
    ///      .build();
    /// # }
    /// ```
    pub fn calendar(mut self, id: &str, dates: &[impl IntoDate]) -> Self {
        {
            let mut c = self.collections.calendars.get_or_create(id);
            for d in dates {
                c.dates.insert(d.into_date());
            }
        }
        self
    }

    /// Change the default Calendar
    /// If not explicitly set, all vehicule journeys will be linked
    /// to this calendar
    ///
    /// ```
    /// # use transit_model::objects::Date;
    ///
    /// # fn main() {
    /// let model = transit_model_builder::ModelBuilder::default()
    ///      .default_calendar(&["2020-01-01"])
    ///      .vj("toto", |vj| {
    ///          vj
    ///            .st("A", "10:00:00", "10:01:00")
    ///            .st("B", "11:00:00", "11:01:00");
    ///      })
    ///      .build();
    /// # }
    /// ```
    pub fn default_calendar(self, dates: &[impl IntoDate]) -> Self {
        self.calendar(DEFAULT_CALENDAR_ID, dates)
    }
    /// Add a new Calendar to the model
    ///
    /// ```
    /// # use transit_model::objects::Date;
    ///
    /// # fn main() {
    /// let model = transit_model_builder::ModelBuilder::default()
    ///      .calendar_mut("c1", |c| {
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
    pub fn calendar_mut<F>(mut self, id: &str, mut calendar_initer: F) -> Self
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
    pub fn build(mut self) -> Model {
        {
            let default_calendar = self.collections.calendars.get_mut(DEFAULT_CALENDAR_ID);
            if let Some(mut cal) = default_calendar {
                if cal.dates.is_empty() {
                    cal.dates.insert(Date::from_ymd(2020, 1, 1));
                }
            }
        }

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

pub trait IntoDate {
    fn into_date(&self) -> Date;
}

impl IntoDate for Date {
    fn into_date(&self) -> Date {
        *self
    }
}

impl IntoDate for &Date {
    fn into_date(&self) -> Date {
        **self
    }
}

impl IntoDate for &str {
    // Note: if the string is not in the right format, this conversion will fail
    fn into_date(&self) -> Date {
        self.parse().expect("invalid date format")
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
                    .unwrap_or_else(|_| panic!("stoppoint {} already exists", sp))
            })
    }

    /// add a StopTime to the vehicle journey
    ///
    /// ```
    /// # fn main() {
    /// let model = transit_model_builder::ModelBuilder::default()
    ///        .vj("toto", |vj_builder| {
    ///            vj_builder
    ///                .st("A", "10:00:00", "10:01:00")
    ///                .st("B", "11:00:00", "11:01:00");
    ///        })
    ///        .build();
    /// # }
    /// ```
    pub fn st(self, name: &str, arrival: impl IntoTime, departure: impl IntoTime) -> Self {
        self.st_mut(name, arrival, departure, |_st| {})
    }

    pub fn st_mut<F>(
        mut self,
        name: &str,
        arrival: impl IntoTime,
        departure: impl IntoTime,
        st_muter: F,
    ) -> Self
    where
        F: FnOnce(&mut StopTime),
    {
        {
            let stop_point_idx = self.find_or_create_sp(name);
            let vj = &mut self
                .model
                .collections
                .vehicle_journeys
                .index_mut(self.vj_idx);
            let sequence = vj.stop_times.len() as u32;
            let mut stop_time = StopTime {
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
                precision: None,
            };
            st_muter(&mut stop_time);

            vj.stop_times.push(stop_time);
        }

        self
    }

    /// Set the route of the vj
    ///
    /// ```
    /// # fn main() {
    /// let model = transit_model_builder::ModelBuilder::default()
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
    /// # use transit_model::objects::Date;
    ///
    /// # fn main() {
    /// let model = transit_model_builder::ModelBuilder::default()
    ///        .calendar("c1", &["2021-01-07"])
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

    pub fn block_id(self, block_id: &str) -> Self {
        {
            let vj = &mut self
                .model
                .collections
                .vehicle_journeys
                .index_mut(self.vj_idx);
            vj.block_id = Some(block_id.to_owned());
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
                .iter()
                .map(|s| model.stop_points.get_idx(s).unwrap())
                .collect()
        );
        assert_eq!(
            model.get_corresponding_from_idx(model.vehicle_journeys.get_idx("tata").unwrap()),
            ["C", "D"]
                .iter()
                .map(|s| model.stop_points.get_idx(s).unwrap())
                .collect()
        );
        let default_calendar = model.calendars.get("default_service").unwrap();
        let dates = [transit_model::objects::Date::from_ymd(2020, 1, 1)]
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(default_calendar.dates, dates);
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
                .iter()
                .map(|s| model.stop_points.get_idx(s).unwrap())
                .collect()
        );
        assert_eq!(
            model.get_corresponding_from_idx(model.stop_points.get_idx("A").unwrap()),
            ["toto", "tata"]
                .iter()
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
                .iter()
                .map(|s| model.stop_points.get_idx(s).unwrap())
                .collect()
        );
        assert_eq!(
            model.get_corresponding_from_idx(model.vehicle_journeys.get_idx("tata").unwrap()),
            ["C", "D"]
                .iter()
                .map(|s| model.stop_points.get_idx(s).unwrap())
                .collect()
        );
        // there should be only 3 routes, the route '1', '2' and the default one for 'tutu'
        assert_eq!(model.routes.len(), 3);
        assert_eq!(
            model.get_corresponding_from_idx(model.routes.get_idx("1").unwrap()),
            ["toto"]
                .iter()
                .map(|s| model.vehicle_journeys.get_idx(s).unwrap())
                .collect()
        );
        assert_eq!(
            model.get_corresponding_from_idx(model.routes.get_idx("2").unwrap()),
            ["tata"]
                .iter()
                .map(|s| model.vehicle_journeys.get_idx(s).unwrap())
                .collect()
        );
        assert_eq!(model.routes.get("1").unwrap().name, "bob");
        assert_eq!(
            model.get_corresponding_from_idx(model.routes.get_idx("default_route").unwrap()),
            ["tutu"]
                .iter()
                .map(|s| model.vehicle_journeys.get_idx(s).unwrap())
                .collect()
        );
    }
}
