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

//! Definition of the navitia transit model.

use crate::{enhancers, objects::*, Error, Result};
use anyhow::{anyhow, bail};
use chrono::NaiveDate;
use derivative::Derivative;
use geo::algorithm::centroid::Centroid;
use geo::MultiPoint;
use relational_types::{GetCorresponding, IdxSet, ManyToMany, OneToMany, Relation};
use serde::{Deserialize, Serialize};
use skip_error::skip_error_and_warn;
use std::{
    cmp::{self, Ordering, Reverse},
    collections::{hash_map::DefaultHasher, BTreeMap, HashMap, HashSet},
    convert::TryFrom,
    hash::{Hash, Hasher},
    ops,
};
use tracing::{debug, warn};
use typed_index_collection::{Collection, CollectionWithId, Id, Idx};

/// Physical mode for Air
pub const AIR_PHYSICAL_MODE: &str = "Air";
/// Physical mode for Bike
pub const BIKE_PHYSICAL_MODE: &str = "Bike";
/// Physical mode for Bike Sharing Service
pub const BIKE_SHARING_SERVICE_PHYSICAL_MODE: &str = "BikeSharingService";
/// Physical mode for Bus
pub const BUS_PHYSICAL_MODE: &str = "Bus";
/// Physical mode for Rapid Bus
pub const BUS_RAPID_TRANSIT_PHYSICAL_MODE: &str = "BusRapidTransit";
/// Physical mode for Car
pub const CAR_PHYSICAL_MODE: &str = "Car";
/// Physical mode for Coach
pub const COACH_PHYSICAL_MODE: &str = "Coach";
/// Physical mode for Ferry
pub const FERRY_PHYSICAL_MODE: &str = "Ferry";
/// Physical mode for Funicular
pub const FUNICULAR_PHYSICAL_MODE: &str = "Funicular";
/// Physical mode for Local Train
pub const LOCAL_TRAIN_PHYSICAL_MODE: &str = "LocalTrain";
/// Physical mode for Long Distance Train
pub const LONG_DISTANCE_TRAIN_PHYSICAL_MODE: &str = "LongDistanceTrain";
/// Physical mode for Metro
pub const METRO_PHYSICAL_MODE: &str = "Metro";
/// Physical mode for Rapid Transit
pub const RAPID_TRANSIT_PHYSICAL_MODE: &str = "RapidTransit";
/// Physical mode for Taxi
pub const TAXI_PHYSICAL_MODE: &str = "Taxi";
/// Physical mode for Train
pub const TRAIN_PHYSICAL_MODE: &str = "Train";
/// Physical mode for Tramway
pub const TRAMWAY_PHYSICAL_MODE: &str = "Tramway";

/// The set of collections representing the model.
#[derive(Derivative, Serialize, Deserialize, Debug)]
#[derivative(Default)]
#[allow(missing_docs)]
pub struct Collections {
    pub contributors: CollectionWithId<Contributor>,
    pub datasets: CollectionWithId<Dataset>,
    pub networks: CollectionWithId<Network>,
    pub commercial_modes: CollectionWithId<CommercialMode>,
    pub lines: CollectionWithId<Line>,
    pub routes: CollectionWithId<Route>,
    pub vehicle_journeys: CollectionWithId<VehicleJourney>,
    pub frequencies: Collection<Frequency>,
    pub physical_modes: CollectionWithId<PhysicalMode>,
    pub stop_areas: CollectionWithId<StopArea>,
    pub stop_points: CollectionWithId<StopPoint>,
    pub stop_locations: CollectionWithId<StopLocation>,
    pub feed_infos: BTreeMap<String, String>,
    pub calendars: CollectionWithId<Calendar>,
    pub companies: CollectionWithId<Company>,
    pub comments: CollectionWithId<Comment>,
    pub equipments: CollectionWithId<Equipment>,
    pub transfers: Collection<Transfer>,
    pub trip_properties: CollectionWithId<TripProperty>,
    pub geometries: CollectionWithId<Geometry>,
    pub admin_stations: Collection<AdminStation>,
    #[serde(skip)]
    //HashMap<(vehicle_journey_id, stop_sequence), headsign>,
    pub stop_time_headsigns: HashMap<(String, u32), String>,
    #[serde(skip)]
    //HashMap<(vehicle_journey_id, stop_sequence), stop_time_id>,
    pub stop_time_ids: HashMap<(String, u32), String>,
    #[serde(skip)]
    //HashMap<(vehicle_journey_id, stop_sequence), comment_id>
    pub stop_time_comments: HashMap<(String, u32), String>,
    pub prices_v1: Collection<PriceV1>,
    pub od_fares_v1: Collection<OdFareV1>,
    pub fares_v1: Collection<FareV1>,
    pub tickets: CollectionWithId<Ticket>,
    pub ticket_uses: CollectionWithId<TicketUse>,
    pub ticket_prices: Collection<TicketPrice>,
    pub ticket_use_perimeters: Collection<TicketUsePerimeter>,
    pub ticket_use_restrictions: Collection<TicketUseRestriction>,
    pub pathways: CollectionWithId<Pathway>,
    pub levels: CollectionWithId<Level>,
    pub grid_calendars: CollectionWithId<GridCalendar>,
    pub grid_exception_dates: Collection<GridExceptionDate>,
    pub grid_periods: Collection<GridPeriod>,
    pub grid_rel_calendar_line: Collection<GridRelCalendarLine>,
    pub addresses: CollectionWithId<Address>,
    pub occupancies: Collection<Occupancy>,
}

impl Collections {
    /// Remove associated schedules with route points
    pub fn remove_route_points(&mut self) {
        let is_route_point = |stop_time: &StopTime| -> bool {
            stop_time.pickup_type == 3 || stop_time.drop_off_type == 3
        };
        for vj_idx in self.vehicle_journeys.indexes() {
            let mut vj = self.vehicle_journeys.index_mut(vj_idx);
            vj.stop_times.retain(|stop_time| !is_route_point(stop_time));
            vj.stop_times.shrink_to_fit();
        }
    }

    /// Remove stop zone
    pub fn remove_stop_zones(&mut self) {
        self.stop_points.retain(|sp| sp.stop_type != StopType::Zone);

        let stop_point_ids: Vec<Idx<StopPoint>> =
            self.stop_points.get_id_to_idx().values().copied().collect();

        self.vehicle_journeys.retain(|vj| {
            vj.stop_times
                .iter()
                .all(|st| stop_point_ids.contains(&st.stop_point_idx))
        });
    }

    /// Restrict the validity period of the current `Collections` with the start_date and end_date
    pub fn restrict_period(&mut self, start_date: NaiveDate, end_date: NaiveDate) -> Result<()> {
        let mut calendars = self.calendars.take();
        for calendar in calendars.iter_mut() {
            calendar.dates = calendar
                .dates
                .iter()
                .cloned()
                .filter(|date| *date >= start_date && *date <= end_date)
                .collect();
        }
        let mut data_sets = self.datasets.take();
        for data_set in data_sets.iter_mut() {
            data_set.start_date = cmp::max(start_date, data_set.start_date);
            data_set.end_date = cmp::min(end_date, data_set.end_date);
        }
        self.datasets = CollectionWithId::new(data_sets)?;
        self.calendars = CollectionWithId::new(calendars)?;
        Ok(())
    }

    /// Keep the collections consistent for the new model by purging unreferenced data by
    /// calendars
    pub fn sanitize(&mut self) -> Result<()> {
        fn log_object_removed(object_type: &str, id: &str) {
            debug!("{} with ID {} has been removed", object_type, id);
        }
        fn log_predicate<'a, T, F>(object_type: &'a str, mut f: F) -> impl 'a + FnMut(&T) -> bool
        where
            T: Id<T>,
            F: 'a + FnMut(&T) -> bool,
        {
            move |item| {
                if f(item) {
                    true
                } else {
                    log_object_removed(object_type, item.id());
                    false
                }
            }
        }

        fn dedup_collection<T: Clone + Eq + Hash>(source: &mut Collection<T>) -> Collection<T> {
            let calculate_hash = |t: &T| -> u64 {
                let mut s = DefaultHasher::new();
                t.hash(&mut s);
                s.finish()
            };
            let mut set: BTreeMap<u64, T> = BTreeMap::new();
            let items = source.take();
            for item in items {
                set.insert(calculate_hash(&item), item);
            }
            let collection: Vec<T> = set.values().cloned().collect();
            Collection::new(collection)
        }

        self.calendars
            .retain(log_predicate("Calendar", |cal: &Calendar| {
                !cal.dates.is_empty()
            }));

        let mut geometries_used = HashSet::<String>::new();
        let mut companies_used = HashSet::<String>::new();
        let mut trip_properties_used = HashSet::<String>::new();
        let mut route_ids_used = HashSet::<String>::new();
        let mut stop_points_used = HashSet::<String>::new();
        let mut data_sets_used = HashSet::<String>::new();
        let mut physical_modes_used = HashSet::<String>::new();
        let mut comments_used = HashSet::<String>::new();
        let mut level_id_used = HashSet::<String>::new();
        let mut calendars_used = HashSet::<String>::new();
        let mut vjs_used = HashSet::<String>::new();
        let mut addresses_used = HashSet::<String>::new();

        for &fallback_mode in enhancers::FALLBACK_PHYSICAL_MODES.iter() {
            physical_modes_used.insert(fallback_mode.to_string());
        }

        let stop_point_id_to_old_idx = self.stop_points.get_id_to_idx().clone();

