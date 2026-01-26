// Copyright (C) 2017 Hove and/or its affiliates.
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
//!  let model = transit_model::ModelBuilder::default()
//!      .vj("toto", |vj| {
//!          vj.route("1")
//!            .st("A", "10:00:00")
//!            .st("B", "11:00:00");
//!      })
//!      .vj("tata", |vj| {
//!          vj.st("A", "10:00:00")
//!            .st("D", "11:00:00");
//!      })
//!      .build();
//! # }
//! ```

use crate::model::{Collections, Model};
use crate::objects::{
    Address, AdministrativeRegion, Calendar, CommercialMode, Company, Contributor, Dataset, Date,
    Equipment, Geometry, Line, Network, Occupancy, OccupancyStatus, PhysicalMode, Route, StopArea,
    StopPoint, StopTime, StopTimePrecision, Time, Transfer, TripProperty, ValidityPeriod,
    VehicleJourney,
};
use chrono::NaiveDateTime;
use chrono_tz;
use typed_index_collection::Idx;

const DEFAULT_CALENDAR_ID: &str = "default_service";
const DEFAULT_ROUTE_ID: &str = "default_route";
const DEFAULT_LINE_ID: &str = "default_line";
const DEFAULT_NETWORK_ID: &str = "default_network";
const DEFAULT_COMMERCIAL_MODE_ID: &str = "default_commercial_mode";
const DEFAULT_PHYSICAL_MODE_ID: &str = "default_physical_mode";

/// The default timezone used in the model
pub const DEFAULT_TIMEZONE: chrono_tz::Tz = chrono_tz::UTC;

/// Builder used to easily create a `Model`
/// Note: if not explicitly set all the vehicule journeys
/// will be attached to a default calendar starting 2020-01-01
pub struct ModelBuilder {
    collections: Collections,
    validity_period: ValidityPeriod,
}

/// Builder used to create and modify a new VehicleJourney
/// Note: if not explicitly set, the vehicule journey
/// will be attached to a default calendar starting 2020-01-01
pub struct VehicleJourneyBuilder<'a> {
    model: &'a mut ModelBuilder,
    vj_idx: Idx<VehicleJourney>,
    info: VehicleJourneyInfo,
}

#[derive(PartialEq, Eq, Default)]
/// Information about what is linked to a VehicleJourney
pub struct VehicleJourneyInfo {
    /// Route of the vehicle journey
    route_id: Option<String>,
    /// Line of the vehicle journey
    line_id: Option<String>,
    /// Network of the vehicle journey
    network_id: Option<String>,
    /// Commercial mode of the vehicle journey
    commercial_mode_id: Option<String>,
    /// Physical mode of the vehicle journey
    physical_mode_id: Option<String>,
    /// Timezone of the network of the vehicle journey
    timezone: Option<chrono_tz::Tz>,
}

impl Default for ModelBuilder {
    fn default() -> Self {
        let date = "2020-01-01";
        Self::new(date, date)
    }
}

impl ModelBuilder {
    /// Create a new ModelBuilder
    pub fn new(start_validity_period: impl AsDate, end_validity_period: impl AsDate) -> Self {
        let start_date = start_validity_period.as_date();
        let end_date = end_validity_period.as_date();
        let model_builder = Self {
            validity_period: ValidityPeriod {
                start_date,
                end_date,
            },
            collections: Collections::default(),
        };

        assert!(start_date <= end_date);
        let dates: Vec<_> = start_date
            .iter_days()
            .take_while(|date| *date <= end_date)
            .collect();

        model_builder.default_calendar(&dates)
    }

    /// Add a new VehicleJourney to the model
    ///
    /// ```
    /// # fn main() {
    /// let model = transit_model::ModelBuilder::default()
    ///        .vj("toto", |vj_builder| {
    ///            vj_builder
    ///                .st("A", "10:00:00")
    ///                .st("B", "11:00:00");
    ///        })
    ///        .vj("tata", |vj_builder| {
    ///            vj_builder
    ///                .st("C", "08:00:00")
    ///                .st("B", "09:00:00");
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

        let vj = &self.collections.vehicle_journeys[vj_idx];

        {
            let mut dataset = self.collections.datasets.get_or_create(&vj.dataset_id);
            dataset.start_date = self.validity_period.start_date;
            dataset.end_date = self.validity_period.end_date;
        }

        let vj_builder = VehicleJourneyBuilder {
            model: &mut self,
            vj_idx,
            info: VehicleJourneyInfo::default(),
        };

        vj_initer(vj_builder);
        self
    }

    /// Add a new Route to the model
    ///
    /// ```
    /// # fn main() {
    /// let model = transit_model::ModelBuilder::default()
    ///      .route("route1", |r| {
    ///             r.name = "route 1".to_owned();
    ///         })
    ///      .vj("toto", |vj| {
    ///          vj.route("route1")
    ///            .st("A", "10:00:00")
    ///            .st("B", "11:00:00");
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

    /// Add a new network to the model
    ///
    /// ```
    /// # fn main() {
    /// let model = transit_model::ModelBuilder::default()
    ///      .network("n1", |n| {
    ///             n.name = "network 1".to_owned();
    ///         })
    ///      .vj("my_vj", |vj| {
    ///          vj.network("n1")
    ///            .st("A", "10:00:00")
    ///            .st("B", "11:00:00");
    ///      })
    ///      .build();
    /// # }
    /// ```
    pub fn network<F>(mut self, id: &str, mut network_initer: F) -> Self
    where
        F: FnMut(&mut Network),
    {
        self.collections.networks.get_or_create_with(id, || {
            let mut n = Network::default();
            network_initer(&mut n);
            n
        });
        self
    }

    /// Add a new line to the model
    ///
    /// ```
    /// # fn main() {
    /// let model = transit_model::ModelBuilder::default()
    ///      .line("l1", |l| {
    ///             l.name = "line 1".to_owned();
    ///         })
    ///      .vj("my_vj", |vj| {
    ///          vj.line("l1")
    ///            .st("A", "10:00:00")
    ///            .st("B", "11:00:00");
    ///      })
    ///      .build();
    /// # }
    /// ```
    pub fn line<F>(mut self, id: &str, mut line_initer: F) -> Self
    where
        F: FnMut(&mut Line),
    {
        self.collections.lines.get_or_create_with(id, || {
            let mut l = Line::default();
            line_initer(&mut l);
            l
        });
        self
    }

    /// Add a new commercial mode to the model
    ///
    /// ```
    /// # fn main() {
    /// let model = transit_model::ModelBuilder::default()
    ///      .commercial_mode("cm1", |cm| {
    ///             cm.name = "cm 1".to_owned();
    ///         })
    ///      .vj("my_vj", |vj| {
    ///          vj.commercial_mode("cm1")
    ///            .st("A", "10:00:00")
    ///            .st("B", "11:00:00");
    ///      })
    ///      .build();
    /// # }
    /// ```
    pub fn commercial_mode<F>(mut self, id: &str, mut initer: F) -> Self
    where
        F: FnMut(&mut CommercialMode),
    {
        self.collections
            .commercial_modes
            .get_or_create_with(id, || {
                let mut c = CommercialMode::default();
                initer(&mut c);
                c
            });
        self
    }

    /// Add a new physical mode to the model
    ///
    /// ```
    /// # fn main() {
    /// let model = transit_model::ModelBuilder::default()
    ///      .physical_mode("pm1", |pm| {
    ///             pm.name = "pm 1".to_owned();
    ///         })
    ///      .vj("my_vj", |vj| {
    ///          vj.physical_mode("pm1")
    ///            .st("A", "10:00:00")
    ///            .st("B", "11:00:00");
    ///      })
    ///      .build();
    /// # }
    /// ```
    pub fn physical_mode<F>(mut self, id: &str, mut initer: F) -> Self
    where
        F: FnMut(&mut PhysicalMode),
    {
        self.collections.physical_modes.get_or_create_with(id, || {
            let mut p = PhysicalMode::default();
            initer(&mut p);
            p
        });
        self
    }

    /// Add a new Calendar or change an existing one
    ///
    /// Note: if the date are in strings not in the right format, this conversion will fail
    ///
    /// ```
    /// # use transit_model::objects::Date;
    ///
    /// # fn main() {
    /// let model = transit_model::ModelBuilder::default()
    ///      .calendar("c1", &["2020-01-01", "2020-01-02"])
    ///      .calendar("default_service", &[Date::from_ymd(2019, 2, 6)])
    ///      .vj("toto", |vj| {
    ///          vj.calendar("c1")
    ///            .st("A", "10:00:00")
    ///            .st("B", "11:00:00");
    ///      })
    ///      .build();
    /// # }
    /// ```
    pub fn calendar(mut self, id: &str, dates: &[impl AsDate]) -> Self {
        {
            let mut c = self.collections.calendars.get_or_create(id);
            for d in dates {
                c.dates.insert(d.as_date());
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
    /// let model = transit_model::ModelBuilder::default()
    ///      .default_calendar(&["2020-01-01"])
    ///      .vj("toto", |vj| {
    ///          vj
    ///            .st("A", "10:00:00")
    ///            .st("B", "11:00:00");
    ///      })
    ///      .build();
    /// # }
    /// ```
    pub fn default_calendar(self, dates: &[impl AsDate]) -> Self {
        self.calendar(DEFAULT_CALENDAR_ID, dates)
    }
    /// Add a new Calendar to the model
    ///
    /// ```
    /// # use transit_model::objects::Date;
    ///
    /// # fn main() {
    /// let model = transit_model::ModelBuilder::default()
    ///      .calendar_mut("c1", |c| {
    ///             c.dates.insert(Date::from_ymd_opt(2019, 2, 6).unwrap());
    ///         })
    ///      .vj("toto", |vj| {
    ///          vj.calendar("c1")
    ///            .st("A", "10:00:00")
    ///            .st("B", "11:00:00");
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

    /// Add a new transfer to the model
    pub fn add_transfer(
        mut self,
        from_stop_id: &str,
        to_stop_id: &str,
        transfer_duration: impl IntoTime,
    ) -> Self {
        let duration = transfer_duration.as_time().total_seconds();
        self.collections.transfers.push(Transfer {
            from_stop_id: from_stop_id.to_string(),
            to_stop_id: to_stop_id.to_string(),
            min_transfer_time: Some(duration),
            real_min_transfer_time: Some(duration),
            equipment_id: None,
        });
        self
    }

    /// Add a new contributor to the model
    pub fn contributor<F>(mut self, id: &str, mut initer: F) -> Self
    where
        F: FnMut(&mut Contributor),
    {
        self.collections.contributors.get_or_create_with(id, || {
            let mut c = Contributor {
                id: id.to_owned(),
                ..Default::default()
            };
            initer(&mut c);
            c
        });
        self
    }

    /// Add a new dataset to the model
    pub fn dataset<F>(mut self, id: &str, mut initer: F) -> Self
    where
        F: FnMut(&mut Dataset),
    {
        self.collections.datasets.get_or_create_with(id, || {
            let mut ds = Dataset {
                id: id.to_owned(),
                ..Default::default()
            };
            initer(&mut ds);
            ds
        });
        self
    }

    /// Add a new company to the model
    pub fn company<F>(mut self, id: &str, mut initer: F) -> Self
    where
        F: FnMut(&mut Company),
    {
        self.collections.companies.get_or_create_with(id, || {
            let mut c = Company {
                id: id.to_owned(),
                ..Default::default()
            };
            initer(&mut c);
            c
        });
        self
    }

    /// Add a new stop area to the model
    pub fn stop_area<F>(mut self, id: &str, mut initer: F) -> Self
    where
        F: FnMut(&mut StopArea),
    {
        self.collections.stop_areas.get_or_create_with(id, || {
            let mut sa = StopArea {
                id: id.to_owned(),
                name: id.to_owned(),
                ..Default::default()
            };
            initer(&mut sa);
            sa
        });
        self
    }

    /// Add a new equipment to the model
    pub fn equipment<F>(mut self, id: &str, mut initer: F) -> Self
    where
        F: FnMut(&mut Equipment),
    {
        self.collections.equipments.get_or_create_with(id, || {
            let mut eq = Equipment {
                id: id.to_owned(),
                ..Default::default()
            };
            initer(&mut eq);
            eq
        });
        self
    }

    /// Add a new trip property to the model
    pub fn trip_property<F>(mut self, id: &str, mut initer: F) -> Self
    where
        F: FnMut(&mut TripProperty),
    {
        self.collections.trip_properties.get_or_create_with(id, || {
            let mut trip_property = TripProperty {
                id: id.to_owned(),
                ..Default::default()
            };
            initer(&mut trip_property);
            trip_property
        });
        self
    }

    /// Add a new stop point to the model
    pub fn stop_point<F>(mut self, id: &str, mut initer: F) -> Self
    where
        F: FnMut(&mut StopPoint),
    {
        self.collections.stop_points.get_or_create_with(id, || {
            let mut sp = StopPoint {
                id: id.to_owned(),
                name: id.to_owned(),
                stop_area_id: format!("sa:{id}"),
                ..Default::default()
            };
            initer(&mut sp);
            sp
        });
        self
    }

    /// Sdd a new geometry to the model
    pub fn geometry(mut self, id: &str, wkt_str: &str) -> Self {
        use std::convert::TryInto;
        use std::str::FromStr;
        use wkt::Wkt;

        let wkt_geometry: Wkt<f64> = Wkt::from_str(wkt_str).unwrap();
        let geo_geometry: geo::Geometry<f64> = wkt_geometry.try_into().ok().unwrap();

        self.collections
            .geometries
            .get_or_create_with(id, || Geometry {
                id: id.to_owned(),
                geometry: geo_geometry.clone(),
            });
        self
    }

    /// Add a new administrative region to the model
    pub fn administrative_region<F>(mut self, id: &str, level: u32, mut initer: F) -> Self
    where
        F: FnMut(&mut AdministrativeRegion),
    {
        self.collections
            .administrative_regions
            .get_or_create_with(id, || {
                let mut ar = AdministrativeRegion {
                    id: id.to_owned(),
                    level: Some(level),
                    insee: None,
                    label: None,
                    lat: None,
                    lon: None,
                    zip_codes: None,
                    name: None,
                };
                initer(&mut ar);
                ar
            });

        self
    }

    /// Add a new address to the model
    pub fn address<F>(mut self, id: &str, mut initer: F) -> Self
    where
        F: FnMut(&mut Address),
    {
        self.collections.addresses.get_or_create_with(id, || {
            let mut ad = Address {
                id: id.to_owned(),
                ..Default::default()
            };
            initer(&mut ad);
            ad
        });
        self
    }

    /// Add a new occupancy to the model
    pub fn occupancy<F>(
        mut self,
        line_id: &str,
        from_stop_point: &str,
        to_stop_point: &str,
        occupancy_status: OccupancyStatus,
        mut occupancy_initer: F,
    ) -> Self
    where
        F: FnMut(&mut Occupancy),
    {
        // by default, apply to all dates and times
        let mut occupancy = Occupancy {
            line_id: line_id.to_string(),
            from_stop_area: format!("sa:{from_stop_point}"),
            to_stop_area: format!("sa:{to_stop_point}"),
            from_date: Date::MIN,
            to_date: Date::MAX,
            from_time: Time::new(0, 0, 0),
            to_time: Time::new(0, 0, u32::MAX),
            occupancy: occupancy_status,
            ..Default::default()
        };
        occupancy_initer(&mut occupancy);
        self.collections.occupancies.push(occupancy);
        self
    }

    /// Consume the builder to create a navitia model
    pub fn build(mut self) -> Model {
        {
            let default_calendar = self.collections.calendars.get_mut(DEFAULT_CALENDAR_ID);
            if let Some(mut cal) = default_calendar {
                if cal.dates.is_empty() {
                    cal.dates.insert(Date::from_ymd_opt(2020, 1, 1).unwrap());
                }
            }
        }

        Model::new(self.collections).unwrap()
    }
}

/// Trait used to convert a type into a `Time`
pub trait IntoTime {
    /// convert the type into a `Time`
    fn as_time(&self) -> Time;
}

impl IntoTime for Time {
    fn as_time(&self) -> Time {
        *self
    }
}

impl IntoTime for &Time {
    fn as_time(&self) -> Time {
        **self
    }
}

impl IntoTime for &str {
    // Note: if the string is not in the right format, this conversion will fail
    fn as_time(&self) -> Time {
        self.parse().expect("invalid time format")
    }
}

/// Trait used to convert a type into a `Date`
pub trait AsDate {
    /// convert the type into a `Date`
    fn as_date(&self) -> Date;
}

impl AsDate for Date {
    fn as_date(&self) -> Date {
        *self
    }
}

impl AsDate for &Date {
    fn as_date(&self) -> Date {
        **self
    }
}

impl AsDate for &str {
    // Note: if the string is not in the right format, this conversion will fail
    fn as_date(&self) -> Date {
        self.parse().expect("invalid date format")
    }
}

/// Trait used to convert a type into a `NaiveDateTime`
pub trait AsDateTime {
    /// convert the type into a `NaiveDateTime`
    fn as_datetime(&self) -> NaiveDateTime;
}

impl AsDateTime for &str {
    fn as_datetime(&self) -> NaiveDateTime {
        self.parse().expect("invalid datetime format")
    }
}

impl AsDateTime for NaiveDateTime {
    fn as_datetime(&self) -> NaiveDateTime {
        *self
    }
}

impl AsDateTime for &NaiveDateTime {
    fn as_datetime(&self) -> NaiveDateTime {
        **self
    }
}

impl VehicleJourneyBuilder<'_> {
    fn find_or_create_sp(&mut self, sp: &str) -> Idx<StopPoint> {
        self.model
            .collections
            .stop_points
            .get_idx(sp)
            .unwrap_or_else(|| {
                let sa_id = format!("sa:{sp}");
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

    /// add a basic StopTime to the vehicle journey
    ///
    /// Note: if the arrival/departure are given in string
    /// not in the right format, this conversion will fail
    ///
    /// ```
    ///
    /// # use transit_model::ModelBuilder;
    ///
    /// # fn main() {
    /// let model = ModelBuilder::default()
    ///        .vj("toto", |vj_builder| {
    ///            vj_builder
    ///                .st("A", "10:00:00")
    ///                .st("B", "11:00:00");
    ///        })
    ///        .build();
    /// # }
    /// ```
    pub fn st(self, name: &str, arrival: impl IntoTime) -> Self {
        self.st_mut(
            name,
            arrival.as_time(),
            arrival.as_time(),
            0u8,
            0u8,
            None,
            |_st| {},
        )
    }

    /// Add a StopTime of type ODT to the vehicle journey
    pub fn st_odt(self, name: &str, arrival: impl IntoTime) -> Self {
        self.st_mut(
            name,
            arrival.as_time(),
            arrival.as_time(),
            2u8,
            2u8,
            None,
            |_st| {},
        )
    }

    /// Add a StopTime where the vehicle does not stop
    pub fn st_skip(self, name: &str, arrival: impl IntoTime) -> Self {
        self.st_mut(
            name,
            arrival.as_time(),
            arrival.as_time(),
            3u8,
            3u8,
            None,
            |_st| {},
        )
    }

    /// Add a StopTime to the vehicle journey
    pub fn st_detailed(
        self,
        name: &str,
        arrival: impl IntoTime,
        depart: impl IntoTime,
        pickup_type: u8,
        drop_off_type: u8,
        local_zone_id: Option<u16>,
    ) -> Self {
        self.st_mut(
            name,
            arrival.as_time(),
            depart.as_time(),
            pickup_type,
            drop_off_type,
            local_zone_id,
            |_st| {},
        )
    }

    /// add a StopTime to the vehicle journey and modify it
    #[allow(clippy::too_many_arguments)]
    pub fn st_mut<F>(
        mut self,
        name: &str,
        arrival: impl IntoTime,
        departure: impl IntoTime,
        pickup_type: u8,
        drop_off_type: u8,
        local_zone_id: Option<u16>,
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
                arrival_time: Some(arrival.as_time()),
                departure_time: Some(departure.as_time()),
                start_pickup_drop_off_window: None,
                end_pickup_drop_off_window: None,
                boarding_duration: 0u16,
                alighting_duration: 0u16,
                pickup_type,
                drop_off_type,
                local_zone_id,
                precision: None,
            };
            st_muter(&mut stop_time);

            vj.stop_times.push(stop_time);
        }

        self
    }

    /// add a window stoptime to the vehicle journey
    /// with (by default) pickup_type/drop_off_type and stop_time_precision at value 2
    ///
    /// ```
    /// # use transit_model::ModelBuilder;
    ///
    /// # fn main() {
    /// let model = ModelBuilder::default()
    ///        .vj("vj1", |vj_builder| {
    ///            vj_builder
    ///                .stw("A", "08:00:00", "19:00:00")
    ///                .stw("B", "08:00:00", "19:00:00");
    ///        })
    ///        .build();
    /// # }
    /// ```
    pub fn stw(
        self,
        stop_point_id: &str,
        start_pickup_drop_off_window: impl IntoTime,
        end_pickup_drop_off_window: impl IntoTime,
    ) -> Self {
        self.stw_mut(
            stop_point_id,
            start_pickup_drop_off_window.as_time(),
            end_pickup_drop_off_window.as_time(),
            2u8,
            2u8,
            Some(StopTimePrecision::Estimated),
            None,
            |_st| {},
        )
    }

    ///  add a more detailed window stoptime to the vehicle journey
    #[allow(clippy::too_many_arguments)]
    pub fn stw_detailed(
        self,
        stop_point_id: &str,
        start_pickup_drop_off_window: impl IntoTime,
        end_pickup_drop_off_window: impl IntoTime,
        pickup_type: u8,
        drop_off_type: u8,
        precision: Option<StopTimePrecision>,
        local_zone_id: Option<u16>,
    ) -> Self {
        self.stw_mut(
            stop_point_id,
            start_pickup_drop_off_window.as_time(),
            end_pickup_drop_off_window.as_time(),
            pickup_type,
            drop_off_type,
            precision,
            local_zone_id,
            |_st| {},
        )
    }

    /// add a window stopTime to the vehicle journey and modify it
    #[allow(clippy::too_many_arguments)]
    fn stw_mut<F>(
        mut self,
        stop_point_id: &str,
        start_pickup_drop_off_window: impl IntoTime,
        end_pickup_drop_off_window: impl IntoTime,
        pickup_type: u8,
        drop_off_type: u8,
        precision: Option<StopTimePrecision>,
        local_zone_id: Option<u16>,
        st_muter: F,
    ) -> Self
    where
        F: FnOnce(&mut StopTime),
    {
        {
            let stop_point_idx = self.find_or_create_sp(stop_point_id);
            let vj = &mut self
                .model
                .collections
                .vehicle_journeys
                .index_mut(self.vj_idx);
            let sequence = vj.stop_times.len() as u32;
            let mut stop_time = StopTime {
                stop_point_idx,
                sequence,
                arrival_time: None,
                departure_time: None,
                start_pickup_drop_off_window: Some(start_pickup_drop_off_window.as_time()),
                end_pickup_drop_off_window: Some(end_pickup_drop_off_window.as_time()),
                boarding_duration: 0u16,
                alighting_duration: 0u16,
                pickup_type,
                drop_off_type,
                local_zone_id,
                precision,
            };
            st_muter(&mut stop_time);

            vj.stop_times.push(stop_time);
        }

        self
    }

    /// Set the route id of the vj
    ///
    /// ```
    /// # fn main() {
    /// let model = transit_model::ModelBuilder::default()
    ///        .vj("toto", |vj_builder| {
    ///            vj_builder.route("1");
    ///        })
    ///        .build();
    /// # }
    /// ```
    pub fn route(mut self, id: &str) -> Self {
        self.info.route_id = Some(id.to_string());

        self
    }

    /// Set the line id of the vj
    ///
    /// ```
    /// # fn main() {
    /// let model = transit_model::ModelBuilder::default()
    ///      .vj("my_vj", |vj| {
    ///          vj.line("l1");
    ///      })
    ///      .build();
    /// # }
    /// ```
    pub fn line(mut self, id: &str) -> Self {
        self.info.line_id = Some(id.to_string());

        self
    }

    /// Set the line id of the vj
    ///
    /// ```
    /// # fn main() {
    /// let model = transit_model::ModelBuilder::default()
    ///      .vj("my_vj", |vj| {
    ///          vj.network("n1")
    ///            .st("A", "10:00:00")
    ///            .st("B", "11:00:00");
    ///      })
    ///      .build();
    /// # }
    /// ```
    pub fn network(mut self, id: &str) -> Self {
        {
            self.info.network_id = Some(id.to_string());
        }

        self
    }

    /// Set the timezone id of the vj
    ///
    /// ```
    /// # fn main() {
    /// let timezone = chrono_tz::Europe::Paris;
    /// let model = transit_model::ModelBuilder::default()
    ///      .vj("my_vj", |vj| {
    ///          vj.network("n1").timezone(timezone);
    ///      })
    ///      .build();
    /// # }
    /// ```
    pub fn timezone(mut self, timezone: chrono_tz::Tz) -> Self {
        {
            self.info.timezone = Some(timezone);
        }

        self
    }

    /// Set the commercial mode id of the vj
    ///
    /// ```
    /// # fn main() {
    /// let model = transit_model::ModelBuilder::default()
    ///      .vj("my_vj", |vj| {
    ///          vj.commercial_mode("cm1");
    ///      })
    ///      .build();
    /// # }
    /// ```
    pub fn commercial_mode(mut self, id: &str) -> Self {
        {
            self.info.commercial_mode_id = Some(id.to_string());
        }

        self
    }

    /// Set the physical mode id of the vj
    ///
    /// ```
    /// # fn main() {
    /// let model = transit_model::ModelBuilder::default()
    ///      .vj("my_vj", |vj| {
    ///          vj.physical_mode("pm1");
    ///      })
    ///      .build();
    /// # }
    /// ```
    pub fn physical_mode(mut self, id: &str) -> Self {
        {
            self.info.physical_mode_id = Some(id.to_string());
        }

        self
    }

    /// Set the dataset (dataset_id) of the vj
    ///
    /// ```
    /// # use transit_model::objects::Date;
    ///
    /// # fn main() {
    /// let model = transit_model::ModelBuilder::default()
    ///        .dataset("d1", |dataset| {
    ///             dataset.id = "d1".to_owned();
    ///          })
    ///        .vj("toto", |vj_builder| {
    ///            vj_builder.dataset("d1");
    ///        })
    ///        .build();
    /// # }
    /// ```
    pub fn dataset(self, id: &str) -> Self {
        {
            let vj = &mut self
                .model
                .collections
                .vehicle_journeys
                .index_mut(self.vj_idx);
            id.clone_into(&mut vj.dataset_id);
        }
        self
    }

    /// Set the company (company_id) of the vj
    ///
    /// ```
    /// # use transit_model::objects::Date;
    ///
    /// # fn main() {
    /// let model = transit_model::ModelBuilder::default()
    ///        .company("c1", |company| {
    ///             company.id = "c1".to_owned();
    ///          })
    ///        .vj("toto", |vj_builder| {
    ///            vj_builder.company("d1");
    ///        })
    ///        .build();
    /// # }
    /// ```
    pub fn company(self, id: &str) -> Self {
        {
            let vj = &mut self
                .model
                .collections
                .vehicle_journeys
                .index_mut(self.vj_idx);
            id.clone_into(&mut vj.company_id);
        }
        self
    }

    /// Set the calendar (service_id) of the vj
    ///
    /// ```
    /// # use transit_model::objects::Date;
    ///
    /// # fn main() {
    /// let model = transit_model::ModelBuilder::default()
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
            id.clone_into(&mut vj.service_id);
        }

        self
    }

    /// Set the trip property id of the vj
    ///
    /// ```
    /// # fn main() {
    /// let model = transit_model::ModelBuilder::default()
    ///      .route("1", |route| {
    ///             route.name = "route 1".to_owned();
    ///         })
    ///      .vj("my_vj", |vj| {
    ///          vj.trip_property("tp1");
    ///      })
    ///      .build();
    /// # }
    /// ```
    pub fn trip_property(self, id: &str) -> Self {
        {
            let vj = &mut self
                .model
                .collections
                .vehicle_journeys
                .index_mut(self.vj_idx);
            vj.trip_property_id = Some(id.to_string());
        }

        self
    }

    /// Set the block_id of the vj
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

impl Drop for VehicleJourneyBuilder<'_> {
    fn drop(&mut self) {
        use std::ops::DerefMut;
        let collections = &mut self.model.collections;
        // add the missing objects to the model (routes, lines, ...)
        let mut new_vj = collections.vehicle_journeys.index_mut(self.vj_idx);
        let dataset = collections.datasets.get_or_create(&new_vj.dataset_id);
        collections
            .contributors
            .get_or_create(&dataset.contributor_id);

        collections.companies.get_or_create(&new_vj.company_id);
        collections.calendars.get_or_create(&new_vj.service_id);

        let route_id = self
            .info
            .route_id
            .clone()
            .unwrap_or(DEFAULT_ROUTE_ID.to_string());

        new_vj.deref_mut().route_id = route_id;

        let mut route = collections.routes.get_or_create(&new_vj.route_id);

        let line_id = self
            .info
            .line_id
            .clone()
            .unwrap_or(DEFAULT_LINE_ID.to_string());

        route.deref_mut().line_id.clone_from(&line_id);
        let mut line = collections.lines.get_or_create(&line_id);
        collections
            .commercial_modes
            .get_or_create(&line.commercial_mode_id);

        let network_id = self
            .info
            .network_id
            .clone()
            .unwrap_or(DEFAULT_NETWORK_ID.to_string());
        line.deref_mut().network_id.clone_from(&network_id);

        let commercial_mode_id = self
            .info
            .commercial_mode_id
            .clone()
            .unwrap_or(DEFAULT_COMMERCIAL_MODE_ID.to_string());
        line.deref_mut().commercial_mode_id = commercial_mode_id;

        let physical_mode_id = self
            .info
            .physical_mode_id
            .clone()
            .unwrap_or(DEFAULT_PHYSICAL_MODE_ID.to_string());
        new_vj.deref_mut().physical_mode_id = physical_mode_id;

        collections
            .physical_modes
            .get_or_create(&new_vj.physical_mode_id);

        let timezone = self.info.timezone.or(Some(DEFAULT_TIMEZONE));
        collections.networks.get_or_create_with(&network_id, || {
            use typed_index_collection::WithId;
            let mut network = Network::with_id(&network_id);
            network.timezone = timezone;
            network
        });
    }
}