        let mut vjs: Vec<VehicleJourney> = self.vehicle_journeys.take();
        vjs.retain(|vj| {
            if vj.stop_times.is_empty() {
                return false;
            }
            if vj.stop_times.len() == 1 {
                warn!("vehicle journey {} only have 1 stop time", vj.id);
            }
            if self.calendars.contains_id(&vj.service_id) {
                calendars_used.insert(vj.service_id.clone());
                if let Some(geo_id) = &vj.geometry_id {
                    geometries_used.insert(geo_id.clone());
                }
                if let Some(prop_id) = &vj.trip_property_id {
                    trip_properties_used.insert(prop_id.clone());
                }
                companies_used.insert(vj.company_id.clone());
                route_ids_used.insert(vj.route_id.clone());
                for stop_time in &vj.stop_times {
                    stop_points_used.insert(self.stop_points[stop_time.stop_point_idx].id.clone());
                }
                data_sets_used.insert(vj.dataset_id.clone());
                physical_modes_used.insert(vj.physical_mode_id.clone());
                comments_used.extend(&mut vj.comment_links.iter().map(|cl| cl.to_string()));
                vjs_used.insert(vj.id.clone());
                true
            } else {
                log_object_removed("Vehicle Journey", &vj.id);
                false
            }
        });
        let mut line_ids_used: HashSet<String> = HashSet::new();
        let routes = self
            .routes
            .take()
            .into_iter()
            .filter(|r| {
                if route_ids_used.contains(&r.id) {
                    if let Some(geo_id) = &r.geometry_id {
                        geometries_used.insert(geo_id.clone());
                    }
                    line_ids_used.insert(r.line_id.clone());
                    comments_used.extend(&mut r.comment_links.iter().map(|cl| cl.to_string()));
                    true
                } else {
                    log_object_removed("Route", &r.id);
                    false
                }
            })
            .collect::<Vec<_>>();
        let mut stop_area_ids_used: HashSet<String> = HashSet::new();
        let mut equipments_used: HashSet<String> = HashSet::new();

        let mut stop_locations = self
            .stop_locations
            .take()
            .into_iter()
            .filter(|sl| {
                if sl.stop_type == StopType::StopEntrance || sl.stop_type == StopType::GenericNode {
                    if let Some(stop_area_id) = &sl.parent_id {
                        stop_area_ids_used.insert(stop_area_id.clone());
                    }
                }
                if sl.stop_type == StopType::BoardingArea {
                    if let Some(stop_point_id) = &sl.parent_id {
                        stop_points_used.insert(stop_point_id.clone());
                        if let Some(stop_area_id) = self
                            .stop_points
                            .get(stop_point_id)
                            .map(|sp| sp.stop_area_id.clone())
                        {
                            stop_area_ids_used.insert(stop_area_id);
                        }
                    }
                }
                if let Some(level_id) = &sl.level_id {
                    level_id_used.insert(level_id.clone());
                }
                if let Some(equipment_id) = &sl.equipment_id {
                    equipments_used.insert(equipment_id.clone());
                }
                comments_used.extend(&mut sl.comment_links.iter().map(|cl| cl.to_string()));
                true
            })
            .collect::<Vec<_>>();

        let pathways = self
            .pathways
            .take()
            .into_iter()
            .filter(|pw| {
                let mut insert_if_used = |stop_type: &StopType, stop_id: &String| {
                    if *stop_type == StopType::BoardingArea || *stop_type == StopType::Point {
                        stop_points_used.insert(stop_id.clone());
                        if let Some(stop_area_id) = self
                            .stop_points
                            .get(stop_id)
                            .map(|sp| sp.stop_area_id.clone())
                        {
                            stop_area_ids_used.insert(stop_area_id);
                        }
                    }
                };
                insert_if_used(&pw.from_stop_type, &pw.from_stop_id);
                insert_if_used(&pw.to_stop_type, &pw.to_stop_id);
                true
            })
            .collect::<Vec<_>>();
        self.pathways = CollectionWithId::new(pathways)?;

        let mut stop_points = self
            .stop_points
            .take()
            .into_iter()
            .filter(|sp| {
                if stop_points_used.contains(&sp.id) {
                    stop_area_ids_used.insert(sp.stop_area_id.clone());
                    if let Some(geo_id) = &sp.geometry_id {
                        geometries_used.insert(geo_id.clone());
                    }
                    if let Some(equipment_id) = &sp.equipment_id {
                        equipments_used.insert(equipment_id.clone());
                    }
                    if let Some(level_id) = &sp.level_id {
                        level_id_used.insert(level_id.clone());
                    }
                    comments_used.extend(&mut sp.comment_links.iter().map(|cl| cl.to_string()));
                    if let Some(address_id) = &sp.address_id {
                        addresses_used.insert(address_id.clone());
                    }
                    true
                } else {
                    log_object_removed("Stop Point", &sp.id);
                    false
                }
            })
            .collect::<Vec<_>>();

        let mut networks_used: HashSet<String> = HashSet::new();
        let mut commercial_modes_used: HashSet<String> = HashSet::new();
        let lines = self
            .lines
            .take()
            .into_iter()
            .filter(|l| {
                if line_ids_used.contains(&l.id) {
                    if let Some(geo_id) = &l.geometry_id {
                        geometries_used.insert(geo_id.clone());
                    }
                    networks_used.insert(l.network_id.clone());
                    commercial_modes_used.insert(l.commercial_mode_id.clone());
                    comments_used.extend(&mut l.comment_links.iter().map(|cl| cl.to_string()));
                    true
                } else {
                    log_object_removed("Line", &l.id);
                    false
                }
            })
            .collect::<Vec<_>>();
        let mut contributors_used: HashSet<String> = HashSet::new();
        self.datasets = CollectionWithId::new(
            self.datasets
                .take()
                .into_iter()
                .filter(|d| {
                    if data_sets_used.contains(&d.id) {
                        contributors_used.insert(d.contributor_id.clone());
                        true
                    } else {
                        log_object_removed("Dataset", &d.id);
                        false
                    }
                })
                .collect(),
        )?;
        let mut stop_areas = self
            .stop_areas
            .take()
            .into_iter()
            .filter(|sa| {
                if stop_area_ids_used.contains(&sa.id) {
                    if let Some(geo_id) = &sa.geometry_id {
                        geometries_used.insert(geo_id.clone());
                    }
                    if let Some(level_id) = &sa.level_id {
                        level_id_used.insert(level_id.clone());
                    }
                    if let Some(equipment_id) = &sa.equipment_id {
                        equipments_used.insert(equipment_id.clone());
                    }
                    comments_used.extend(&mut sa.comment_links.iter().map(|cl| cl.to_string()));
                    true
                } else {
                    log_object_removed("Stop Area", &sa.id);
                    false
                }
            })
            .collect::<Vec<_>>();

        for sa in stop_areas.iter_mut().filter(|sa| !sa.coord.is_valid()) {
            debug!(
                "stop area {} geographic coordinates are not valid and set to (0, 0)",
                sa.id
            );
            sa.coord = Coord::default();
        }

        for sp in stop_points.iter_mut().filter(|sp| !sp.coord.is_valid()) {
            debug!(
                "stop point {} geographic coordinates are not valid and set to (0, 0)",
                sp.id
            );
            sp.coord = Coord::default();
        }

        for sl in stop_locations
            .iter_mut()
            .filter(|sl| sl.stop_type == StopType::StopEntrance && !sl.coord.is_valid())
        {
            debug!(
                "stop access {} geographic coordinates are not valid and set to (0, 0)",
                sl.id
            );
            sl.coord = Coord::default();
        }

        comments_used.extend(self.stop_time_comments.iter().filter_map(
            |((vj_id, _), comment_id)| {
                if vjs_used.contains(vj_id.as_str()) {
                    Some(comment_id.clone())
                } else {
                    None
                }
            },
        ));

        self.comments
            .retain(log_predicate("Comment", |comment: &Comment| {
                comments_used.contains(&comment.id)
            }));

        self.lines = CollectionWithId::new(lines)?;
        self.stop_points = CollectionWithId::new(stop_points)?;
        let stop_point_old_idx_to_new_idx: HashMap<Idx<StopPoint>, Idx<StopPoint>> = self
            .stop_points
            .iter()
            .map(|(new_idx, stop_point)| (stop_point_id_to_old_idx[&stop_point.id], new_idx))
            .collect();
        for vj in vjs.iter_mut() {
            for st in vj.stop_times.iter_mut() {
                st.stop_point_idx = stop_point_old_idx_to_new_idx[&st.stop_point_idx];
            }
        }
        self.stop_areas = CollectionWithId::new(stop_areas)?;
        self.routes = CollectionWithId::new(routes)?;
        let vehicle_journeys_used: HashSet<String> = vjs.iter().map(|vj| vj.id.clone()).collect();
        self.vehicle_journeys = CollectionWithId::new(vjs)?;
        self.stop_locations = CollectionWithId::new(stop_locations)?;
        self.stop_time_comments.retain(|(vj_id, _), comment_id| {
            vehicle_journeys_used.contains(vj_id) && comments_used.contains(comment_id)
        });
        self.stop_time_ids
            .retain(|(vj_id, _), _| vehicle_journeys_used.contains(vj_id));
        self.stop_time_headsigns
            .retain(|(vj_id, _), _| vehicle_journeys_used.contains(vj_id));
        self.grid_rel_calendar_line
            .retain(|grid_rel_calendar_line| {
                line_ids_used.contains(&grid_rel_calendar_line.line_id)
                    // If `line_external_code` is used,
                    // it is not possible to sanitize without the exact `line` identifier
                    || (grid_rel_calendar_line.line_id.is_empty() && grid_rel_calendar_line.line_external_code.is_some())
            });
        let grid_calendar_id_used: Vec<_> = self
            .grid_rel_calendar_line
            .values()
            .map(|grid_rel_calendar_line| grid_rel_calendar_line.grid_calendar_id.clone())
            .collect();
        self.grid_calendars.retain(log_predicate(
            "GridCalendar",
            |grid_calendar: &GridCalendar| grid_calendar_id_used.contains(&grid_calendar.id),
        ));
        self.grid_exception_dates.retain(|grid_exception_date| {
            grid_calendar_id_used.contains(&grid_exception_date.grid_calendar_id)
        });
        self.grid_periods
            .retain(|grid_period| grid_calendar_id_used.contains(&grid_period.grid_calendar_id));

        self.networks
            .retain(log_predicate("Network", |network: &Network| {
                networks_used.contains(&network.id)
            }));
        self.trip_properties.retain(log_predicate(
            "Trip Property",
            |trip_property: &TripProperty| trip_properties_used.contains(&trip_property.id),
        ));
        self.geometries
            .retain(log_predicate("Geometry", |geometry: &Geometry| {
                geometries_used.contains(&geometry.id)
            }));
        self.companies
            .retain(log_predicate("Company", |company: &Company| {
                companies_used.contains(&company.id)
            }));
        self.equipments
            .retain(log_predicate("Equipment", |equipment: &Equipment| {
                equipments_used.contains(&equipment.id)
            }));
        self.contributors
            .retain(log_predicate("Contributor", |contributor: &Contributor| {
                contributors_used.contains(&contributor.id)
            }));
        self.commercial_modes.retain(log_predicate(
            "Commercial Mode",
            |commercial_mode: &CommercialMode| commercial_modes_used.contains(&commercial_mode.id),
        ));
        self.physical_modes.retain(log_predicate(
            "Physical Mode",
            |physical_mode: &PhysicalMode| physical_modes_used.contains(&physical_mode.id),
        ));
        self.transfers.retain(|t| {
            stop_points_used.contains(&t.from_stop_id) && stop_points_used.contains(&t.to_stop_id)
        });
        self.frequencies
            .retain(|frequency| vehicle_journeys_used.contains(&frequency.vehicle_journey_id));
        self.levels
            .retain(|level| level_id_used.contains(&level.id));
        self.calendars.retain(|c| calendars_used.contains(&c.id));
        self.addresses
            .retain(|address| addresses_used.contains(&address.id));
        self.admin_stations
            .retain(|admin_station| stop_area_ids_used.contains(&admin_station.stop_id));
        self.occupancies.retain(|occupancy| {
            if !line_ids_used.contains(&occupancy.line_id) {
                debug!(
                    "Occupancy with 'line_id={}' has been removed because line is not used",
                    occupancy.line_id
                );
                return false;
            }
            if !stop_area_ids_used.contains(&occupancy.from_stop_area) {
                debug!(
                    "Occupancy with 'from_stop_area={}' has been removed because stop area is not used",
                    occupancy.from_stop_area
                );
                return false;
            }
            if !stop_area_ids_used.contains(&occupancy.to_stop_area){
                debug!(
                    "Occupancy with 'to_stop_area={}' has been removed because stop area is not used",
                    occupancy.to_stop_area
                );
                return false;
            }
            true
        });

        self.frequencies = dedup_collection(&mut self.frequencies);
        self.transfers = dedup_collection(&mut self.transfers);
        self.admin_stations = dedup_collection(&mut self.admin_stations);
        self.prices_v1 = dedup_collection(&mut self.prices_v1);
        self.od_fares_v1 = dedup_collection(&mut self.od_fares_v1);
        self.fares_v1 = dedup_collection(&mut self.fares_v1);
        self.ticket_prices = dedup_collection(&mut self.ticket_prices);
        self.ticket_use_perimeters = dedup_collection(&mut self.ticket_use_perimeters);
        self.ticket_use_restrictions = dedup_collection(&mut self.ticket_use_restrictions);
        self.grid_exception_dates = dedup_collection(&mut self.grid_exception_dates);
        self.grid_periods = dedup_collection(&mut self.grid_periods);
        self.grid_rel_calendar_line = dedup_collection(&mut self.grid_rel_calendar_line);
        self.occupancies = dedup_collection(&mut self.occupancies);