#[cfg(test)]
mod test {
    use super::ModelBuilder;
    use super::{StopTimePrecision, Time};

    #[test]
    fn simple_model_creation() {
        let model = ModelBuilder::default()
            .vj("toto", |vj_builder| {
                vj_builder.st("A", "10:00:00").st("B", "11:00:00");
            })
            .vj("tata", |vj_builder| {
                vj_builder.st("C", "10:00:00").st("D", "11:00:00");
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
        let dates = [crate::objects::Date::from_ymd_opt(2020, 1, 1).unwrap()]
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(default_calendar.dates, dates);
    }

    #[test]
    fn same_sp_model_creation() {
        let model = ModelBuilder::default()
            .vj("toto", |vj| {
                vj.st("A", "10:00:00").st("B", "11:00:00");
            })
            .vj("tata", |vj| {
                vj.st("A", "10:00:00").st("D", "11:00:00");
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
                    .st("A", "10:00:00")
                    .st("B", "11:00:00");
            })
            .vj("tata", |vj_builder| {
                vj_builder
                    .route("2")
                    .st("C", "10:00:00")
                    .st("D", "11:00:00");
            })
            .vj("tutu", |vj_builder| {
                vj_builder.st("C", "10:00:00").st("E", "11:00:00");
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

    #[test]
    fn test_vj_with_window_stoptimes() {
        let model = ModelBuilder::default()
            .vj("vj1", |vj| {
                vj.stw("sp1", "10:00:00", "20:00:00")
                    .stw("sp2", "10:10:00", "20:10:00");
            })
            .build();

        let vj = model.vehicle_journeys.get("vj1").unwrap();
        let mut st_iter = vj.stop_times.iter();

        let st1 = st_iter.next().unwrap();
        assert_eq!(st1.start_pickup_drop_off_window, Some(Time::new(10, 0, 0)));
        assert_eq!(st1.end_pickup_drop_off_window, Some(Time::new(20, 0, 0)));
        assert!(st1.departure_time.is_none());
        assert!(st1.arrival_time.is_none());
        assert_eq!(st1.pickup_type, 2u8);
        assert_eq!(st1.drop_off_type, 1u8); // see enhancers::enhance_pickup_dropoff
        assert_eq!(st1.precision, Some(StopTimePrecision::Estimated));

        let st2 = st_iter.next().unwrap();
        assert_eq!(st2.start_pickup_drop_off_window, Some(Time::new(10, 10, 0)));
        assert_eq!(st2.end_pickup_drop_off_window, Some(Time::new(20, 10, 0)));
        assert!(st2.departure_time.is_none());
        assert!(st2.arrival_time.is_none());
        assert_eq!(st2.pickup_type, 1u8); // see enhancers::enhance_pickup_dropoff
        assert_eq!(st2.drop_off_type, 2u8);
        assert_eq!(st2.precision, Some(StopTimePrecision::Estimated));

        assert_eq!(
            model.get_corresponding_from_idx(model.vehicle_journeys.get_idx("vj1").unwrap()),
            ["sp1", "sp2"]
                .iter()
                .map(|s| model.stop_points.get_idx(s).unwrap())
                .collect()
        );
    }

    #[test]
    fn test_vj_with_window_detailed_stoptimes() {
        let model = ModelBuilder::default()
            .vj("vj1", |vj| {
                vj.stw_detailed(
                    "sp1",
                    "10:00:00",
                    "20:00:00",
                    1u8,
                    1u8,
                    Some(StopTimePrecision::Exact),
                    Some(7u16),
                )
                .stw_detailed(
                    "sp2",
                    "10:10:00",
                    "20:10:00",
                    1u8,
                    1u8,
                    Some(StopTimePrecision::Exact),
                    Some(7u16),
                );
            })
            .build();

        let vj = model.vehicle_journeys.get("vj1").unwrap();
        let mut st_iter = vj.stop_times.iter();

        let st1 = st_iter.next().unwrap();
        assert_eq!(st1.start_pickup_drop_off_window, Some(Time::new(10, 0, 0)));
        assert_eq!(st1.end_pickup_drop_off_window, Some(Time::new(20, 0, 0)));
        assert!(st1.departure_time.is_none());
        assert!(st1.arrival_time.is_none());
        assert_eq!(st1.pickup_type, 1u8);
        assert_eq!(st1.drop_off_type, 1u8);
        assert_eq!(st1.precision, Some(StopTimePrecision::Exact));

        let st2 = st_iter.next().unwrap();
        assert_eq!(st2.start_pickup_drop_off_window, Some(Time::new(10, 10, 0)));
        assert_eq!(st2.end_pickup_drop_off_window, Some(Time::new(20, 10, 0)));
        assert!(st2.departure_time.is_none());
        assert!(st2.arrival_time.is_none());
        assert_eq!(st2.pickup_type, 1u8);
        assert_eq!(st2.drop_off_type, 1u8);
        assert_eq!(st2.precision, Some(StopTimePrecision::Exact));
    }

    #[test]
    fn dataset_model_creation() {
        let model = ModelBuilder::default()
            .dataset("d1", |dataset| {
                dataset.id = "d1".to_owned();
            })
            .vj("toto", |vj| {
                vj.dataset("d1").st("A", "10:00:00").st("B", "11:00:00");
            })
            .build();

        assert_eq!(
            model.get_corresponding_from_idx(model.vehicle_journeys.get_idx("toto").unwrap()),
            ["d1"]
                .iter()
                .map(|s| model.datasets.get_idx(s).unwrap())
                .collect()
        );
        assert_eq!(
            model.get_corresponding_from_idx(model.datasets.get_idx("d1").unwrap()),
            ["toto"]
                .iter()
                .map(|s| model.vehicle_journeys.get_idx(s).unwrap())
                .collect()
        );

        assert_eq!(model.datasets.len(), 1);
    }

    #[test]
    fn company_model_creation() {
        let model = ModelBuilder::default()
            .company("c1", |dataset| {
                dataset.id = "c1".to_owned();
            })
            .vj("toto", |vj| {
                vj.company("c1").st("A", "10:00:00").st("B", "11:00:00");
            })
            .build();

        assert_eq!(
            model.get_corresponding_from_idx(model.vehicle_journeys.get_idx("toto").unwrap()),
            ["c1"]
                .iter()
                .map(|s| model.companies.get_idx(s).unwrap())
                .collect()
        );
        assert_eq!(
            model.get_corresponding_from_idx(model.companies.get_idx("c1").unwrap()),
            ["toto"]
                .iter()
                .map(|s| model.vehicle_journeys.get_idx(s).unwrap())
                .collect()
        );

        assert_eq!(model.companies.len(), 1);
    }

    #[test]
    fn contributor_model_creation() {
        let model = ModelBuilder::default()
            .contributor("contrib1", |contrib| {
                contrib.id = "c1".to_owned();
            })
            .dataset("d1", |dataset_initer| {
                dataset_initer.id = "d1".to_owned();
                dataset_initer.contributor_id = "contrib1".to_owned();
            })
            .vj("toto", |vj| {
                vj.dataset("d1").st("A", "10:00:00").st("B", "11:00:00");
            })
            .build();

        assert_eq!(
            model.get_corresponding_from_idx(model.vehicle_journeys.get_idx("toto").unwrap()),
            ["d1"]
                .iter()
                .map(|s| model.datasets.get_idx(s).unwrap())
                .collect()
        );
        assert_eq!(
            model.get_corresponding_from_idx(model.datasets.get_idx("d1").unwrap()),
            ["toto"]
                .iter()
                .map(|s| model.vehicle_journeys.get_idx(s).unwrap())
                .collect()
        );

        assert_eq!(
            model.get_corresponding_from_idx(model.datasets.get_idx("d1").unwrap()),
            ["contrib1"]
                .iter()
                .map(|s| model.contributors.get_idx(s).unwrap())
                .collect()
        );

        assert_eq!(
            model.get_corresponding_from_idx(model.contributors.get_idx("contrib1").unwrap()),
            ["d1"]
                .iter()
                .map(|s| model.datasets.get_idx(s).unwrap())
                .collect()
        );
        assert_eq!(model.datasets.len(), 1);
        assert_eq!(model.contributors.len(), 1);
    }

    #[test]
    fn address_model_creation() {
        let model = ModelBuilder::default()
            .administrative_region("admin_8", 8, |admin_builder| {
                admin_builder.id = "admin_8".to_owned();
            })
            .administrative_region("admin_9", 9, |admin_builder| {
                admin_builder.id = "admin_9".to_owned();
            })
            .administrative_region("admin_10", 10, |admin_builder| {
                admin_builder.id = "admin_10".to_owned();
            })
            .address("addr1", |addr_builder| {
                addr_builder.id = "addr1".to_owned();
                addr_builder.admin_level_8_id = Some("admin_8".to_owned());
                addr_builder.admin_level_9_id = Some("admin_9".to_owned());
                addr_builder.admin_level_10_id = Some("admin_10".to_owned());
            })
            .stop_area("A", |sp_builder| {
                sp_builder.address_id = Some("addr1".to_owned());
            })
            .stop_point("A", |sp_builder| {
                sp_builder.stop_area_id = "A".to_owned();
                sp_builder.address_id = Some("addr1".to_owned());
            })
            .vj("toto", |vj| {
                vj.st("A", "10:00:00").st("B", "11:00:00");
            })
            .build();

        assert_eq!(model.addresses.len(), 1);
        assert_eq!(model.administrative_regions.len(), 3);

        let stop_area = model.stop_areas.get("A").unwrap();
        assert_eq!(stop_area.address_id.as_deref(), Some("addr1"));

        let stop_point = model.stop_points.get("A").unwrap();
        assert_eq!(stop_point.address_id.as_deref(), Some("addr1"));

        let address = model.addresses.get("addr1").unwrap();
        assert_eq!(address.admin_level_8_id.as_deref(), Some("admin_8"));
        assert_eq!(address.admin_level_9_id.as_deref(), Some("admin_9"));
        assert_eq!(address.admin_level_10_id.as_deref(), Some("admin_10"));

        let admin_8 = model.administrative_regions.get("admin_8").unwrap();
        assert_eq!(admin_8.level, Some(8));
        let admin_9 = model.administrative_regions.get("admin_9").unwrap();
        assert_eq!(admin_9.level, Some(9));
        let admin_10 = model.administrative_regions.get("admin_10").unwrap();
        assert_eq!(admin_10.level, Some(10));
    }
}