        Ok(())
    }

    /// Sets the opening and closing times of lines (if they are missing).
    pub fn enhance_line_opening_time(&mut self) {
        type TimeTable = BTreeMap<u8, Time>;
        const HOURS_PER_DAY: u8 = 24;
        const SECONDS_PER_DAY: u32 = 86400;

        fn get_vjs_by_line(c: &Collections) -> HashMap<String, IdxSet<VehicleJourney>> {
            c.vehicle_journeys
                .iter()
                .filter_map(|(vj_idx, vj)| {
                    c.routes
                        .get(&vj.route_id)
                        .map(|route| (route.line_id.clone(), vj_idx))
                })
                .filter_map(|(line_id, vj_idx)| {
                    if let Some(line) = c.lines.get(&line_id) {
                        if line.opening_time.is_none() || line.closing_time.is_none() {
                            return Some((line.id.clone(), vj_idx));
                        }
                    }
                    None
                })
                .fold(HashMap::new(), |mut lines, (line_id, vj_idx)| {
                    lines
                        .entry(line_id)
                        .or_insert_with(IdxSet::new)
                        .insert(vj_idx);
                    lines
                })
        }

        // Create a map to store all arrival and departure times per frequency vehicle.
        // This kind of vehicle has several departure and arrival times;
        // for its multiple possible frequencies (ex 08:00->12:00 and 14:00->19:00)
        // and for each of them, at each iteration (ex for a 40mn trip, 08:00->08:40, 08:10->08:50,...)
        fn get_departures_arrivals_by_frequenced_vj(
            c: &Collections,
        ) -> HashMap<Idx<VehicleJourney>, Vec<(Time, Time)>> {
            c.frequencies
                .values()
                .filter_map(|frequency| {
                    c.vehicle_journeys
                        .get_idx(&frequency.vehicle_journey_id)
                        .map(|vj_idx| (vj_idx, frequency))
                })
                .filter_map(|(vj_idx, frequency)| {
                    c.vehicle_journeys[vj_idx]
                        .first_departure_time()
                        .zip(c.vehicle_journeys[vj_idx].last_arrival_time())
                        .map(|(first_departure, last_arrival)| {
                            (vj_idx, frequency, last_arrival - first_departure)
                        })
                })
                .fold(
                    HashMap::new(),
                    |mut departures_arrivals, (vj_idx, frequency, trip_duration)| {
                        let mut start_time = frequency.start_time;
                        while start_time <= frequency.end_time {
                            let end_time = start_time + trip_duration;
                            departures_arrivals
                                .entry(vj_idx)
                                .or_insert_with(Vec::new)
                                .push((start_time % SECONDS_PER_DAY, end_time % SECONDS_PER_DAY));
                            start_time = start_time + Time::new(0, 0, frequency.headway_secs);
                        }
                        departures_arrivals
                    },
                )
        }

        // Creates a map of (maximum) 24 elements, with possible indexes ranging from 0 to 23.
        // 2 timetables are created, one to store departures and the other for arrivals.
        // Indeed, for a line/vehicle journey, the distance between two consecutive stops can take more than an hour.
        //
        // For opening_timetable we keep the smallest schedule of the first departure for each time slot.
        // For closing_timetable we keep the biggest schedule of the last arrival for each time slot.
        //
        // Example for a line with a vehicle journey leaving every half hour between 8:10am and 10:40am
        // (same vehicle journey, with a frequency of 30mn, so 6 vehicles journeys in total).
        // opening_timetable will be so {8: Time(29400), 9: Time(33000), 10: Time(36600)}.
        // Departures 8:40am, 9:40am and 10:40am are omitted (because superior in their respective slots).
        fn fill_timetables(
            opening_timetable: &mut TimeTable,
            closing_timetable: &mut TimeTable,
            vj_departure_time: Time,
            vj_arrival_time: Time,
        ) -> Result<()> {
            let departure_hour = u8::try_from(vj_departure_time.hours())?;
            let arrival_hour = u8::try_from(vj_arrival_time.hours())?;
            opening_timetable
                .entry(departure_hour)
                .and_modify(|h| {
                    if vj_departure_time < *h {
                        *h = vj_departure_time
                    }
                })
                .or_insert(vj_departure_time);
            closing_timetable
                .entry(arrival_hour)
                .and_modify(|h| {
                    if vj_arrival_time > *h {
                        *h = vj_arrival_time
                    }
                })
                .or_insert(vj_arrival_time);
            Ok(())
        }

        // Find the main hole for a line (i.e. without traffic), based on the timetable parameter.
        // For example a line with vjs running from 04:10 to 22:45 will give this segment: [[23,24,0,1,2,3]]
        // Several holes are possible in a day, such as: [[23,24,0,1,2,3],[12,13]]
        // This function finds the largest and returns the two bounds/index (23 and 3 in the first example)
        fn find_main_hole_boundaries(timetable: &TimeTable) -> Option<(u8, u8)> {
            let mut holes: Vec<Vec<u8>> = Vec::new();
            let mut is_last_elem_hole = false;
            for i in 0..HOURS_PER_DAY {
                if !timetable.contains_key(&i) {
                    if !is_last_elem_hole {
                        holes.push(vec![i]);
                        is_last_elem_hole = true;
                    } else if let Some(last) = holes.last_mut() {
                        last.push(i)
                    }
                    // for the midnight passing, concatenate the vectors
                    // [0,1] and [22,23] will become [22,23,0,1]
                    if i == HOURS_PER_DAY - 1
                        && holes.len() > 1
                        && holes.get(0).filter(|h0| h0.contains(&0)).is_some()
                    {
                        let hole0 = holes[0].clone();
                        if let Some(last) = holes.last_mut() {
                            last.extend_from_slice(&hole0)
                        }
                        holes.remove(0);
                    }
                } else {
                    is_last_elem_hole = false;
                }
            }
            // *** first, sorts in descending order of width
            // [[9,10],[12,13,14],[23,0,1]] --> [[12,13,14],[23,0,1],[9,10]]
            // *** then, in case of equal width, sorts by taking the segment early in the morning (smallest index)
            // --> [[23,0,1],[12,13,14],[9,10]]
            #[allow(clippy::unnecessary_sort_by)] // key borrows, so lint is "wrong"
            holes.sort_unstable_by(|l, r| l.iter().min().cmp(&r.iter().min()));
            holes.sort_by_key(|v| Reverse(v.len()));
            holes.first().and_then(|mh| {
                let first_idx = mh.first();
                let last_idx = mh.last();
                match (first_idx, last_idx) {
                    (Some(first_idx), Some(last_idx)) => {
                        Some((first_idx.to_owned(), last_idx.to_owned()))
                    }
                    _ => None,
                }
            })
        }

        fn get_vj_departure_arrival(vj: &VehicleJourney) -> Result<(Time, Time)> {
            let vj_departure_time = vj
                .first_departure_time()
                .map(|departure_time| departure_time % SECONDS_PER_DAY)
                .ok_or_else(|| anyhow!("undefined departure time for vj {}", vj.id))?;
            let vj_arrival_time = vj
                .last_arrival_time()
                .map(|arrival_time| arrival_time % SECONDS_PER_DAY)
                .ok_or_else(|| anyhow!("undefined arrival time for vj {}", vj.id))?;
            Ok((vj_departure_time, vj_arrival_time))
        }

        // Check if there is a need to calculate the opening/closing times.
        // Generally it should be all or nothing (absent from a Gtfs, normally present if it's a recent Ntfs)
        // In all cases a 2nd check is made below
        let check_time_empty =
            |line: &Line| line.opening_time.is_none() || line.closing_time.is_none();
        let required_operation = self.lines.values().any(|line| check_time_empty(line));

        if required_operation {
            let vjs_by_line = get_vjs_by_line(self);
            let departures_arrivals_by_frequenced_vj =
                get_departures_arrivals_by_frequenced_vj(self);
            let mut lines = self.lines.take();
            for line in &mut lines {
                // 2nd check (see above) to avoid overwriting line opening/closing
                if check_time_empty(line) {
                    let mut opening_timetable = TimeTable::new();
                    let mut closing_timetable = TimeTable::new();
                    if let Some(vjs_idx) = vjs_by_line.get(&line.id) {
                        for vj_idx in vjs_idx {
                            departures_arrivals_by_frequenced_vj
                                .get(vj_idx)
                                .cloned()
                                .or_else(|| {
                                    get_vj_departure_arrival(&self.vehicle_journeys[*vj_idx])
                                        .map_or_else(
                                            |e| {
                                                warn!("{e}");
                                                None
                                            },
                                            |(departure, arrival)| Some(vec![(departure, arrival)]),
                                        )
                                })
                                .into_iter()
                                .flatten()
                                .for_each(|(vj_departure_time, vj_arrival_time)| {
                                    let result = fill_timetables(
                                        &mut opening_timetable,
                                        &mut closing_timetable,
                                        vj_departure_time,
                                        vj_arrival_time,
                                    );
                                    if let Err(e) = result {
                                        warn!("{e}");
                                    }
                                });
                        }
                    }
                    line.opening_time = find_main_hole_boundaries(&opening_timetable)
                        .map(|mhb| mhb.1) // gets the last index of the main hole
                        .map(|lmhb| (lmhb + 1) % HOURS_PER_DAY) // gets the index just after that of the main hole
                        .and_then(|ot_idx| opening_timetable.get(&ot_idx))
                        .copied()
                        .or_else(|| Some(Time::new(0, 0, 0))); // continuous circulation or absent (and later cleaned)
                    line.closing_time = find_main_hole_boundaries(&closing_timetable)
                        .map(|mhb| mhb.0) // gets the first index of the main hole
                        .map(|fmhb| (fmhb + HOURS_PER_DAY - 1) % HOURS_PER_DAY) // gets the index just before that of the main hole
                        .and_then(|ct_idx| closing_timetable.get(&ct_idx))
                        .copied()
                        .or_else(|| Some(Time::new(23, 59, 59))) // continuous circulation or absent (and later cleaned)
                        .map(|mut closing_time| {
                            // Add one day if opening_time > closing_time (midnight-passing)
                            if let Some(opening_time) = line.opening_time {
                                if opening_time > closing_time {
                                    closing_time =
                                        closing_time + Time::new(HOURS_PER_DAY.into(), 0, 0);
                                }
                            }
                            closing_time
                        });
                }
            }
            self.lines = CollectionWithId::new(lines).unwrap();
        }
    }

    /// If the vehicle didn't stop (point of route) on pickup,
    /// it must not stop on drop off and conversely
    pub fn pickup_drop_off_harmonisation(&mut self) {
        for vj_idx in self.vehicle_journeys.indexes() {
            let mut vj = self.vehicle_journeys.index_mut(vj_idx);
            for stop_time in vj
                .stop_times
                .iter_mut()
                .filter(|stop_time| stop_time.pickup_type == 3 || stop_time.drop_off_type == 3)
            {
                stop_time.pickup_type = 3;
                stop_time.drop_off_type = 3;
            }
        }
    }

    /// Trip headsign can be derived from the name of the stop point of the
    /// last stop time of the associated trip.
    pub fn enhance_trip_headsign(&mut self) {
        let mut vehicle_journeys = self.vehicle_journeys.take();
        for vehicle_journey in &mut vehicle_journeys {
            if vehicle_journey
                .headsign
                .as_ref()
                .filter(|s| !s.is_empty())
                .is_none()
            {
                vehicle_journey.headsign = vehicle_journey
                    .stop_times
                    .last()
                    .map(|stop_time| self.stop_points[stop_time.stop_point_idx].name.clone());
            }
        }
        self.vehicle_journeys = CollectionWithId::new(vehicle_journeys).unwrap();
    }

    /// Many calendars are identical and can be deduplicate
    pub fn calendar_deduplication(&mut self) {
        let mut calendars_used: Vec<Calendar> = vec![];
        let mut vehicle_journeys = self.vehicle_journeys.take();
        vehicle_journeys.sort_unstable_by(|vj1, vj2| vj1.service_id.cmp(&vj2.service_id));
        for vehicle_journey in &mut vehicle_journeys {
            if let Some(calendar) = self.calendars.get(&vehicle_journey.service_id) {
                if let Some(dup_calendar) =
                    calendars_used.iter().find(|c| c.dates == calendar.dates)
                {
                    vehicle_journey.service_id = dup_calendar.id.clone();
                } else {
                    calendars_used.push(calendar.clone());
                }
            }
        }
        self.calendars
            .retain(|calendar| calendars_used.contains(calendar));
        self.vehicle_journeys = CollectionWithId::new(vehicle_journeys).unwrap();
    }

    /// Some comments are identical and can be deduplicated
    pub fn comment_deduplication(&mut self) {
        let duplicate2ref = self.get_comment_map_duplicate_to_referent();
        if duplicate2ref.is_empty() {
            return;
        }

        replace_comment_duplicates_by_ref(&mut self.lines, &duplicate2ref);
        replace_comment_duplicates_by_ref(&mut self.routes, &duplicate2ref);
        replace_comment_duplicates_by_ref(&mut self.stop_areas, &duplicate2ref);
        replace_comment_duplicates_by_ref(&mut self.stop_points, &duplicate2ref);
        replace_comment_duplicates_by_ref(&mut self.stop_locations, &duplicate2ref);

        fn replace_comment_duplicates_by_ref<T>(
            collection: &mut CollectionWithId<T>,
            duplicate2ref: &BTreeMap<String, String>,
        ) where
            T: Id<T> + CommentLinks,
        {
            let map_pt_object_duplicates: BTreeMap<Idx<T>, Vec<&str>> = collection
                .iter()
                .filter_map(|(idx, pt_object)| {
                    let intersection: Vec<&str> = pt_object
                        .comment_links()
                        .iter()
                        .filter_map(|comment_id| {
                            duplicate2ref
                                .get_key_value(comment_id)
                                .map(|(duplicate_id_ref, _)| duplicate_id_ref.as_str())
                        })
                        .collect();
                    if !intersection.is_empty() {
                        Some((idx, intersection))
                    } else {
                        None
                    }
                })
                .collect();

            for (idx, intersection) in map_pt_object_duplicates {
                for i in intersection {
                    let mut pt_object = collection.index_mut(idx);
                    pt_object.comment_links_mut().remove(i);
                    pt_object
                        .comment_links_mut()
                        .insert(duplicate2ref[i].clone());
                }
            }
        }
    }

    /// Remove comments with empty message from the model
    pub fn clean_comments(&mut self) {
        fn remove_comment<T: Id<T> + CommentLinks>(
            collection: &mut CollectionWithId<T>,
            comment_id: &str,
        ) {
            let object_idxs: Vec<Idx<T>> = collection
                .iter()
                .filter_map(|(idx, object)| {
                    if object.comment_links().contains(comment_id) {
                        Some(idx)
                    } else {
                        None
                    }
                })
                .collect();
            for object_idx in object_idxs {
                collection
                    .index_mut(object_idx)
                    .comment_links_mut()
                    .remove(comment_id);
            }
        }
        let comments_to_del: Vec<Idx<Comment>> = self
            .comments
            .iter()
            .filter(|(_, comment)| comment.name.is_empty())
            .map(|(idx, _)| idx)
            .collect();
        for comment_idx in comments_to_del {
            let comment_id = &self.comments[comment_idx].id;
            remove_comment(&mut self.lines, comment_id);
            remove_comment(&mut self.routes, comment_id);
            remove_comment(&mut self.vehicle_journeys, comment_id);
            remove_comment(&mut self.stop_areas, comment_id);
            remove_comment(&mut self.stop_points, comment_id);
            remove_comment(&mut self.stop_locations, comment_id);
        }
    }

    /// From comment collection only, return a map of the similar comments.
    ///
    /// Result: duplicates (comments to be removed) are mapped to their similar
    /// referent (unique to be kept)
    fn get_comment_map_duplicate_to_referent(&self) -> BTreeMap<String, String> {
        let mut duplicate2ref = BTreeMap::<String, String>::new();
        // Map of the referent comments id (uniqueness given the similarity_key)
        let mut map_ref = HashMap::<&str, &str>::new();

        for comment in self.comments.values() {
            let similarity_key = comment.name.as_str(); // name only is considered
            if let Some(ref_id) = map_ref.get(similarity_key) {
                duplicate2ref.insert(comment.id.to_string(), ref_id.to_string());
            } else {
                map_ref.insert(similarity_key, &comment.id);
            }
        }
        duplicate2ref
    }

    /// If the route name is empty, it is derived from the most frequent
    /// `stop_area` origin and `stop_area` destination of all the associated
    /// trips.  The `stop_area` name is used to create the following `String`:
    /// `[most frequent origin] - [most frequent destination]`
    ///
    /// If 2 stops have equal frequency, the biggest `stop_area` (biggest number
    /// of `stop_point`) is chosen.
    ///
    /// If still equality between multiple `stop_area`, then alphabetical order
    /// of `stop_area`'s name is used.
    ///
    /// `route.destination_id` is also replaced with the destination stop area
    /// found with the above rules.
    pub fn enhance_route_names(
        &mut self,
        routes_to_vehicle_journeys: &impl Relation<From = Route, To = VehicleJourney>,
    ) {
        fn find_best_origin_destination<'a>(
            route_idx: Idx<Route>,
            collections: &'a Collections,
            routes_to_vehicle_journeys: &impl Relation<From = Route, To = VehicleJourney>,
        ) -> Result<(&'a StopArea, &'a StopArea)> {
            fn select_stop_areas<F>(
                collections: &Collections,
                vehicle_journey_idxs: &IdxSet<VehicleJourney>,
                select_stop_point_in_vj: F,
            ) -> Vec<Idx<StopArea>>
            where
                F: Fn(&VehicleJourney) -> Idx<StopPoint>,
            {
                vehicle_journey_idxs
                    .iter()
                    .map(|vj_idx| &collections.vehicle_journeys[*vj_idx])
                    .filter(|vj| !vj.stop_times.is_empty())
                    .map(select_stop_point_in_vj)
                    .map(|sp_idx| &collections.stop_points[sp_idx])
                    .map(|stop_point| &stop_point.stop_area_id)
                    .filter_map(|sa_id| collections.stop_areas.get_idx(sa_id))
                    .collect()
            }
            fn group_by_frequencies(
                stop_areas: Vec<Idx<StopArea>>,
            ) -> HashMap<Idx<StopArea>, usize> {
                stop_areas
                    .into_iter()
                    .fold(HashMap::new(), |mut frequencies, sa_idx| {
                        *frequencies.entry(sa_idx).or_insert(0) += 1;
                        frequencies
                    })
            }
            fn find_indexes_with_max_frequency(
                frequencies: HashMap<Idx<StopArea>, usize>,
            ) -> Vec<Idx<StopArea>> {
                if frequencies.is_empty() {
                    return Vec::new();
                }
                let mut max_frequency = *frequencies.values().next().unwrap();
                let mut max_indexes = Vec::new();
                for (idx, frequency) in frequencies {
                    match frequency.cmp(&max_frequency) {
                        Ordering::Greater => {
                            max_frequency = frequency;
                            max_indexes = vec![idx];
                        }
                        Ordering::Equal => max_indexes.push(idx),
                        Ordering::Less => {}
                    }
                }
                max_indexes
            }
            fn find_biggest_stop_areas(
                stop_area_indexes: Vec<Idx<StopArea>>,
                collections: &Collections,
            ) -> Vec<&StopArea> {
                if stop_area_indexes.is_empty() {
                    return Vec::new();
                }
                if stop_area_indexes.len() == 1 {
                    return vec![&collections.stop_areas[stop_area_indexes[0]]];
                }
                let mut max_sp_number = 0;
                let mut biggest_stop_areas = Vec::new();
                for sa_idx in stop_area_indexes {
                    let stop_area = &collections.stop_areas[sa_idx];
                    let sp_number = collections
                        .stop_points
                        .values()
                        .filter(|stop_point| stop_point.stop_area_id == stop_area.id)
                        .count();

                    match sp_number.cmp(&max_sp_number) {
                        Ordering::Greater => {
                            max_sp_number = sp_number;
                            biggest_stop_areas = vec![stop_area];
                        }
                        Ordering::Equal => biggest_stop_areas.push(stop_area),
                        Ordering::Less => {}
                    }
                }
                biggest_stop_areas
            }
            fn find_first_by_alphabetical_order(
                mut stop_areas: Vec<&StopArea>,
            ) -> Option<&StopArea> {
                stop_areas.sort_by_key(|stop_area| &stop_area.name);
                stop_areas.get(0).cloned()
            }
            fn find_best_stop_area_for<'a, F>(
                collections: &'a Collections,
                vehicle_journey_idxs: &IdxSet<VehicleJourney>,
                select_stop_point_in_vj: F,
            ) -> Option<&'a StopArea>
            where
                F: Fn(&VehicleJourney) -> Idx<StopPoint>,
            {
                let stop_areas: Vec<Idx<StopArea>> =
                    select_stop_areas(collections, vehicle_journey_idxs, select_stop_point_in_vj);
                let by_frequency: HashMap<Idx<StopArea>, usize> = group_by_frequencies(stop_areas);
                let most_frequent_stop_areas = find_indexes_with_max_frequency(by_frequency);
                let biggest_stop_areas =
                    find_biggest_stop_areas(most_frequent_stop_areas, collections);
                find_first_by_alphabetical_order(biggest_stop_areas)
            }
            let vehicle_journey_idxs = routes_to_vehicle_journeys
                .get_corresponding_forward(&std::iter::once(route_idx).collect());

            let origin_stop_area =
                find_best_stop_area_for(collections, &vehicle_journey_idxs, |vj| {
                    vj.stop_times[0].stop_point_idx
                });
            let destination_stop_area =
                find_best_stop_area_for(collections, &vehicle_journey_idxs, |vj| {
                    vj.stop_times[vj.stop_times.len() - 1].stop_point_idx
                });

            if let (Some(origin_stop_area), Some(destination_stop_area)) =
                (origin_stop_area, destination_stop_area)
            {
                Ok((origin_stop_area, destination_stop_area))
            } else {
                bail!(
                    "Failed to generate a `name` for route {}",
                    &collections.routes[route_idx].id
                )
            }
        }

        let mut route_names: BTreeMap<Idx<Route>, String> = BTreeMap::new();
        let mut route_destination_ids: BTreeMap<Idx<Route>, Option<String>> = BTreeMap::new();
        for (route_idx, route) in &self.routes {
            let no_route_name = route.name.is_empty();
            let no_destination_id = route.destination_id.is_none();
            if no_route_name || no_destination_id {
                let (origin, destination) = skip_error_and_warn!(find_best_origin_destination(
                    route_idx,
                    self,
                    routes_to_vehicle_journeys,
                ));
                if no_route_name
                    && !origin.name.trim().is_empty()
                    && !destination.name.trim().is_empty()
                {
                    let route_name = format!("{} - {}", origin.name, destination.name);
                    route_names.insert(route_idx, route_name);
                }
                if no_destination_id {
                    route_destination_ids.insert(route_idx, Some(destination.id.clone()));
                }
            }
        }
        for (route_idx, route_name) in route_names {
            self.routes.index_mut(route_idx).name = route_name;
        }
        for (route_idx, destination_id) in route_destination_ids {
            self.routes.index_mut(route_idx).destination_id = destination_id;
        }
    }

    /// If a route direction is empty, it's set by default with the "forward" value
    pub fn enhance_route_directions(&mut self) {
        let mut direction_types: BTreeMap<Idx<Route>, Option<String>> = BTreeMap::new();
        for (route_idx, _) in self
            .routes
            .iter()
            .filter(|(_, r)| r.direction_type.is_none())
        {
            direction_types.insert(route_idx, Some(String::from("forward")));
        }
        for (route_idx, direction_type) in direction_types {
            self.routes.index_mut(route_idx).direction_type = direction_type;
        }
    }

    /// Compute the coordinates of stop areas according to the centroid of stop points
    /// if the stop area has no coordinates (lon = 0, lat = 0)
    fn update_stop_area_coords(&mut self) {
        let mut updated_stop_areas = self.stop_areas.take();
        for stop_area in &mut updated_stop_areas
            .iter_mut()
            .filter(|sa| sa.coord == Coord::default())
        {
            if let Some(coord) = self
                .stop_points
                .values()
                .filter(|sp| sp.stop_area_id == stop_area.id)
                .map(|sp| (sp.coord.lon, sp.coord.lat))
                .collect::<MultiPoint<_>>()
                .centroid()
                .map(|c| Coord {
                    lon: c.x(),
                    lat: c.y(),
                })
            {
                stop_area.coord = coord;
            } else {
                warn!("failed to calculate a centroid of stop area {} because it does not refer to any corresponding stop point", stop_area.id)
            }
        }

        self.stop_areas = CollectionWithId::new(updated_stop_areas).unwrap();
    }

    /// Check that all references to geometries actually points towards existing
    /// `Geometry`. This is a common problem where a `geometries.txt` is read in
    /// NTFS, a line of this file is not a valid WKT format, then the `Geometry`
    /// is not created. However, the object that references this `Geometry` will
    /// very likely be created (for example, a `Route` from `routes.txt`). This
    /// creates an incoherent model where a `Route` points to a `Geometry` which
    /// doesn't exist.
    ///
    /// This function checks that all objects points to existing `Geometry` and,
    /// in the case it doesn't, fix the model by removing this pointer.
    fn check_geometries_coherence(&mut self) {
        macro_rules! check_and_fix_object_geometries {
            ($collection:expr) => {
                let objects_to_fix: Vec<String> = $collection
                    .values()
                    .filter(|object| {
                        object
                            .geometry_id
                            .as_ref()
                            .map(|geometry_id| self.geometries.get(geometry_id).is_none())
                            .unwrap_or(false)
                    })
                    .map(|object| object.id.clone())
                    .collect();
                for object_id in objects_to_fix {
                    $collection.get_mut(&object_id).unwrap().geometry_id = None;
                }
            };
        }
        check_and_fix_object_geometries!(self.lines);
        check_and_fix_object_geometries!(self.routes);
        check_and_fix_object_geometries!(self.vehicle_journeys);
        check_and_fix_object_geometries!(self.stop_points);
        check_and_fix_object_geometries!(self.stop_areas);
    }

    /// Calculate the validity period in the 'Model'.
    /// The calculation is based on the minimum start date and the maximum end
    /// date of all the datasets.
    /// If no dataset is found, an error is returned.
    pub fn calculate_validity_period(&self) -> Result<(Date, Date)> {
        let start_date = self
            .datasets
            .values()
            .map(|dataset| dataset.start_date)
            .min();
        let end_date = self.datasets.values().map(|dataset| dataset.end_date).max();
        start_date
            .zip(end_date)
            .ok_or_else(|| anyhow!("Cannot calculate validity period because there is no dataset"))
    }
}

/// The navitia transit model.
#[derive(GetCorresponding)]
pub struct Model {
    collections: Collections,

    // original relations
    networks_to_lines: OneToMany<Network, Line>,
    commercial_modes_to_lines: OneToMany<CommercialMode, Line>,
    lines_to_routes: OneToMany<Line, Route>,
    routes_to_vehicle_journeys: OneToMany<Route, VehicleJourney>,
    physical_modes_to_vehicle_journeys: OneToMany<PhysicalMode, VehicleJourney>,
    stop_areas_to_stop_points: OneToMany<StopArea, StopPoint>,
    contributors_to_datasets: OneToMany<Contributor, Dataset>,
    datasets_to_vehicle_journeys: OneToMany<Dataset, VehicleJourney>,
    companies_to_vehicle_journeys: OneToMany<Company, VehicleJourney>,
    vehicle_journeys_to_stop_points: ManyToMany<VehicleJourney, StopPoint>,
    transfers_to_stop_points: ManyToMany<Transfer, StopPoint>,
    calendars_to_vehicle_journeys: OneToMany<Calendar, VehicleJourney>,

    // shortcuts
    #[get_corresponding(weight = "1.9")]
    routes_to_stop_points: ManyToMany<Route, StopPoint>,
    #[get_corresponding(weight = "1.9")]
    physical_modes_to_stop_points: ManyToMany<PhysicalMode, StopPoint>,
    #[get_corresponding(weight = "1.9")]
    physical_modes_to_routes: ManyToMany<PhysicalMode, Route>,
    #[get_corresponding(weight = "1.9")]
    datasets_to_stop_points: ManyToMany<Dataset, StopPoint>,
    #[get_corresponding(weight = "1.9")]
    datasets_to_routes: ManyToMany<Dataset, Route>,
    #[get_corresponding(weight = "1.9")]
    datasets_to_physical_modes: ManyToMany<Dataset, PhysicalMode>,
}

impl Model {
    /// Constructs a model from the given `Collections`.  Fails in
    /// case of incoherence, as invalid external references.
    ///
    /// # Examples
    ///
    /// ```
    /// # use transit_model::model::*;
    /// # fn run() -> transit_model::Result<()> {
    /// let _: Model = Model::new(Collections::default())?;
    /// # Ok(())
    /// # }
    /// # run().unwrap()
    /// ```
    ///
    /// ```
    /// # use transit_model::model::*;
    /// # use typed_index_collection::Collection;
    /// # use transit_model::objects::Transfer;
    /// let mut collections = Collections::default();
    /// // This transfer is invalid as there is no stop points in collections
    /// // but objects not referenced are removed from the model
    /// collections.transfers = Collection::from(Transfer {
    ///     from_stop_id: "invalid".into(),
    ///     to_stop_id: "also_invalid".into(),
    ///     min_transfer_time: None,
    ///     real_min_transfer_time: None,
    ///     equipment_id: None,
    /// });
    /// assert!(Model::new(collections).is_ok());
    /// ```
    pub fn new(mut c: Collections) -> Result<Self> {
        enhancers::check_stop_times_order(&mut c);
        c.comment_deduplication();
        c.clean_comments();
        c.sanitize()?;

        let forward_vj_to_sp = c
            .vehicle_journeys
            .iter()
            .map(|(idx, vj)| {
                let sps = vj.stop_times.iter().map(|st| st.stop_point_idx).collect();
                (idx, sps)
            })
            .collect();

        let forward_tr_to_sp = c
            .transfers
            .iter()
            .map(|(idx, tr)| {
                let mut stop_points = IdxSet::default();
                stop_points.insert(c.stop_points.get_idx(&tr.from_stop_id).ok_or_else(|| {
                    anyhow!("Invalid id: transfer.from_stop_id={:?}", tr.from_stop_id)
                })?);
                stop_points.insert(c.stop_points.get_idx(&tr.to_stop_id).ok_or_else(|| {
                    anyhow!("Invalid id: transfer.to_stop_id={:?}", tr.to_stop_id)
                })?);
                Ok((idx, stop_points))
            })
            .collect::<Result<BTreeMap<_, _>, Error>>()?;
        let vehicle_journeys_to_stop_points = ManyToMany::from_forward(forward_vj_to_sp);
        let routes_to_vehicle_journeys =
            OneToMany::new(&c.routes, &c.vehicle_journeys, "routes_to_vehicle_journeys")?;
        let physical_modes_to_vehicle_journeys = OneToMany::new(
            &c.physical_modes,
            &c.vehicle_journeys,
            "physical_modes_to_vehicle_journeys",
        )?;
        let datasets_to_vehicle_journeys = OneToMany::new(
            &c.datasets,
            &c.vehicle_journeys,
            "datasets_to_vehicle_journeys",
        )?;
        let routes_to_stop_points = ManyToMany::from_relations_chain(
            &routes_to_vehicle_journeys,
            &vehicle_journeys_to_stop_points,
        );
        let physical_modes_to_stop_points = ManyToMany::from_relations_chain(
            &physical_modes_to_vehicle_journeys,
            &vehicle_journeys_to_stop_points,
        );
        let physical_modes_to_routes = ManyToMany::from_relations_sink(
            &physical_modes_to_vehicle_journeys,
            &routes_to_vehicle_journeys,
        );
        let datasets_to_stop_points = ManyToMany::from_relations_chain(
            &datasets_to_vehicle_journeys,
            &vehicle_journeys_to_stop_points,
        );
        let datasets_to_routes = ManyToMany::from_relations_sink(
            &datasets_to_vehicle_journeys,
            &routes_to_vehicle_journeys,
        );
        let datasets_to_physical_modes = ManyToMany::from_relations_sink(
            &datasets_to_vehicle_journeys,
            &physical_modes_to_vehicle_journeys,
        );
        let transfers_to_stop_points = ManyToMany::from_forward(forward_tr_to_sp);
        let networks_to_lines = OneToMany::new(&c.networks, &c.lines, "networks_to_lines")?;
        let commercial_modes_to_lines =
            OneToMany::new(&c.commercial_modes, &c.lines, "commercial_modes_to_lines")?;
        let lines_to_routes = OneToMany::new(&c.lines, &c.routes, "lines_to_routes")?;
        let stop_areas_to_stop_points =
            OneToMany::new(&c.stop_areas, &c.stop_points, "stop_areas_to_stop_points")?;
        let contributors_to_datasets =
            OneToMany::new(&c.contributors, &c.datasets, "contributors_to_datasets")?;
        let companies_to_vehicle_journeys = OneToMany::new(
            &c.companies,
            &c.vehicle_journeys,
            "companies_to_vehicle_journeys",
        )?;
        let calendars_to_vehicle_journeys = OneToMany::new(
            &c.calendars,
            &c.vehicle_journeys,
            "calendars_to_vehicle_journeys",
        )?;

        c.update_stop_area_coords();
        enhancers::fill_co2(&mut c);
        c.enhance_trip_headsign();
        c.enhance_route_names(&routes_to_vehicle_journeys);
        c.enhance_route_directions();
        c.check_geometries_coherence();
        enhancers::adjust_lines_names(&mut c, &lines_to_routes);
        c.enhance_line_opening_time();
        c.pickup_drop_off_harmonisation();
        enhancers::enhance_pickup_dropoff(&mut c);
        enhancers::memory_shrink(&mut c);

        Ok(Model {
            routes_to_stop_points,
            physical_modes_to_stop_points,
            physical_modes_to_routes,
            datasets_to_stop_points,
            datasets_to_routes,
            datasets_to_physical_modes,
            transfers_to_stop_points,
            datasets_to_vehicle_journeys,
            routes_to_vehicle_journeys,
            vehicle_journeys_to_stop_points,
            physical_modes_to_vehicle_journeys,
            networks_to_lines,
            commercial_modes_to_lines,
            lines_to_routes,
            stop_areas_to_stop_points,
            contributors_to_datasets,
            companies_to_vehicle_journeys,
            calendars_to_vehicle_journeys,
            collections: c,
        })
    }

    /// Consumes collections,
    ///
    /// # Examples
    ///
    /// ```
    /// # use transit_model::model::*;
    /// # use std::collections::HashMap;
    /// # fn run() -> transit_model::Result<()> {
    /// let model: Model = Model::new(Collections::default())?;
    /// let mut collections = model.into_collections();
    ///  collections
    ///    .feed_infos
    ///    .insert("foo".to_string(), "bar".to_string());
    /// let feeds: Vec<(_, _)> = collections.feed_infos.into_iter().collect();
    /// assert_eq!(
    ///    vec![("foo".to_string(), "bar".to_string())],
    ///    feeds
    /// );
    /// # Ok(())
    /// # }
    /// # run().unwrap()
    /// ```
    pub fn into_collections(self) -> Collections {
        self.collections
    }
}

impl ::serde::Serialize for Model {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        self.collections.serialize(serializer)
    }
}
impl<'de> ::serde::Deserialize<'de> for Model {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        use serde::de::Error;
        ::serde::Deserialize::deserialize(deserializer)
            .and_then(|o| Model::new(o).map_err(D::Error::custom))
    }
}
impl ops::Deref for Model {
    type Target = Collections;
    fn deref(&self) -> &Self::Target {
        &self.collections
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod enhance_trip_headsign {
        use super::*;
        use pretty_assertions::assert_eq;

        fn collections(trip_headsign: Option<String>) -> Collections {
            let mut collections = Collections::default();
            collections
                .stop_points
                .push(StopPoint {
                    id: String::from("stop_point_id"),
                    name: String::from("Stop Name"),
                    ..Default::default()
                })
                .unwrap();
            let stop_time = StopTime {
                stop_point_idx: collections.stop_points.get_idx("stop_point_id").unwrap(),
                sequence: 0,
                arrival_time: Time::new(0, 0, 0),
                departure_time: Time::new(0, 0, 0),
                boarding_duration: 0,
                alighting_duration: 0,
                pickup_type: 0,
                drop_off_type: 0,
                local_zone_id: Some(0),
                precision: None,
            };
            collections
                .vehicle_journeys
                .push(VehicleJourney {
                    id: String::from("vehicle_journey_id_1"),
                    stop_times: vec![stop_time],
                    headsign: trip_headsign,
                    ..Default::default()
                })
                .unwrap();
            collections
                .vehicle_journeys
                .push(VehicleJourney {
                    id: String::from("vehicle_journey_id_2"),
                    headsign: Some(String::from("Headsign")),
                    ..Default::default()
                })
                .unwrap();
            collections
        }

        #[test]
        fn enhance() {
            let mut collections = collections(None);
            collections.enhance_trip_headsign();
            let vehicle_journey = collections
                .vehicle_journeys
                .get("vehicle_journey_id_1")
                .unwrap();
            assert_eq!("Stop Name", vehicle_journey.headsign.as_ref().unwrap());
            let vehicle_journey = collections
                .vehicle_journeys
                .get("vehicle_journey_id_2")
                .unwrap();
            assert_eq!("Headsign", vehicle_journey.headsign.as_ref().unwrap());
        }

        #[test]
        fn enhance_when_string_empty() {
            let mut collections = collections(Some(String::new()));
            collections.enhance_trip_headsign();
            let vehicle_journey = collections
                .vehicle_journeys
                .get("vehicle_journey_id_1")
                .unwrap();
            assert_eq!("Stop Name", vehicle_journey.headsign.as_ref().unwrap());
            let vehicle_journey = collections
                .vehicle_journeys
                .get("vehicle_journey_id_2")
                .unwrap();
            assert_eq!("Headsign", vehicle_journey.headsign.as_ref().unwrap());
        }
    }

    mod calendar_deduplication {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn enhance() {
            let mut collections = Collections::default();

            let mut service_1 = Calendar::new(String::from("service_1"));
            service_1
                .dates
                .insert(NaiveDate::from_ymd_opt(2019, 10, 1).unwrap());
            service_1
                .dates
                .insert(NaiveDate::from_ymd_opt(2019, 10, 2).unwrap());
            service_1
                .dates
                .insert(NaiveDate::from_ymd_opt(2019, 10, 3).unwrap());
            service_1
                .dates
                .insert(NaiveDate::from_ymd_opt(2019, 10, 10).unwrap());
            collections.calendars.push(service_1).unwrap();

            let mut service_2 = Calendar::new(String::from("service_2"));
            service_2
                .dates
                .insert(NaiveDate::from_ymd_opt(2019, 10, 1).unwrap());
            service_2
                .dates
                .insert(NaiveDate::from_ymd_opt(2019, 10, 2).unwrap());
            service_2
                .dates
                .insert(NaiveDate::from_ymd_opt(2019, 10, 3).unwrap());
            service_2
                .dates
                .insert(NaiveDate::from_ymd_opt(2019, 10, 10).unwrap());
            collections.calendars.push(service_2).unwrap();

            let mut service_3 = Calendar::new(String::from("service_3"));
            service_3
                .dates
                .insert(NaiveDate::from_ymd_opt(2019, 10, 1).unwrap());
            service_3
                .dates
                .insert(NaiveDate::from_ymd_opt(2019, 10, 3).unwrap());
            service_3
                .dates
                .insert(NaiveDate::from_ymd_opt(2019, 10, 10).unwrap());
            collections.calendars.push(service_3).unwrap();

            collections
                .vehicle_journeys
                .push(VehicleJourney {
                    id: String::from("vehicle_journey_id_1"),
                    service_id: String::from("service_1"),
                    ..Default::default()
                })
                .unwrap();

            collections
                .vehicle_journeys
                .push(VehicleJourney {
                    id: String::from("vehicle_journey_id_2"),
                    service_id: String::from("service_2"),
                    ..Default::default()
                })
                .unwrap();

            collections
                .vehicle_journeys
                .push(VehicleJourney {
                    id: String::from("vehicle_journey_id_3"),
                    service_id: String::from("service_3"),
                    ..Default::default()
                })
                .unwrap();

            collections.calendar_deduplication();

            let vehicle_journey = collections
                .vehicle_journeys
                .get("vehicle_journey_id_2")
                .unwrap();
            assert_eq!("service_1", vehicle_journey.service_id);

            let vehicle_journey = collections
                .vehicle_journeys
                .get("vehicle_journey_id_3")
                .unwrap();
            assert_eq!("service_3", vehicle_journey.service_id);

            let calendar = collections.calendars.get("service_2");
            assert_eq!(None, calendar);
        }
    }

    mod clean_comments {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn remove_empty_comment() {
            let mut collections = Collections::default();
            let comment = Comment {
                id: "comment_id".to_string(),
                name: "Some useless comment.".to_string(),
                ..Default::default()
            };
            let empty_comment = Comment {
                id: "empty_comment_id".to_string(),
                name: String::new(),
                ..Default::default()
            };
            let mut comment_links = CommentLinksT::default();
            comment_links.insert(comment.id.clone());
            comment_links.insert(empty_comment.id.clone());
            collections.comments.push(comment).unwrap();
            collections.comments.push(empty_comment).unwrap();
            collections
                .lines
                .push(Line {
                    id: "line_id".to_string(),
                    comment_links: comment_links.clone(),
                    ..Default::default()
                })
                .unwrap();
            collections
                .routes
                .push(Route {
                    id: "route_id".to_string(),
                    comment_links: comment_links.clone(),
                    ..Default::default()
                })
                .unwrap();
            collections
                .vehicle_journeys
                .push(VehicleJourney {
                    id: "vehicle_journey_id".to_string(),
                    comment_links: comment_links.clone(),
                    ..Default::default()
                })
                .unwrap();
            collections
                .stop_points
                .push(StopPoint {
                    id: "stop_point_id".to_string(),
                    comment_links: comment_links.clone(),
                    ..Default::default()
                })
                .unwrap();
            collections
                .stop_areas
                .push(StopArea {
                    id: "stop_area_id".to_string(),
                    comment_links: comment_links.clone(),
                    ..Default::default()
                })
                .unwrap();
            collections
                .stop_locations
                .push(StopLocation {
                    id: "stop_location_id".to_string(),
                    comment_links,
                    ..Default::default()
                })
                .unwrap();
            collections.clean_comments();
            let line = collections.lines.get("line_id").unwrap();
            assert_eq!(1, line.comment_links.len());
            assert!(line.comment_links.get("comment_id").is_some());
            let route = collections.routes.get("route_id").unwrap();
            assert_eq!(1, route.comment_links.len());
            assert!(route.comment_links.get("comment_id").is_some());
            let vehicle_journey = collections
                .vehicle_journeys
                .get("vehicle_journey_id")
                .unwrap();
            assert_eq!(1, vehicle_journey.comment_links.len());
            assert!(vehicle_journey.comment_links.get("comment_id").is_some());
            let stop_point = collections.stop_points.get("stop_point_id").unwrap();
            assert_eq!(1, stop_point.comment_links.len());
            assert!(stop_point.comment_links.get("comment_id").is_some());
            let stop_area = collections.stop_areas.get("stop_area_id").unwrap();
            assert_eq!(1, stop_area.comment_links.len());
            assert!(stop_area.comment_links.get("comment_id").is_some());
            let stop_location = collections.stop_locations.get("stop_location_id").unwrap();
            assert_eq!(1, stop_location.comment_links.len());
            assert!(stop_location.comment_links.get("comment_id").is_some());
        }
    }

    mod enhance_route_directions {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn generate_route_direction() {
            let mut collections = Collections::default();
            collections
                .routes
                .push(Route {
                    id: String::from("route_id1"),
                    name: String::new(),
                    ..Default::default()
                })
                .unwrap();
            collections
                .routes
                .push(Route {
                    id: String::from("route_id2"),
                    name: String::new(),
                    direction_type: Some("clockwise".to_string()),
                    ..Default::default()
                })
                .unwrap();
            collections.enhance_route_directions();
            let route1 = collections.routes.get("route_id1").unwrap();
            assert_eq!("forward", route1.direction_type.as_ref().unwrap());
            let route2 = collections.routes.get("route_id2").unwrap();
            assert_eq!("clockwise", route2.direction_type.as_ref().unwrap());
        }
    }

    mod enhance_route_names {
        use super::*;
        use pretty_assertions::assert_eq;

        fn stop_areas() -> CollectionWithId<StopArea> {
            CollectionWithId::new(
                (1..9)
                    .map(|index| StopArea {
                        id: format!("stop_area:{}", index),
                        name: format!("Stop Area {}", index),
                        ..Default::default()
                    })
                    .collect(),
            )
            .unwrap()
        }

        fn stop_points() -> CollectionWithId<StopPoint> {
            CollectionWithId::new(
                (1..9)
                    .map(|index| StopPoint {
                        id: format!("stop_point:{}", index),
                        stop_area_id: format!("stop_area:{}", index),
                        ..Default::default()
                    })
                    .collect(),
            )
            .unwrap()
        }

        fn collections() -> Collections {
            let mut collections = Collections {
                stop_areas: stop_areas(),
                stop_points: stop_points(),
                ..Default::default()
            };
            collections
                .routes
                .push(Route {
                    id: String::from("route_id"),
                    name: String::new(),
                    ..Default::default()
                })
                .unwrap();
            collections
        }

        fn create_vehicle_journey_with(
            trip_id: &str,
            stop_point_ids: Vec<&str>,
            collections: &Collections,
        ) -> VehicleJourney {
            let stop_time_at = |stop_point_id: &str| StopTime {
                stop_point_idx: collections.stop_points.get_idx(stop_point_id).unwrap(),
                sequence: 0,
                arrival_time: Time::new(0, 0, 0),
                departure_time: Time::new(0, 0, 0),
                boarding_duration: 0,
                alighting_duration: 0,
                pickup_type: 0,
                drop_off_type: 0,
                local_zone_id: None,
                precision: None,
            };
            let stop_times: Vec<_> = stop_point_ids.into_iter().map(stop_time_at).collect();
            VehicleJourney {
                id: String::from(trip_id),
                codes: KeysValues::default(),
                object_properties: PropertiesMap::default(),
                comment_links: CommentLinksT::default(),
                route_id: String::from("route_id"),
                physical_mode_id: String::new(),
                dataset_id: String::new(),
                service_id: String::new(),
                headsign: None,
                short_name: None,
                block_id: None,
                company_id: String::new(),
                trip_property_id: None,
                geometry_id: None,
                stop_times,
                journey_pattern_id: None,
            }
        }

        #[test]
        fn generate_route_name() {
            let mut collections = collections();
            collections
                .vehicle_journeys
                .push(create_vehicle_journey_with(
                    "trip:1",
                    vec!["stop_point:1", "stop_point:2"],
                    &collections,
                ))
                .unwrap();
            let routes_to_vehicle_journeys = OneToMany::new(
                &collections.routes,
                &collections.vehicle_journeys,
                "routes_to_vehicle_journeys",
            )
            .unwrap();
            collections.enhance_route_names(&routes_to_vehicle_journeys);
            let route = collections.routes.get("route_id").unwrap();
            assert_eq!("Stop Area 1 - Stop Area 2", route.name);
            assert_eq!("stop_area:2", route.destination_id.as_ref().unwrap());
        }

        #[test]
        fn do_not_generate_route_name_when_stops_names_are_empty() {
            let mut collections = collections();
            collections
                .vehicle_journeys
                .push(create_vehicle_journey_with(
                    "trip:1",
                    vec!["stop_point:1", "stop_point:2"],
                    &collections,
                ))
                .unwrap();
            let routes_to_vehicle_journeys = OneToMany::new(
                &collections.routes,
                &collections.vehicle_journeys,
                "routes_to_vehicle_journeys",
            )
            .unwrap();
            collections.stop_areas.get_mut("stop_area:1").unwrap().name = String::new();
            collections.enhance_route_names(&routes_to_vehicle_journeys);
            let route = collections.routes.get("route_id").unwrap();
            assert_eq!("", route.name);
            assert_eq!("stop_area:2", route.destination_id.as_ref().unwrap());
        }

        #[test]
        fn generate_destination_id() {
            let mut collections = collections();
            collections
                .vehicle_journeys
                .push(create_vehicle_journey_with(
                    "trip:1",
                    vec!["stop_point:1", "stop_point:2"],
                    &collections,
                ))
                .unwrap();
            let route_idx = collections.routes.get_idx("route_id").unwrap();
            collections.routes.index_mut(route_idx).name = String::from("Route to Mordor");
            collections.routes.index_mut(route_idx).destination_id = None;
            let routes_to_vehicle_journeys = OneToMany::new(
                &collections.routes,
                &collections.vehicle_journeys,
                "routes_to_vehicle_journeys",
            )
            .unwrap();
            collections.enhance_route_names(&routes_to_vehicle_journeys);
            let route = collections.routes.get("route_id").unwrap();
            // Check route name hasn't been changed
            assert_eq!("Route to Mordor", route.name);
            assert_eq!("stop_area:2", route.destination_id.as_ref().unwrap());
        }

        #[test]
        fn most_frequent_origin_destination() {
            let mut collections = collections();
            collections
                .vehicle_journeys
                .push(create_vehicle_journey_with(
                    "trip:1",
                    vec!["stop_point:1", "stop_point:2"],
                    &collections,
                ))
                .unwrap();
            collections
                .vehicle_journeys
                .push(create_vehicle_journey_with(
                    "trip:2",
                    vec!["stop_point:1", "stop_point:3"],
                    &collections,
                ))
                .unwrap();
            collections
                .vehicle_journeys
                .push(create_vehicle_journey_with(
                    "trip:3",
                    vec!["stop_point:2", "stop_point:3"],
                    &collections,
                ))
                .unwrap();
            let routes_to_vehicle_journeys = OneToMany::new(
                &collections.routes,
                &collections.vehicle_journeys,
                "routes_to_vehicle_journeys",
            )
            .unwrap();
            collections.enhance_route_names(&routes_to_vehicle_journeys);
            let route = collections.routes.get("route_id").unwrap();
            assert_eq!("Stop Area 1 - Stop Area 3", route.name);
            assert_eq!("stop_area:3", route.destination_id.as_ref().unwrap());
        }

        #[test]
        fn same_frequency_then_biggest_stop_area() {
            let mut collections = collections();
            // Make 'stop_area:1' the biggest stop area by number of stop points
            collections
                .stop_points
                .get_mut("stop_point:2")
                .unwrap()
                .stop_area_id = String::from("stop_area:1");
            collections
                .vehicle_journeys
                .push(create_vehicle_journey_with(
                    "trip:1",
                    vec!["stop_point:1", "stop_point:3"],
                    &collections,
                ))
                .unwrap();
            collections
                .vehicle_journeys
                .push(create_vehicle_journey_with(
                    "trip:2",
                    vec!["stop_point:3", "stop_point:2"],
                    &collections,
                ))
                .unwrap();
            let routes_to_vehicle_journeys = OneToMany::new(
                &collections.routes,
                &collections.vehicle_journeys,
                "routes_to_vehicle_journeys",
            )
            .unwrap();
            collections.enhance_route_names(&routes_to_vehicle_journeys);
            let route = collections.routes.get("route_id").unwrap();
            assert_eq!("Stop Area 1 - Stop Area 1", route.name);
            assert_eq!("stop_area:1", route.destination_id.as_ref().unwrap());
        }

        #[test]
        fn same_frequency_same_size_stop_area_then_first_aphabetical_order() {
            let mut collections = collections();
            // Make 'stop_area:1' the biggest stop area by number of stop points
            collections
                .stop_points
                .get_mut("stop_point:2")
                .unwrap()
                .stop_area_id = String::from("stop_area:1");
            // Make 'stop_area:3' as big as 'stop_area:1'
            collections
                .stop_points
                .get_mut("stop_point:4")
                .unwrap()
                .stop_area_id = String::from("stop_area:3");
            collections
                .vehicle_journeys
                .push(create_vehicle_journey_with(
                    "trip:1",
                    vec!["stop_point:1", "stop_point:3"],
                    &collections,
                ))
                .unwrap();
            collections
                .vehicle_journeys
                .push(create_vehicle_journey_with(
                    "trip:2",
                    vec!["stop_point:4", "stop_point:2"],
                    &collections,
                ))
                .unwrap();
            let routes_to_vehicle_journeys = OneToMany::new(
                &collections.routes,
                &collections.vehicle_journeys,
                "routes_to_vehicle_journeys",
            )
            .unwrap();
            collections.enhance_route_names(&routes_to_vehicle_journeys);
            let route = collections.routes.get("route_id").unwrap();
            // 'Stop Area 1' is before 'Stop Area 3' in alphabetical order
            assert_eq!("Stop Area 1 - Stop Area 1", route.name);
            assert_eq!("stop_area:1", route.destination_id.as_ref().unwrap());
        }
    }

    mod check_geometries_coherence {
        use super::*;
        use geo::{Geometry as GeoGeometry, Point as GeoPoint};
        use pretty_assertions::assert_eq;

        #[test]
        fn remove_dead_reference() {
            let mut collections = Collections {
                vehicle_journeys: CollectionWithId::new(vec![VehicleJourney {
                    id: String::from("vehicle_journey_id"),
                    geometry_id: Some(String::from("geometry_id")),
                    ..Default::default()
                }])
                .unwrap(),
                ..Default::default()
            };
            collections.check_geometries_coherence();
            assert_eq!(
                None,
                collections
                    .vehicle_journeys
                    .get("vehicle_journey_id")
                    .unwrap()
                    .geometry_id
            );
        }

        #[test]
        fn preserve_valid_reference() {
            let mut collections = Collections {
                vehicle_journeys: CollectionWithId::new(vec![VehicleJourney {
                    id: String::from("vehicle_journey_id"),
                    geometry_id: Some(String::from("geometry_id")),
                    ..Default::default()
                }])
                .unwrap(),
                geometries: CollectionWithId::new(vec![Geometry {
                    id: String::from("geometry_id"),
                    geometry: GeoGeometry::Point(GeoPoint::new(0.0, 0.0)),
                }])
                .unwrap(),
                ..Default::default()
            };
            collections.check_geometries_coherence();
            assert_eq!(
                Some(String::from("geometry_id")),
                collections
                    .vehicle_journeys
                    .get("vehicle_journey_id")
                    .unwrap()
                    .geometry_id
            );
        }
    }

    mod update_stop_area_coords {
        use super::*;
        use approx::assert_relative_eq;

        fn collections(sp_amount: usize) -> Collections {
            Collections {
                stop_areas: stop_areas(),
                stop_points: stop_points(sp_amount),
                ..Default::default()
            }
        }

        fn stop_areas() -> CollectionWithId<StopArea> {
            CollectionWithId::from(StopArea {
                id: "stop_area:1".into(),
                name: "Stop Area 1".into(),
                coord: Coord::default(),
                ..Default::default()
            })
        }

        fn stop_points(sp_amount: usize) -> CollectionWithId<StopPoint> {
            CollectionWithId::new(
                (1..=sp_amount)
                    .map(|index| StopPoint {
                        id: format!("stop_point:{}", index),
                        stop_area_id: "stop_area:1".into(),
                        coord: Coord {
                            lon: index as f64,
                            lat: index as f64,
                        },
                        ..Default::default()
                    })
                    .collect(),
            )
            .unwrap()
        }
        #[test]
        fn update_coords() {
            let mut collections = collections(3);
            collections.update_stop_area_coords();
            let stop_area = collections.stop_areas.get("stop_area:1").unwrap();
            assert_relative_eq!(stop_area.coord.lon, 2.0);
            assert_relative_eq!(stop_area.coord.lat, 2.0);
        }

        #[test]
        fn update_coords_on_not_referenced_stop_area() {
            let mut collections = collections(0);
            collections.update_stop_area_coords();
            let stop_area = collections.stop_areas.get("stop_area:1").unwrap();
            assert_relative_eq!(stop_area.coord.lon, 0.0);
            assert_relative_eq!(stop_area.coord.lat, 0.0);
        }
    }

    mod pickup_dropoff_harmonisation {
        use pretty_assertions::assert_eq;

        #[test]
        fn update_pickup_drop_off_type() {
            let model = transit_model_builder::ModelBuilder::default()
                .vj("vj1", |vj| {
                    vj.st_mut("SP1", "10:00:00", "10:01:00", |st| {
                        st.pickup_type = 0;
                        st.drop_off_type = 3;
                    })
                    .st_mut("SP2", "11:00:00", "11:01:00", |st| {
                        st.pickup_type = 3;
                        st.drop_off_type = 2;
                    });
                })
                .build();

            let mut collections = model.into_collections();
            collections.pickup_drop_off_harmonisation();
            let vj = collections
                .vehicle_journeys
                .get("vj1")
                .expect("Failed to find vehicle journey vj1");
            let stop_time = &vj.stop_times[0];
            assert_eq!(3, stop_time.pickup_type);
            assert_eq!(3, stop_time.drop_off_type);
            let stop_time = &vj.stop_times[1];
            assert_eq!(3, stop_time.pickup_type);
            assert_eq!(3, stop_time.drop_off_type);
        }

        #[test]
        fn remove_pickup_drop_off_type_3() {
            let model = transit_model_builder::ModelBuilder::default()
                .vj("vj1", |vj| {
                    vj.st_mut("SP1", "10:00:00", "10:01:00", |st| {
                        st.pickup_type = 0;
                        st.drop_off_type = 1;
                    })
                    .st_mut("SP2", "11:00:00", "11:01:00", |st| {
                        st.pickup_type = 3;
                        st.drop_off_type = 2;
                    })
                    .st_mut("SP3", "12:00:00", "12:01:00", |st| {
                        st.pickup_type = 3;
                        st.drop_off_type = 2;
                    })
                    .st_mut("SP4", "13:00:00", "13:01:00", |st| {
                        st.pickup_type = 1;
                        st.drop_off_type = 0;
                    });
                })
                .build();

            let mut collections = model.into_collections();
            collections.remove_route_points();
            let vj = collections
                .vehicle_journeys
                .get("vj1")
                .expect("Failed to find vehicle journey vj1");
            assert_eq!(2, vj.stop_times.len());
        }
    }

    mod check_coord_integrity {
        use crate::objects::Coord;

        #[test]
        fn check_valid_coord() {
            let coord = Coord {
                lon: 2.372987,
                lat: 48.844746,
            };
            assert!(coord.is_valid());
        }
    }

    #[test]
    fn check_unvalid_lon() {
        let coord = Coord {
            lon: -182.,
            lat: 48.844746,
        };
        assert!(!coord.is_valid());
    }

    #[test]
    fn check_unvalid_lat() {
        let coord = Coord {
            lon: 2.372987,
            lat: 91.,
        };
        assert!(!coord.is_valid());
    }
}
