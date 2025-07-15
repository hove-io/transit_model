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

use super::{
    Agency, Attribution, BookingRule, DirectionType, LocationGroupStop, Route, RouteType, Shape,
    Stop, StopLocationType, StopTime, Transfer, TransferType, Trip,
};
use crate::{
    file_handler::FileHandler,
    model::Collections,
    objects::{
        self, Availability, Comment, Company, CompanyRole, Coord, KeysValues, LinksT, ObjectType,
        Pathway, PropertiesMap, StopLocation, StopPoint, StopTimePrecision, StopType, Time,
        TransportType, VehicleJourney,
    },
    parser::{read_collection, read_objects, read_objects_loose},
    serde_utils::de_with_empty_default,
    Result,
};
use anyhow::{anyhow, bail, Error};
use derivative::Derivative;
use geo::{LineString, Point};
use serde::Deserialize;
use skip_error::{skip_error_and_warn, SkipError};
use std::convert::TryFrom;
use std::{
    cmp,
    collections::{BTreeMap, BTreeSet, HashMap},
    hash::{DefaultHasher, Hash, Hasher},
};
use tracing::{info, warn};
use typed_index_collection::{impl_id, Collection, CollectionWithId, Idx};

fn default_agency_id() -> String {
    1.to_string()
}

fn get_agency_id(route: &Route, networks: &CollectionWithId<objects::Network>) -> Result<String> {
    route
        .agency_id
        .clone()
        .ok_or(())
        .or_else(|()| match networks.values().next() {
            Some(n) if networks.len() == 1 => Ok(n.id.clone()),
            Some(_) => bail!("Impossible to get agency id, several networks found"),
            None => bail!("Impossible to get agency id, no network found"),
        })
}

impl From<Agency> for objects::Network {
    fn from(agency: Agency) -> objects::Network {
        let id = agency.id.unwrap_or_else(default_agency_id);
        let mut codes = KeysValues::default();
        codes.insert(("source".to_string(), id.clone()));
        objects::Network {
            id,
            name: agency.name,
            codes,
            timezone: Some(agency.timezone),
            url: Some(agency.url),
            lang: agency.lang,
            phone: agency.phone,
            address: None,
            fare_url: agency.fare_url,
            sort_order: None,
        }
    }
}
impl From<Agency> for objects::Company {
    fn from(agency: Agency) -> objects::Company {
        let id = agency.id.unwrap_or_else(default_agency_id);
        let mut codes = KeysValues::default();
        codes.insert(("source".to_string(), id.clone()));
        objects::Company {
            id,
            name: agency.name,
            address: None,
            url: Some(agency.url),
            mail: agency.email,
            phone: agency.phone,
            codes,
            ..Default::default()
        }
    }
}

impl TryFrom<Stop> for objects::StopArea {
    type Error = Error;
    fn try_from(stop: Stop) -> Result<Self> {
        let mut codes: KeysValues = BTreeSet::new();
        codes.insert(("source".to_string(), stop.id.clone()));
        if let Some(c) = stop.code.as_ref() {
            codes.insert(("gtfs_stop_code".to_string(), c.clone()));
        }
        if stop.name.is_empty() {
            warn!("stop_id: {}: for station stop_name is required", stop.id);
        }
        let coord = Coord::from((stop.lon, stop.lat));
        if coord == Coord::default() {
            warn!("stop_id: {}: for station coordinates are required", stop.id);
        }

        let stop_area = objects::StopArea {
            id: stop.id,
            name: stop.name,
            codes,
            object_properties: PropertiesMap::default(),
            comment_links: objects::LinksT::default(),
            coord,
            timezone: stop.timezone,
            visible: true,
            geometry_id: None,
            level_id: stop.level_id,
            equipment_id: None,
            address_id: None,
        };
        Ok(stop_area)
    }
}

impl TryFrom<Stop> for objects::StopPoint {
    type Error = Error;
    fn try_from(stop: Stop) -> Result<Self> {
        let mut codes: KeysValues = BTreeSet::new();
        codes.insert(("source".to_string(), stop.id.clone()));
        if let Some(c) = stop.code.as_ref() {
            codes.insert(("gtfs_stop_code".to_string(), c.clone()));
        }
        if stop.name.is_empty() {
            warn!("stop_id: {}: for platform name is required", stop.id);
        };

        let coord = Coord::from((stop.lon, stop.lat));
        if coord == Coord::default() {
            warn!(
                "stop_id: {}: for platform coordinates are required",
                stop.id
            );
        }
        let stop_point = objects::StopPoint {
            id: stop.id,
            name: stop.name,
            code: stop.code,
            codes,
            coord,
            stop_area_id: stop
                .parent_station
                .unwrap_or_else(|| String::from("default_id")),
            timezone: stop.timezone,
            visible: true,
            stop_type: StopType::Point,
            platform_code: stop.platform_code,
            level_id: stop.level_id,
            fare_zone_id: stop.fare_zone_id,
            ..Default::default()
        };
        Ok(stop_point)
    }
}

impl TryFrom<Stop> for objects::StopLocation {
    type Error = Error;
    fn try_from(stop: Stop) -> Result<Self> {
        let coord = Coord::from((stop.lon, stop.lat));

        if stop.location_type == StopLocationType::StopEntrance {
            if coord == Coord::default() {
                bail!(
                    "stop_id: {}: for entrances/exits coordinates is required",
                    stop.id
                );
            }
            if stop.parent_station.is_none() {
                bail!(
                    "stop_id: {}: for entrances/exits parent_station is required",
                    stop.id
                );
            }
            if stop.name.is_empty() {
                bail!(
                    "stop_id: {}: for entrances/exits stop_name is required",
                    stop.id
                );
            }
        }
        if stop.location_type == StopLocationType::GenericNode && stop.parent_station.is_none() {
            bail!(
                "stop_id: {}: for generic node parent_station is required",
                stop.id
            );
        }
        if stop.location_type == StopLocationType::BoardingArea && stop.parent_station.is_none() {
            bail!(
                "stop_id: {}: for boarding area parent_station is required",
                stop.id
            );
        }
        let stop_location = StopLocation {
            id: stop.id,
            name: stop.name,
            code: stop.code,
            comment_links: LinksT::default(),
            visible: false, // disable for autocomplete
            coord,
            parent_id: stop.parent_station,
            timezone: stop.timezone,
            geometry_id: None,
            equipment_id: None,
            level_id: stop.level_id,
            stop_type: stop.location_type.into(),
            address_id: None,
        };
        Ok(stop_location)
    }
}

impl TryFrom<Frequency> for objects::Frequency {
    type Error = Error;
    fn try_from(gtfs_frequency: Frequency) -> Result<Self> {
        let ntm_frequency = objects::Frequency {
            vehicle_journey_id: gtfs_frequency.trip_id,
            start_time: gtfs_frequency.start_time,
            end_time: gtfs_frequency.end_time,
            headway_secs: gtfs_frequency.headway_secs,
        };
        Ok(ntm_frequency)
    }
}

impl RouteType {
    fn to_gtfs_value(&self) -> String {
        match *self {
            RouteType::Tramway => "0".to_string(),
            RouteType::Metro => "1".to_string(),
            RouteType::Train => "2".to_string(),
            RouteType::Bus
            | RouteType::UnknownMode
            | RouteType::Coach
            | RouteType::Air
            | RouteType::Taxi => "3".to_string(),
            RouteType::Ferry => "4".to_string(),
            RouteType::CableCar => "5".to_string(),
            RouteType::SuspendedCableCar => "6".to_string(),
            RouteType::Funicular => "7".to_string(),
        }
    }
}

impl ::serde::Serialize for RouteType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        serializer.serialize_str(&self.to_gtfs_value())
    }
}

impl<'de> ::serde::Deserialize<'de> for RouteType {
    fn deserialize<D>(deserializer: D) -> Result<RouteType, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        let i = u16::deserialize(deserializer)?;
        let hundreds = i / 100;
        Ok(match (i, hundreds) {
            (0, _) | (_, 9) => RouteType::Tramway,
            (1, _) | (_, 4) | (_, 5) | (_, 6) => RouteType::Metro,
            (2, _) | (_, 1) | (_, 3) => RouteType::Train,
            (3, _) | (_, 7) | (_, 8) => RouteType::Bus,
            (4, _) | (_, 10) | (_, 12) => RouteType::Ferry,
            (5, _) => RouteType::CableCar,
            (6, _) | (_, 13) => RouteType::SuspendedCableCar,
            (7, _) | (_, 14) => RouteType::Funicular,
            (_, 2) => RouteType::Coach,
            (_, 11) => RouteType::Air,
            (_, 15) => RouteType::Taxi,
            _ => RouteType::UnknownMode,
        })
    }
}

impl_id!(Route);

impl Route {
    fn generate_line_key(
        &self,
        read_as_line: bool,
        idx: Idx<Route>,
    ) -> (Option<String>, String, Option<Idx<Route>>) {
        let name = if !self.short_name.is_empty() {
            self.short_name.clone()
        } else {
            self.long_name.clone()
        };
        let key = if read_as_line { Some(idx) } else { None };
        (self.agency_id.clone(), name, key)
    }

    fn get_id_by_direction(&self, d: DirectionType) -> String {
        let id = self.id.clone();
        match d {
            DirectionType::Forward => id,
            DirectionType::Backward => id + "_R",
        }
    }
}

impl Trip {
    fn to_ntfs_vehicle_journey(
        &self,
        routes: &CollectionWithId<Route>,
        dataset: &objects::Dataset,
        trip_property_id: &Option<String>,
        networks: &CollectionWithId<objects::Network>,
        read_trip_short_name: bool,
    ) -> Result<objects::VehicleJourney> {
        let route = match routes.get(&self.route_id) {
            Some(route) => route,
            None => bail!("Coudn't find route {} for trip {}", self.route_id, self.id),
        };
        let physical_mode = get_physical_mode(&route.route_type);
        let mut codes = KeysValues::default();
        codes.insert(("source".to_string(), self.id.clone()));

        Ok(objects::VehicleJourney {
            id: self.id.clone(),
            codes,
            object_properties: PropertiesMap::default(),
            comment_links: LinksT::default(),
            booking_rule_links: LinksT::default(),
            route_id: route.get_id_by_direction(self.direction),
            physical_mode_id: physical_mode.id,
            dataset_id: dataset.id.clone(),
            service_id: self.service_id.clone(),
            headsign: if read_trip_short_name {
                self.headsign.clone()
            } else {
                self.short_name.clone().or_else(|| self.headsign.clone())
            },
            short_name: if read_trip_short_name {
                self.short_name.clone()
            } else {
                None
            },
            block_id: self.block_id.clone(),
            company_id: get_agency_id(route, networks)?,
            trip_property_id: trip_property_id.clone(),
            geometry_id: self.shape_id.clone(),
            stop_times: Vec::with_capacity(crate::STOP_TIMES_INIT_CAPACITY),
            journey_pattern_id: None,
        })
    }
}

/// Reading rules for mapping vehicle travel paths, sometimes referred to as route alignments.
pub fn manage_shapes<H>(collections: &mut Collections, file_handler: &mut H) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    let file = "shapes.txt";
    let mut shapes = read_objects_loose::<_, Shape>(file_handler, file, false)?;
    shapes.sort_unstable_by_key(|s| s.sequence);
    let mut map: HashMap<String, Vec<Point<f64>>> = HashMap::new();
    for s in &shapes {
        map.entry(s.id.clone())
            .or_default()
            .push((s.lon, s.lat).into())
    }

    collections.geometries = CollectionWithId::new(
        map.iter()
            .filter(|(_, points)| !points.is_empty())
            .map(|(id, points)| {
                let linestring: LineString<f64> = points.to_vec().into();
                objects::Geometry {
                    id: id.to_string(),
                    geometry: linestring.into(),
                }
            })
            .collect(),
    )?;

    Ok(())
}

/// Reading times that a vehicle arrives at and departs from stops for each trip
pub fn manage_stop_times<H>(
    collections: &mut Collections,
    file_handler: &mut H,
    on_demand_transport: bool,
    on_demand_transport_comment: Option<String>,
    location_groups: &LocationGroups,
) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    let file_name = "stop_times.txt";
    let mut headsigns = HashMap::new();
    let mut tmp_vjs = BTreeMap::new();
    let stop_times = read_objects::<_, StopTime>(file_handler, file_name, true)?;

    for stop_time in stop_times {
        if let Some(vj_idx) = collections.vehicle_journeys.get_idx(&stop_time.trip_id) {
            tmp_vjs
                .entry(vj_idx)
                .or_insert_with(Vec::new)
                .push(stop_time);
        } else {
            warn!(
                "Problem reading {:?}: trip_id={:?} not found. Skipping this stop_time",
                file_name, stop_time.trip_id
            )
        }
    }

    'vj_loop: for (vj_idx, mut stop_times) in tmp_vjs {
        stop_times.sort_unstable_by_key(|st| st.stop_sequence);
        stop_times.dedup_by(|st2, st1| {
            let is_same_seq = st2.stop_sequence == st1.stop_sequence;
            if is_same_seq {
                warn!(
                    "remove duplicated stop_sequence '{}' of trip '{}'",
                    st2.stop_sequence, st2.trip_id
                );
            }
            is_same_seq
        });
        let (st_values, has_pickup_drop_off_windows) = interpolate_undefined_stop_times(
            &collections.vehicle_journeys[vj_idx].id,
            &stop_times,
        )?;
        let company_idx = collections
            .companies
            .get_idx(&collections.vehicle_journeys[vj_idx].company_id);

        let mut booking_rule_found = false;
        let mut auto_generated_sequence = 0;

        for (stop_time, st_values) in stop_times.iter().zip(st_values) {
            let stop_point_idxs = if let Some(stop_id) = stop_time.stop_id.as_ref() {
                collections
                    .stop_points
                    .get_idx(stop_id)
                    .map(|idx| vec![idx])
                    .unwrap_or_default()
            } else if let Some(location_group_id) = stop_time.location_group_id.as_deref() {
                location_groups
                    .get(location_group_id)
                    .cloned()
                    .unwrap_or_default()
            } else {
                warn!(
                    "stop_time with trip_id '{}' has no stop_id or location_group_id. Skipping this vehicle journey",
                    collections.vehicle_journeys[vj_idx].id
                );
                continue 'vj_loop;
            };

            if stop_point_idxs.is_empty() {
                println!(
                    "stop_time with trip_id '{}' has no stop points. Skipping this vehicle journey",
                    collections.vehicle_journeys[vj_idx].id
                );
                continue 'vj_loop;
            }

            let precision =
                if on_demand_transport && st_values.precision == StopTimePrecision::Approximate {
                    Some(StopTimePrecision::Estimated)
                } else {
                    Some(st_values.precision)
                };

            if let Some(headsign) = &stop_time.stop_headsign {
                let stop_sequence = if has_pickup_drop_off_windows {
                    auto_generated_sequence
                } else {
                    stop_time.stop_sequence
                };
                headsigns.insert((stop_time.trip_id.clone(), stop_sequence), headsign.clone());
            }

            if let Some(message) = on_demand_transport_comment.as_ref() {
                if stop_time.pickup_type == 2 || stop_time.drop_off_type == 2 {
                    if let Some(company_idx) = company_idx {
                        manage_odt_comment_from_stop_time(
                            collections,
                            message,
                            company_idx,
                            vj_idx,
                            stop_time,
                        );
                    }
                }
            }
            let (pickup_type, drop_off_type) =
                if stop_time.pickup_type == 3 || stop_time.drop_off_type == 3 {
                    (
                        cmp::min(stop_time.pickup_type, 2),
                        cmp::min(stop_time.drop_off_type, 2),
                    )
                } else {
                    (stop_time.pickup_type, stop_time.drop_off_type)
                };

            // Try to find the first booking rule associated with the stop time in
            // pickup_booking_rule_id or drop_off_booking_rule_id
            let booking_rule_id = if !booking_rule_found {
                [
                    &stop_time.pickup_booking_rule_id,
                    &stop_time.drop_off_booking_rule_id,
                ]
                .iter()
                .filter_map(|rule_id| rule_id.as_deref())
                .find_map(|rule_id| {
                    collections
                        .booking_rules
                        .get(rule_id)
                        .map(|rule| rule.id.clone())
                })
            } else {
                None
            };

            let mut vj = collections.vehicle_journeys.index_mut(vj_idx);

            if let Some(rule_id) = booking_rule_id {
                vj.booking_rule_links.insert(rule_id);
                booking_rule_found = true;
            }

            for stop_point_idx in stop_point_idxs {
                vj.stop_times.push(objects::StopTime {
                    stop_point_idx,
                    sequence: if has_pickup_drop_off_windows {
                        auto_generated_sequence
                    } else {
                        stop_time.stop_sequence
                    },
                    arrival_time: st_values.arrival_time,
                    departure_time: st_values.departure_time,
                    start_pickup_drop_off_window: stop_time.start_pickup_drop_off_window,
                    end_pickup_drop_off_window: stop_time.end_pickup_drop_off_window,
                    boarding_duration: 0,
                    alighting_duration: 0,
                    pickup_type,
                    drop_off_type,
                    local_zone_id: stop_time.local_zone_id,
                    precision,
                });
                auto_generated_sequence += 1;
            }
        }
    }

    collections.stop_time_headsigns = headsigns;

    Ok(())
}

fn ventilate_stop_times(
    undefined_stop_times: &[&StopTime],
    before_departure_time: Time,
    after_arrival_time: Time,
) -> Vec<StopTimesValues> {
    let duration = after_arrival_time - before_departure_time;
    let step = duration / (undefined_stop_times.len() + 1) as u32;
    let mut res = vec![];
    for idx in 0..undefined_stop_times.len() {
        let num = idx as u32 + 1u32;
        let time = before_departure_time + objects::Time::new(0, 0, num * step.total_seconds());
        res.push(StopTimesValues {
            departure_time: Some(time),
            arrival_time: Some(time),
            precision: StopTimePrecision::Approximate,
        });
    }
    res
}

#[derive(Debug)]
enum StopTimeType<'a> {
    WithPickupDropOffWindow(Vec<&'a StopTime>),
    NoPickupDropOffWindow(Vec<&'a StopTime>),
}

/// The group_stop_times_by_type function takes a slice of StopTime objects
/// and groups them into contiguous segments based on whether each stop time
/// has "pickup/dropoff window" or not.
/// The function returns a vector of `StopTimeType` values,
/// where each element represents a group of consecutive stop times of the same type.
///
/// eg: [without_window_st1, without_window_st2, with_window_st1, with_window_st2, without_window_st3]
/// will be grouped into:
/// [
///     StopTimeType::NoPickupDropOffWindow(vec![&without_window_st1, &without_window_st2]),
///     StopTimeType::WithPickupDropOffWindow(vec![&with_window_st1, &with_window_st2]),
///     StopTimeType::NoPickupDropOffWindow(vec![&without_window_st3]),
/// ]
fn group_stop_times_by_type(stop_times: &[StopTime]) -> Vec<StopTimeType> {
    let mut result = Vec::new();
    let mut current_group: Vec<&StopTime> = Vec::new();
    let mut last_with_pickup_dropoff_window: Option<bool> = None;

    fn make_group(with_pickup_dropoff_window: bool, group: Vec<&StopTime>) -> StopTimeType {
        if with_pickup_dropoff_window {
            StopTimeType::WithPickupDropOffWindow(group)
        } else {
            StopTimeType::NoPickupDropOffWindow(group)
        }
    }

    for stop_time in stop_times {
        match last_with_pickup_dropoff_window {
            Some(with_pickup_dropoff_window)
                if with_pickup_dropoff_window == stop_time.has_pickup_drop_off_windows() =>
            {
                current_group.push(stop_time);
            }
            Some(_) | None => {
                if !current_group.is_empty() {
                    result.push(make_group(
                        last_with_pickup_dropoff_window.unwrap(),
                        current_group,
                    ));
                }
                current_group = vec![stop_time];
            }
        }
        last_with_pickup_dropoff_window = Some(stop_time.has_pickup_drop_off_windows());
    }

    if !current_group.is_empty() {
        result.push(make_group(
            last_with_pickup_dropoff_window.unwrap(),
            current_group,
        ));
    }

    result
}

fn process_stop_time<'a>(
    st: &'a StopTime,
    vj_id: &str,
    current_st_values: &mut Vec<StopTimesValues>,
    undefined_stops_bulk: &mut Vec<&'a StopTime>,
) -> Result<()> {
    // if only one in departure/arrival value is defined, we set it to the other value
    let (departure_time, arrival_time) = match (st.departure_time, st.arrival_time) {
        (Some(departure_time), None) => {
            tracing::debug!("for vj '{}', stop time n° {} has no arrival defined, we set it to its departure value", vj_id, st.stop_sequence);
            (departure_time, departure_time)
        }
        (None, Some(arrival_time)) => {
            tracing::debug!("for vj '{}', stop time n° {} has no departure defined, we set it to its arrival value", vj_id, st.stop_sequence);
            (arrival_time, arrival_time)
        }
        (Some(departure_time), Some(arrival_time)) => (departure_time, arrival_time),
        (None, None) => {
            undefined_stops_bulk.push(st);
            return Ok(());
        }
    };

    let st_value = StopTimesValues {
        departure_time: Some(departure_time),
        arrival_time: Some(arrival_time),
        precision: if !st.timepoint {
            StopTimePrecision::Approximate
        } else {
            StopTimePrecision::Exact
        },
    };

    if !undefined_stops_bulk.is_empty() {
        let before_departure: Time = if let Some(before) = current_st_values
            .last()
            .and_then(|s: &StopTimesValues| s.departure_time)
        {
            before
        } else {
            bail!("the first stop time of the vj '{}' has no departure/arrival, the stop_times.txt file is not valid", vj_id);
        };
        let values = ventilate_stop_times(
            undefined_stops_bulk,
            before_departure,
            st_value.arrival_time.unwrap(),
        );
        current_st_values.extend(values);
        undefined_stops_bulk.clear();
    }
    current_st_values.push(st_value);
    Ok(())
}

// Temporary struct used by the interpolation process
#[derive(Debug)]
struct StopTimesValues {
    arrival_time: Option<Time>,
    departure_time: Option<Time>,
    precision: StopTimePrecision,
}

fn interpolate_undefined_stop_times(
    vj_id: &str,
    stop_times: &[StopTime],
) -> Result<(Vec<StopTimesValues>, bool)> {
    let grouped = group_stop_times_by_type(stop_times);

    let mut res = vec![];
    let mut has_pickup_dropoff_window = false;

    for group in grouped {
        let mut current_st_values = vec![];
        let mut undefined_stops_bulk: Vec<&StopTime> = Vec::with_capacity(0);
        match group {
            StopTimeType::NoPickupDropOffWindow(stop_times) => {
                for st in stop_times {
                    process_stop_time(
                        st,
                        vj_id,
                        &mut current_st_values,
                        &mut undefined_stops_bulk,
                    )?;
                }

                if !undefined_stops_bulk.is_empty() {
                    bail!("the last stop time of the vj '{}' has no departure/arrival, the stop_times.txt file is not valid", vj_id);
                }
            }

            StopTimeType::WithPickupDropOffWindow(stop_times) => {
                has_pickup_dropoff_window = true;

                // For stop times with pickup/drop-off windows, just copy the times without ventilation
                for st in stop_times {
                    let st_value = StopTimesValues {
                        departure_time: st.departure_time,
                        arrival_time: st.arrival_time,
                        precision: StopTimePrecision::Estimated,
                    };
                    current_st_values.push(st_value);
                }
            }
        }

        res.extend(current_st_values);
    }

    Ok((res, has_pickup_dropoff_window))
}

///Reading transit agencies with service represented in this dataset.
pub fn read_agency<H>(
    file_handler: &mut H,
) -> Result<(
    CollectionWithId<objects::Network>,
    CollectionWithId<objects::Company>,
)>
where
    for<'a> &'a mut H: FileHandler,
{
    let filename = "agency.txt";
    let gtfs_agencies = read_objects::<_, Agency>(file_handler, filename, true)?;

    if let Some(referent_agency) = gtfs_agencies.first() {
        for agency in gtfs_agencies.iter().skip(1) {
            if referent_agency.timezone != agency.timezone {
                warn!(
                    "different agency timezone: {} ({}) - {} ({})",
                    referent_agency.timezone,
                    referent_agency.id.clone().unwrap_or_default(),
                    agency.timezone,
                    agency.id.clone().unwrap_or_default(),
                );
                break;
            }
        }
    }

    let networks = gtfs_agencies
        .iter()
        .cloned()
        .map(objects::Network::from)
        .collect();
    let networks = CollectionWithId::new(networks)?;
    let companies = gtfs_agencies
        .into_iter()
        .map(objects::Company::from)
        .collect();
    let companies = CollectionWithId::new(companies)?;
    Ok((networks, companies))
}

fn generate_stop_comment(stop: &Stop) -> Option<objects::Comment> {
    stop.desc.as_ref().map(|desc| objects::Comment {
        id: "stop:".to_string() + &stop.id,
        comment_type: objects::CommentType::Information,
        label: None,
        name: desc.to_string(),
        url: None,
    })
}

fn insert_comment<T: typed_index_collection::Id<T> + objects::Links<Comment>>(
    collection: &mut CollectionWithId<T>,
    comments: &mut CollectionWithId<objects::Comment>,
    prefix: &str,
    gtfs_route: &Route,
) {
    let opt_comment = gtfs_route.desc.as_ref().map(|desc| objects::Comment {
        id: format!("{}:{}", prefix, gtfs_route.id),
        comment_type: objects::CommentType::Information,
        label: None,
        name: desc.to_string(),
        url: None,
    });

    if let Some(comment) = opt_comment {
        if let Some(mut object) = collection.get_mut(&gtfs_route.id) {
            object.links_mut().insert(comment.id.to_string());
            comments
                .push(comment)
                .expect("Duplicated comment id that shouldn’t be possible");
        }
    }
}

fn manage_odt_comment_from_stop_time(
    collections: &mut Collections,
    on_demand_transport_comment: &str,
    company_idx: Idx<objects::Company>,
    vj_idx: Idx<objects::VehicleJourney>,
    stop_time: &StopTime,
) {
    let comment_id = format!("ODT:{}", collections.companies[company_idx].id);
    if !collections.comments.contains_id(&comment_id) {
        let comment = objects::Comment {
            id: comment_id.clone(),
            comment_type: objects::CommentType::OnDemandTransport,
            label: None,
            name: on_demand_transport_comment
                .replace("{agency_name}", &collections.companies[company_idx].name)
                .replace(
                    "{agency_phone}",
                    &collections.companies[company_idx]
                        .phone
                        .clone()
                        .unwrap_or_default(),
                ),
            url: None,
        };
        // Ok to unwrap since we already tested for existence of the identifier
        collections.comments.push(comment).unwrap();
    }
    collections.stop_time_comments.insert(
        (
            collections.vehicle_journeys[vj_idx].id.to_string(),
            stop_time.stop_sequence,
        ),
        comment_id,
    );
    let stop_time_id = format!("{}-{}", stop_time.trip_id, stop_time.stop_sequence);
    collections.stop_time_ids.insert(
        (
            collections.vehicle_journeys[vj_idx].id.to_string(),
            stop_time.stop_sequence,
        ),
        stop_time_id,
    );
}

/// To associate a list of equipment with a stop
#[derive(Default)]
pub struct EquipmentList {
    equipments: HashMap<objects::Equipment, String>,
}

impl EquipmentList {
    /// Convert EquipmentList to a list of transit model equipments
    pub fn into_equipments(self) -> Vec<objects::Equipment> {
        let mut eqs: Vec<_> = self
            .equipments
            .into_iter()
            .map(|(mut eq, id)| {
                eq.id = id;
                eq
            })
            .collect();

        eqs.sort_by(|l, r| l.id.cmp(&r.id));
        eqs
    }
    /// Insert transit model equipment into EquipmentList
    pub fn push(&mut self, equipment: objects::Equipment) -> String {
        let equipment_id = self.equipments.len().to_string();
        let id = self.equipments.entry(equipment).or_insert(equipment_id);
        id.clone()
    }
}

fn get_equipment_id_and_populate_equipments(
    equipments: &mut EquipmentList,
    stop: &Stop,
) -> Option<String> {
    match stop.wheelchair_boarding {
        Availability::Available | Availability::NotAvailable => {
            Some(equipments.push(objects::Equipment {
                id: "".to_string(),
                wheelchair_boarding: stop.wheelchair_boarding,
                sheltered: Availability::InformationNotAvailable,
                elevator: Availability::InformationNotAvailable,
                escalator: Availability::InformationNotAvailable,
                bike_accepted: Availability::InformationNotAvailable,
                bike_depot: Availability::InformationNotAvailable,
                visual_announcement: Availability::InformationNotAvailable,
                audible_announcement: Availability::InformationNotAvailable,
                appropriate_escort: Availability::InformationNotAvailable,
                appropriate_signage: Availability::InformationNotAvailable,
            }))
        }
        _ => None,
    }
}

/// Reading stops where vehicles pick up or drop off riders. Also defines stations and station entrances.
pub fn read_stops<H>(
    file_handler: &mut H,
    comments: &mut CollectionWithId<objects::Comment>,
    equipments: &mut EquipmentList,
) -> Result<(
    CollectionWithId<objects::StopArea>,
    CollectionWithId<objects::StopPoint>,
    CollectionWithId<objects::StopLocation>,
)>
where
    for<'a> &'a mut H: FileHandler,
{
    let file = "stops.txt";
    info!(file_name = %file, "Reading");
    let gtfs_stops = read_objects::<_, Stop>(file_handler, file, true)?;
    let mut stop_areas = vec![];
    let mut stop_points = vec![];
    let mut stop_locations = vec![];
    for stop in gtfs_stops {
        let mut comment_links = LinksT::default();
        if let Some(comment) = generate_stop_comment(&stop) {
            comment_links.insert(comment.id.to_string());
            comments
                .push(comment)
                .expect("Duplicated comment id that shouldn’t be possible");
        }
        let equipment_id = get_equipment_id_and_populate_equipments(equipments, &stop);
        match stop.location_type {
            StopLocationType::StopPoint => {
                let mut stop_point =
                    skip_error_and_warn!(objects::StopPoint::try_from(stop.clone()));
                if stop.parent_station.is_none() {
                    let stop_area = objects::StopArea::from(stop_point.clone());
                    stop_point.stop_area_id.clone_from(&stop_area.id);
                    stop_areas.push(stop_area);
                };
                stop_point.comment_links = comment_links;
                stop_point.equipment_id = equipment_id;
                stop_points.push(stop_point);
            }
            StopLocationType::StopArea => {
                let mut stop_area = skip_error_and_warn!(objects::StopArea::try_from(stop));
                stop_area.comment_links = comment_links;
                stop_area.equipment_id = equipment_id;
                stop_areas.push(stop_area);
            }
            _ => {
                let mut stop_location = skip_error_and_warn!(objects::StopLocation::try_from(stop));
                stop_location.comment_links = comment_links;
                stop_location.equipment_id = equipment_id;
                stop_locations.push(stop_location);
            }
        }
    }
    let stoppoints = CollectionWithId::new(stop_points)?;
    let stopareas = CollectionWithId::new(stop_areas)?;
    let stoplocations = CollectionWithId::new(stop_locations)?;
    Ok((stopareas, stoppoints, stoplocations))
}

/// Reading pathways linking together locations within stations.
pub fn manage_pathways<H>(collections: &mut Collections, file_handler: &mut H) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    let file = "pathways.txt";

    let gtfs_pathways = read_objects_loose::<_, Pathway>(file_handler, file, false)?;
    let mut pathways = vec![];
    for mut pathway in gtfs_pathways {
        pathway.from_stop_type = skip_error_and_warn!(collections
            .stop_points
            .get(&pathway.from_stop_id)
            .map(|st| st.stop_type.clone())
            .or_else(|| collections
                .stop_locations
                .get(&pathway.from_stop_id)
                .map(|sl| sl.stop_type.clone()))
            .ok_or_else(|| {
                anyhow!(
                    "Problem reading {:?}: from_stop_id={:?} not found",
                    file,
                    pathway.from_stop_id
                )
            }));

        pathway.to_stop_type = skip_error_and_warn!(collections
            .stop_points
            .get(&pathway.to_stop_id)
            .map(|st| st.stop_type.clone())
            .or_else(|| collections
                .stop_locations
                .get(&pathway.to_stop_id)
                .map(|sl| sl.stop_type.clone()))
            .ok_or_else(|| {
                anyhow!(
                    "Problem reading {:?}: to_stop_id={:?} not found",
                    file,
                    pathway.to_stop_id
                )
            }));
        pathways.push(pathway);
    }
    collections.pathways = CollectionWithId::new(pathways)?;
    Ok(())
}

/// Reading rules for making connections at transfer points between routes.
pub fn read_transfers<H>(
    file_handler: &mut H,
    stop_points: &CollectionWithId<objects::StopPoint>,
    stop_areas: &CollectionWithId<objects::StopArea>,
) -> Result<Collection<objects::Transfer>>
where
    for<'a> &'a mut H: FileHandler,
{
    let file = "transfers.txt";
    let gtfs_transfers = read_objects_loose::<_, Transfer>(file_handler, file, false)?;

    let mut transfers = vec![];
    for transfer in gtfs_transfers {
        let expand_stop_area = |stop_id: &str| -> Result<Vec<&StopPoint>> {
            if stop_areas.get(stop_id).is_some() {
                let list_stop_points = stop_points
                    .values()
                    .filter(|stop_point| stop_point.stop_area_id == stop_id)
                    .collect();
                Ok(list_stop_points)
            } else {
                stop_points
                    .get(stop_id)
                    .ok_or_else(|| {
                        anyhow!(
                            "Problem reading {:?}: stop_id={:?} not found",
                            file,
                            stop_id
                        )
                    })
                    .map(|stop_point| vec![stop_point])
            }
        };
        let from_stop_points =
            skip_error_and_warn!(expand_stop_area(transfer.from_stop_id.as_str()));
        let to_stop_points = skip_error_and_warn!(expand_stop_area(transfer.to_stop_id.as_str()));
        for from_stop_point in &from_stop_points {
            let approx = from_stop_point.coord.approx();
            for to_stop_point in &to_stop_points {
                let (min_transfer_time, real_min_transfer_time) = match transfer.transfer_type {
                    TransferType::Recommended => {
                        let sq_distance = approx.sq_distance_to(&to_stop_point.coord);
                        let transfer_time = (sq_distance.sqrt() / 0.785) as u32;

                        (Some(transfer_time), Some(transfer_time + 2 * 60))
                    }
                    TransferType::Timed => (Some(0), Some(0)),
                    TransferType::WithTransferTime => {
                        if transfer.min_transfer_time.is_none() {
                            warn!(
                            "The min_transfer_time between from_stop_id {} and to_stop_id {} is empty",
                            from_stop_point.id, to_stop_point.id
                        );
                        }
                        (transfer.min_transfer_time, transfer.min_transfer_time)
                    }
                    TransferType::NotPossible => (Some(86400), Some(86400)),
                };

                transfers.push(objects::Transfer {
                    from_stop_id: from_stop_point.id.clone(),
                    to_stop_id: to_stop_point.id.clone(),
                    min_transfer_time,
                    real_min_transfer_time,
                    equipment_id: None,
                });
            }
        }
    }

    Ok(Collection::new(transfers))
}

fn get_commercial_mode(route_type: &RouteType) -> objects::CommercialMode {
    objects::CommercialMode {
        id: route_type.to_string(),
        name: match route_type {
            RouteType::CableCar => "Cable car".to_string(),
            RouteType::SuspendedCableCar => "Suspended cable car".to_string(),
            RouteType::UnknownMode => "Unknown mode".to_string(),
            RouteType::Air => "Airplane".to_string(),
            _ => route_type.to_string(),
        },
    }
}

fn get_physical_mode(route_type: &RouteType) -> objects::PhysicalMode {
    let repres = match route_type {
        RouteType::UnknownMode => "Bus".into(),
        RouteType::CableCar => "Funicular".into(),
        _ => route_type.to_string(),
    };
    objects::PhysicalMode {
        id: repres.clone(),
        name: repres,
        co2_emission: None,
    }
}

fn get_modes_from_gtfs(
    gtfs_routes: &CollectionWithId<Route>,
) -> (Vec<objects::CommercialMode>, Vec<objects::PhysicalMode>) {
    let gtfs_mode_types: BTreeSet<RouteType> =
        gtfs_routes.values().map(|r| r.route_type.clone()).collect();

    let commercial_modes = gtfs_mode_types.iter().map(get_commercial_mode).collect();
    let physical_modes = gtfs_mode_types
        .iter()
        .map(get_physical_mode)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    (commercial_modes, physical_modes)
}

fn get_route_with_smallest_name<'a>(routes: &'a [&Route]) -> &'a Route {
    routes.iter().min_by_key(|r| &r.id).unwrap()
}

type MapLineRoutes<'a> = BTreeMap<(Option<String>, String, Option<Idx<Route>>), Vec<&'a Route>>;

fn map_line_routes<'a>(
    gtfs_routes: &'a CollectionWithId<Route>,
    gtfs_trips: &[Trip],
    read_as_line: bool,
) -> MapLineRoutes<'a> {
    let mut map = BTreeMap::new();
    for (idx, r) in gtfs_routes.iter().filter(|(_, r)| {
        if !gtfs_trips.iter().any(|t| t.route_id == r.id) {
            warn!("Couldn't find trips for route_id {}", r.id);
            return false;
        }
        true
    }) {
        map.entry(r.generate_line_key(read_as_line, idx))
            .or_insert_with(Vec::new)
            .push(r);
    }
    map
}

fn make_lines(
    map_line_routes: &MapLineRoutes<'_>,
    networks: &CollectionWithId<objects::Network>,
) -> Result<Vec<objects::Line>> {
    let mut lines = vec![];

    let line_code = |r: &Route| {
        if r.short_name.is_empty() {
            None
        } else {
            Some(r.short_name.to_string())
        }
    };

    for routes in map_line_routes.values() {
        let r = get_route_with_smallest_name(routes);
        let mut codes = KeysValues::default();
        codes.insert(("source".to_string(), r.id.clone()));
        lines.push(objects::Line {
            id: r.id.clone(),
            code: line_code(r),
            codes,
            object_properties: PropertiesMap::default(),
            comment_links: LinksT::default(),
            booking_rule_links: LinksT::default(),
            name: r.long_name.to_string(),
            forward_name: None,
            backward_name: None,
            color: r.color.clone(),
            text_color: r.text_color.clone(),
            sort_order: r.sort_order,
            network_id: get_agency_id(r, networks)?,
            commercial_mode_id: r.route_type.to_string(),
            geometry_id: None,
            opening_time: None,
            closing_time: None,
        });
    }

    Ok(lines)
}

fn make_routes(gtfs_trips: &[Trip], map_line_routes: &MapLineRoutes<'_>) -> Vec<objects::Route> {
    let mut routes = vec![];

    let get_direction_name = |d: DirectionType| match d {
        DirectionType::Forward => "forward".to_string(),
        DirectionType::Backward => "backward".to_string(),
    };

    for rs in map_line_routes.values() {
        let sr = get_route_with_smallest_name(rs);
        for r in rs {
            let mut route_directions: BTreeSet<DirectionType> = BTreeSet::new();
            for t in gtfs_trips.iter().filter(|t| t.route_id == r.id) {
                route_directions.insert(t.direction);
            }

            let has_one_direction = route_directions.len() <= 1;

            for d in route_directions {
                let mut codes = KeysValues::default();
                codes.insert(("source".to_string(), r.id.clone()));
                routes.push(objects::Route {
                    id: r.get_id_by_direction(d),
                    // When only one direction, keep the route name. When
                    // multiple directions are possible, leave the `route_name`
                    // empty, it'll be auto-generated later in
                    // `Collections::enhance_route_names()`.
                    name: if has_one_direction {
                        if !r.long_name.is_empty() {
                            r.long_name.clone()
                        } else {
                            r.short_name.clone()
                        }
                    } else {
                        String::new()
                    },
                    direction_type: Some(get_direction_name(d)),
                    codes,
                    object_properties: PropertiesMap::default(),
                    comment_links: LinksT::default(),
                    line_id: sr.id.clone(),
                    geometry_id: None,
                    destination_id: None,
                });
            }
        }
    }
    routes
}

fn make_ntfs_vehicle_journeys(
    gtfs_trips: &[Trip],
    routes: &CollectionWithId<Route>,
    datasets: &CollectionWithId<objects::Dataset>,
    networks: &CollectionWithId<objects::Network>,
    read_trip_short_name: bool,
) -> (Vec<objects::VehicleJourney>, Vec<objects::TripProperty>) {
    // there always is one dataset from config or a default one
    let (_, dataset) = datasets.iter().next().unwrap();
    let mut vehicle_journeys: Vec<objects::VehicleJourney> = vec![];
    let mut trip_properties: Vec<objects::TripProperty> = vec![];
    let mut map_tps_trips: BTreeMap<(Availability, Availability), Vec<&Trip>> = BTreeMap::new();
    let mut id_incr: u8 = 1;
    let mut property_id: Option<String>;

    for t in gtfs_trips {
        map_tps_trips
            .entry((t.wheelchair_accessible, t.bikes_allowed))
            .or_default()
            .push(t);
    }

    for ((wheelchair, bike), trips) in &map_tps_trips {
        if *wheelchair == Availability::InformationNotAvailable
            && *bike == Availability::InformationNotAvailable
        {
            property_id = None;
        } else {
            property_id = Some(id_incr.to_string());
            trip_properties.push(objects::TripProperty {
                id: id_incr.to_string(),
                wheelchair_accessible: *wheelchair,
                bike_accepted: *bike,
                air_conditioned: Availability::InformationNotAvailable,
                visual_announcement: Availability::InformationNotAvailable,
                audible_announcement: Availability::InformationNotAvailable,
                appropriate_escort: Availability::InformationNotAvailable,
                appropriate_signage: Availability::InformationNotAvailable,
                school_vehicle_type: TransportType::Regular,
            });
            id_incr += 1;
        }
        trips
            .iter()
            .map(|t| {
                t.to_ntfs_vehicle_journey(
                    routes,
                    dataset,
                    &property_id,
                    networks,
                    read_trip_short_name,
                )
            })
            .skip_error_and_warn()
            .for_each(|vj| vehicle_journeys.push(vj));
    }

    (vehicle_journeys, trip_properties)
}

/// Reading transit routes. A route is a group of trips that are displayed to riders as a single service.
pub fn read_routes<H>(
    file_handler: &mut H,
    collections: &mut Collections,
    read_as_line: bool,
    read_trip_short_name: bool,
) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    let file = "routes.txt";
    info!(file_name = %file, "Reading");
    let gtfs_routes_collection = read_collection(file_handler, file)?;
    let (commercial_modes, physical_modes) = get_modes_from_gtfs(&gtfs_routes_collection);
    collections.commercial_modes = CollectionWithId::new(commercial_modes)?;
    collections.physical_modes = CollectionWithId::new(physical_modes)?;

    let gtfs_trips = read_objects(file_handler, "trips.txt", true)?;
    let map_line_routes = map_line_routes(&gtfs_routes_collection, &gtfs_trips, read_as_line);
    let lines = make_lines(&map_line_routes, &collections.networks)?;
    collections.lines = CollectionWithId::new(lines)?;

    let routes = make_routes(&gtfs_trips, &map_line_routes);
    collections.routes = CollectionWithId::new(routes)?;

    gtfs_routes_collection.iter().for_each(|(_id, gtfs_route)| {
        if read_as_line {
            insert_comment(
                &mut collections.lines,
                &mut collections.comments,
                "line",
                gtfs_route,
            );
        } else {
            insert_comment(
                &mut collections.routes,
                &mut collections.comments,
                "route",
                gtfs_route,
            );
        };
    });

    let (vehicle_journeys, trip_properties) = make_ntfs_vehicle_journeys(
        &gtfs_trips,
        &gtfs_routes_collection,
        &collections.datasets,
        &collections.networks,
        read_trip_short_name,
    );
    collections.vehicle_journeys = CollectionWithId::new(vehicle_journeys)?;
    collections.trip_properties = CollectionWithId::new(trip_properties)?;

    Ok(())
}

#[derive(Derivative, Deserialize, Debug, Clone, PartialEq)]
#[derivative(Default)]
enum FrequencyPrecision {
    #[derivative(Default)]
    #[serde(rename = "0")]
    Inexact,
    #[serde(rename = "1")]
    Exact,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
struct Frequency {
    trip_id: String,
    start_time: Time,
    end_time: Time,
    headway_secs: u32,
    #[serde(default, deserialize_with = "de_with_empty_default")]
    exact_times: FrequencyPrecision,
}

///Reading headway (time between trips) for headway-based service or a compressed representation of fixed-schedule service.
pub fn manage_frequencies<H>(collections: &mut Collections, file_handler: &mut H) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    let file = "frequencies.txt";
    let frequencies = read_objects::<_, Frequency>(file_handler, file, false)?
        .iter()
        .map(|gtfs_frequency| objects::Frequency::try_from(gtfs_frequency.clone()))
        .skip_error_and_warn()
        .collect();

    collections.convert_frequencies_to_stoptimes(frequencies)
}

/// attributions applied to the dataset.
#[derive(Eq, Hash, PartialEq)]
pub struct AttributionRule {
    id: String,
    /// Type of public transport object to which the allocation applies
    /// only line or VehicleJourney objects are accepted
    object_type: ObjectType,
    /// Object identifier
    object_id: String,
    /// Name of the organization that the dataset is attributed to.
    organization_name: String,
    /// URL of the organization that the dataset is attributed to.
    attribution_url: Option<String>,
    /// Email of the organization that the dataset is attributed to.
    attribution_email: Option<String>,
    /// Phone number of the organization.
    attribution_phone: Option<String>,
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

impl TryFrom<&Attribution> for AttributionRule {
    type Error = Error;
    fn try_from(attribution: &Attribution) -> Result<Self> {
        let (object_type, object_id) = match (
            attribution.route_id.is_some(),
            attribution.trip_id.is_some(),
        ) {
            (true, false) => (Some(ObjectType::Route), attribution.route_id.clone()),
            (false, true) => (
                Some(ObjectType::VehicleJourney),
                attribution.trip_id.clone(),
            ),
            _ => (None, None),
        };

        if object_type.is_none() {
            let message = if let Some(attribution_id) = &attribution.attribution_id {
                format!("Attribution {attribution_id} must have either route_id or trip_id")
            } else {
                format!("Attribution {attribution:?} must have either route_id or trip_id")
            };
            bail!(message);
        }

        let attribution_rule = AttributionRule {
            id: calculate_hash(&attribution).to_string(),
            object_type: object_type.expect("an error occured"),
            object_id: object_id.expect("an error occured"),
            organization_name: attribution.organization_name.clone(),
            attribution_url: attribution.attribution_url.clone(),
            attribution_email: attribution.attribution_email.clone(),
            attribution_phone: attribution.attribution_phone.clone(),
        };
        Ok(attribution_rule)
    }
}

impl AttributionRule {
    fn get_or_create_company(
        &self,
        companies: &mut CollectionWithId<objects::Company>,
    ) -> Result<String> {
        if !companies.contains_id(&self.id) {
            let company = Company {
                id: self.id.to_string(),
                name: self.organization_name.clone(),
                url: self.attribution_url.clone(),
                mail: self.attribution_email.clone(),
                phone: self.attribution_phone.clone(),
                role: CompanyRole::Operator,
                ..Default::default()
            };
            companies.push(company)?;
        }
        Ok(self.id.to_string())
    }
}

/// Read attributions rules applied to the trip.
pub fn read_attributions<H>(file_handler: &mut H, file_name: &str) -> Result<Vec<AttributionRule>>
where
    for<'a> &'a mut H: FileHandler,
{
    let (reader, path) = file_handler.get_file_if_exists(file_name)?;
    let file_name = path.file_name();
    let basename = file_name.map_or(path.to_string_lossy(), |b| b.to_string_lossy());
    let mut attribution_rules = Vec::new();
    if let Some(reader) = reader {
        info!(file_name = %basename, "Reading");
        let mut rdr = csv::ReaderBuilder::new()
            .flexible(true)
            .trim(csv::Trim::All)
            .from_reader(reader);

        for result in rdr.deserialize() {
            let attribution: Attribution = result?;
            if let Some(true) = attribution.is_operator {
                let attribution_rule =
                    skip_error_and_warn!(AttributionRule::try_from(&attribution));
                attribution_rules.push(attribution_rule);
            }
        }
    };

    Ok(attribution_rules)
}
/// Apply attributions rules on trips.
pub fn apply_attribution_rules(
    collections: &mut Collections,
    attribution_rules: &[AttributionRule],
) -> Result<()> {
    let vjs_idx_by_company: HashMap<&AttributionRule, Vec<Idx<VehicleJourney>>> = attribution_rules
        .iter()
        .filter_map(|attribution_rule| {
            if attribution_rule.object_type == ObjectType::VehicleJourney {
                if let Some(vj_idx) = collections
                    .vehicle_journeys
                    .get_idx(&attribution_rule.object_id)
                {
                    Some((attribution_rule, vec![vj_idx]))
                } else {
                    warn!(
                        "VehicleJourney {} not found for attribution",
                        attribution_rule.object_id
                    );
                    None
                }
            } else if !collections.routes.contains_id(&attribution_rule.object_id) {
                warn!(
                    "Route {} not found for attribution",
                    attribution_rule.object_id
                );
                None
            } else {
                let vjs_idx = collections
                    .vehicle_journeys
                    .iter()
                    .filter(|(_, vj)| vj.route_id == attribution_rule.object_id)
                    .map(|(vj_idx, _)| vj_idx)
                    .collect();
                Some((attribution_rule, vjs_idx))
            }
        })
        .fold(
            HashMap::new(),
            |mut vjs_idx_by_company, (attribution_rule, vjs_idx)| {
                vjs_idx_by_company
                    .entry(attribution_rule)
                    .or_default()
                    .extend(vjs_idx);
                vjs_idx_by_company
            },
        );

    for (attribution_rule, vjs_idx) in vjs_idx_by_company.iter() {
        let company_id = attribution_rule.get_or_create_company(&mut collections.companies)?;
        for vj_idx in vjs_idx {
            collections
                .vehicle_journeys
                .index_mut(*vj_idx)
                .company_id
                .clone_from(&company_id);
        }
    }

    Ok(())
}

type LocationGroups = HashMap<String, Vec<Idx<StopPoint>>>;

/// Reading location groups
pub fn read_location_groups<H>(
    file_handler: &mut H,
    stop_points: &mut CollectionWithId<objects::StopPoint>,
    stop_areas: &CollectionWithId<objects::StopArea>,
) -> Result<LocationGroups>
where
    for<'a> &'a mut H: FileHandler,
{
    let location_group_stops: Vec<LocationGroupStop> =
        read_objects(file_handler, "location_group_stops.txt", false)?;
    let mut location_groups: LocationGroups = HashMap::new();

    for location_group_stop in location_group_stops {
        let group = location_groups
            .entry(location_group_stop.location_group_id.clone())
            .or_default();

        match stop_points.get_idx(&location_group_stop.stop_id) {
            Some(stop_point_idx) => {
                group.push(stop_point_idx);
            }
            None => {
                if let Some(stop_area) = stop_areas.get(&location_group_stop.stop_id) {
                    let stop_point_idxs: Vec<_> = stop_points
                        .iter()
                        .filter_map(|(idx, sp)| (sp.stop_area_id == stop_area.id).then_some(idx))
                        .collect();

                    if stop_point_idxs.is_empty() {
                        warn!(
                            "Problem reading location_group_stops.txt: no stop points for stop area with stop_id={} found, creating a new stop point from stop area",
                            location_group_stop.stop_id
                        );
                        let stop_point = StopPoint::from(stop_area);
                        let stop_point_idx = stop_points.push(stop_point)?;
                        group.push(stop_point_idx);
                    } else {
                        group.extend(stop_point_idxs);
                    }
                } else {
                    warn!(
                        "Problem reading location_group_stops.txt: stop_id={} not found in stop_point or stop_areas",
                        location_group_stop.stop_id
                    );
                }
            }
        }
    }
    Ok(location_groups)
}

pub fn read_booking_rules<H>(file_handler: &mut H) -> Result<CollectionWithId<objects::BookingRule>>
where
    for<'a> &'a mut H: FileHandler,
{
    let booking_rules: Vec<BookingRule> = read_objects(file_handler, "booking_rules.txt", false)?;

    let ntm_booking_rules: Vec<objects::BookingRule> = booking_rules
        .into_iter()
        .map(objects::BookingRule::try_from)
        .collect::<Result<Vec<_>>>()?;

    Ok(CollectionWithId::new(ntm_booking_rules)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        calendars,
        configuration::read_config,
        file_handler::PathFileHandler,
        gtfs::{read::EquipmentList, StopTime as GtfsStopTime},
        model::Collections,
        objects::*,
        objects::{Calendar, Comment, CommentType, Equipment, Geometry, Rgb, StopTime, Transfer},
        parser::read_opt_collection,
        test_utils::*,
        AddPrefix, PrefixConfiguration,
    };
    use geo::line_string;
    use pretty_assertions::assert_eq;
    use typed_index_collection::Id;

    fn extract<'a, T, S: ::std::cmp::Ord>(f: fn(&'a T) -> S, c: &'a Collection<T>) -> Vec<S> {
        let mut extracted_props: Vec<S> = c.values().map(f).collect();
        extracted_props.sort();
        extracted_props
    }

    fn extract_ids<T: Id<T>>(c: &Collection<T>) -> Vec<&str> {
        extract(T::id, c)
    }

    #[test]
    fn load_minimal_agency() {
        let agency_content = "agency_name,agency_url,agency_timezone\n\
                              My agency,http://my-agency_url.com,Europe/London";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "agency.txt", agency_content);
            let (networks, companies) = super::read_agency(&mut handler).unwrap();
            assert_eq!(1, networks.len());
            let agency = networks.iter().next().unwrap().1;
            assert_eq!("1", agency.id);
            assert_eq!(1, companies.len());
        });
    }

    #[test]
    fn load_standard_agency() {
        let agency_content = "agency_id,agency_name,agency_url,agency_timezone\n\
                              id_1,My agency,http://my-agency_url.com,Europe/London";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "agency.txt", agency_content);
            let (networks, companies) = super::read_agency(&mut handler).unwrap();
            assert_eq!(1, networks.len());
            assert_eq!(1, companies.len());
        });
    }

    #[test]
    fn load_complete_agency() {
        let agency_content =
            "agency_id,agency_name,agency_url,agency_timezone,agency_lang,agency_phone,\
             agency_fare_url,agency_email\n\
             id_1,My agency,http://my-agency_url.com,Europe/London,EN,0123456789,\
             http://my-agency_fare_url.com,my-mail@example.com";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "agency.txt", agency_content);
            let (networks, companies) = super::read_agency(&mut handler).unwrap();
            assert_eq!(1, networks.len());
            let network = networks.iter().next().unwrap().1;
            let expected_netword = Network {
                id: "id_1".to_string(),
                name: "My agency".to_string(),
                url: Some("http://my-agency_url.com".to_string()),
                codes: BTreeSet::from([("source".to_string(), "id_1".to_string())]),
                timezone: Some(chrono_tz::Europe::London),
                lang: Some("EN".to_string()),
                phone: Some("0123456789".to_string()),
                address: None,
                fare_url: Some("http://my-agency_fare_url.com".to_string()),
                sort_order: None,
            };
            assert_eq!(&expected_netword, network);
            assert_eq!(1, companies.len());
        });
    }

    #[test]
    #[should_panic(expected = "`Err` value: identifier 1 already exists")]
    fn load_2_agencies_with_no_id() {
        let agency_content = "agency_name,agency_url,agency_timezone\n\
                              My agency 1,http://my-agency_url.com,Europe/London\n\
                              My agency 2,http://my-agency_url.com,Europe/London";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "agency.txt", agency_content);
            super::read_agency(&mut handler).unwrap();
        });
    }

    #[test]
    fn load_2_agencies_with_different_timezone() {
        let agency_content = "agency_id,agency_name,agency_url,agency_timezone\n\
                              id_1,My agency 1,http://my-agency_url.com,Europe/London\n\
                              id_2,My agency 2,http://my-agency_url.com,Europe/Paris";

        test_in_tmp_dir(|path| {
            testing_logger::setup();
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "agency.txt", agency_content);
            super::read_agency(&mut handler).unwrap();
            testing_logger::validate(|captured_logs| {
                assert_eq!(captured_logs.len(), 2);
                assert!(captured_logs[1].body.contains(
                    "different agency timezone: Europe/London (id_1) - Europe/Paris (id_2)"
                ));
                assert_eq!(captured_logs[1].level, tracing::log::Level::Warn);
            });
        });
    }

    #[test]
    fn load_one_stop_point() {
        let stops_content = "stop_id,stop_name,stop_code,stop_lat,stop_lon\n\
                             id1,my stop name,stopcode,0.1,1.2";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "stops.txt", stops_content);
            let mut equipments = EquipmentList::default();
            let mut comments: CollectionWithId<Comment> = CollectionWithId::default();

            let (stop_areas, stop_points, stop_locations) =
                super::read_stops(&mut handler, &mut comments, &mut equipments).unwrap();
            assert_eq!(1, stop_areas.len());
            assert_eq!(1, stop_points.len());
            assert_eq!(0, stop_locations.len());
            let stop_area = stop_areas.iter().next().unwrap().1;
            assert_eq!("Navitia:id1", stop_area.id);

            assert_eq!(1, stop_points.len());
            let stop_point = stop_points.iter().next().unwrap().1;
            assert_eq!("Navitia:id1", stop_point.stop_area_id);
            assert_eq!("stopcode", stop_point.code.as_ref().unwrap());
        });
    }

    #[test]
    fn load_without_slashes() {
        let stops_content = "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\n\
                             stoparea/01,my stop name 1,0.1,1.2,1,\n\
                             stoppoint/01,my stop name 2,0.1,1.2,0,stoparea/01\n\
                             stoppoint/02,my stop name 3,0.1,1.2,0,stoparea/01";
        let shapes_content =
            "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\n\
             relation/1,12.1280176,-86.214164,1,\n\
             relation/1,12.1279272,-86.2132786,2,";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "stops.txt", stops_content);
            create_file_with_content(path, "shapes.txt", shapes_content);
            let mut collections = Collections::default();
            let mut equipments = EquipmentList::default();
            let mut comments: CollectionWithId<Comment> = CollectionWithId::default();
            // let stop_file = File::open(path.join("stops.txt")).unwrap();
            let (stop_areas, stop_points, stop_locations) =
                super::read_stops(&mut handler, &mut comments, &mut equipments).unwrap();
            collections.stop_areas = stop_areas;
            collections.stop_points = stop_points;
            collections.stop_locations = stop_locations;
            super::manage_shapes(&mut collections, &mut handler).unwrap();
            let stop_area = collections.stop_areas.iter().next().unwrap().1;
            assert_eq!("stoparea01", stop_area.id);
            assert_eq!(
                vec![("stoppoint01", "stoparea01"), ("stoppoint02", "stoparea01"),],
                extract(
                    |sp| (sp.id.as_str(), sp.stop_area_id.as_str()),
                    &collections.stop_points
                )
            );

            assert_eq!(
                vec!["relation1"],
                extract(|geo| geo.id.as_str(), &collections.geometries)
            );
        });
    }

    #[test]
    fn stop_code_on_stops() {
        let stops_content =
            "stop_id,stop_code,stop_name,stop_lat,stop_lon,location_type,parent_station\n\
             stoppoint_id,1234,my stop name,0.1,1.2,0,stop_area_id\n\
             stoparea_id,5678,stop area name,0.1,1.2,1,";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "stops.txt", stops_content);
            let mut equipments = EquipmentList::default();
            let mut comments: CollectionWithId<Comment> = CollectionWithId::default();
            let (stop_areas, stop_points, stop_locations) =
                super::read_stops(&mut handler, &mut comments, &mut equipments).unwrap();
            //validate stop_point code
            assert_eq!(1, stop_points.len());
            let stop_point = stop_points.iter().next().unwrap().1;
            assert_eq!("1234", stop_point.code.as_ref().unwrap());
            assert_eq!(2, stop_point.codes.len());
            let mut codes_iterator = stop_point.codes.iter();
            let code = codes_iterator.next().unwrap();
            assert_eq!("gtfs_stop_code", code.0);
            assert_eq!("1234", code.1);
            let code = codes_iterator.next().unwrap();
            assert_eq!("source", code.0);
            assert_eq!("stoppoint_id", code.1);

            //validate stop_area code
            assert_eq!(1, stop_areas.len());
            let stop_area = stop_areas.iter().next().unwrap().1;
            assert_eq!(2, stop_area.codes.len());
            let mut codes_iterator = stop_area.codes.iter();
            let code = codes_iterator.next().unwrap();
            assert_eq!("gtfs_stop_code", code.0);
            assert_eq!("5678", code.1);
            let code = codes_iterator.next().unwrap();
            assert_eq!("source", code.0);
            assert_eq!("stoparea_id", code.1);
            assert_eq!(0, stop_locations.len());
        });
    }

    #[test]
    fn no_stop_code_on_autogenerated_stoparea() {
        let stops_content =
            "stop_id,stop_code,stop_name,stop_lat,stop_lon,location_type,parent_station\n\
             stoppoint_id,1234,my stop name,0.1,1.2,0,";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "stops.txt", stops_content);
            let mut equipments = EquipmentList::default();
            let mut comments: CollectionWithId<Comment> = CollectionWithId::default();
            let (stop_areas, _, _) =
                super::read_stops(&mut handler, &mut comments, &mut equipments).unwrap();
            //validate stop_area code
            assert_eq!(1, stop_areas.len());
            let stop_area = stop_areas.iter().next().unwrap().1;
            assert_eq!(0, stop_area.codes.len());
        });
    }

    #[test]
    fn gtfs_routes_as_line() {
        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1,3,8F7A32,FFFFFF\n\
                              route_2,agency_2,,My line 2,2,7BC142,000000\n\
                              route_3,agency_3,3,My line 3,8,,\n\
                              route_4,agency_4,3,My line 3 for agency 3,8,,";

        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,,service_1,,\n\
             2,route_1,1,service_1,,\n\
             3,route_2,0,service_2,,\n\
             4,route_3,0,service_3,,\n\
             5,route_4,0,service_4,,";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "routes.txt", routes_content);
            create_file_with_content(path, "trips.txt", trips_content);
            let mut collections = Collections::default();
            let (contributor, dataset, _) = read_config(None::<&str>).unwrap();
            collections.contributors = CollectionWithId::new(vec![contributor]).unwrap();
            collections.datasets = CollectionWithId::new(vec![dataset]).unwrap();
            super::read_routes(&mut handler, &mut collections, false, false).unwrap();
            assert_eq!(4, collections.lines.len());
            assert_eq!(
                vec!["agency_1", "agency_2", "agency_3", "agency_4"],
                extract(|l| &l.network_id, &collections.lines)
            );
            assert_eq!(3, collections.commercial_modes.len());

            assert_eq!(
                vec!["Bus", "Train", "Unknown mode"],
                extract(|cm| &cm.name, &collections.commercial_modes)
            );

            let lines_commercial_modes_id: Vec<String> = collections
                .lines
                .values()
                .map(|l| l.commercial_mode_id.clone())
                .collect();
            assert!(lines_commercial_modes_id.contains(&"Train".to_string()));
            assert!(lines_commercial_modes_id.contains(&"Bus".to_string()));
            assert!(lines_commercial_modes_id.contains(&"UnknownMode".to_string()));

            assert_eq!(2, collections.physical_modes.len());
            assert_eq!(
                vec!["Bus", "Train"],
                extract(|pm| &pm.name, &collections.physical_modes)
            );

            assert_eq!(5, collections.routes.len());

            assert_eq!(
                vec!["route_1", "route_1_R", "route_2", "route_3", "route_4"],
                extract_ids(&collections.routes)
            );
        });
    }

    #[test]
    fn gtfs_read_trip_short_name_and_headsign() {
        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1,3,8F7A32,FFFFFF\n\
                              route_2,agency_2,,My line 2,2,7BC142,000000\n\
                              route_3,agency_3,3,My line 3,8,,";

        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed,trip_headsign,trip_short_name\n\
             1,route_1,,service_1,,,,\n\
             2,route_1,1,service_1,,,headsign2,\n\
             3,route_2,0,service_2,,,,3333\n\
             4,route_3,0,service_3,,,headsign4,4444";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "routes.txt", routes_content);
            create_file_with_content(path, "trips.txt", trips_content);
            let mut collections = Collections::default();
            let (contributor, dataset, _) = read_config(None::<&str>).unwrap();
            collections.contributors = CollectionWithId::new(vec![contributor]).unwrap();
            collections.datasets = CollectionWithId::new(vec![dataset]).unwrap();

            // NTFS trip heasign = GTFS trip short name if exists else GTFS headsign
            // NTFS trip short name is always None
            super::read_routes(&mut handler, &mut collections, false, false).unwrap();

            let vjs = &collections.vehicle_journeys;
            assert_eq!(vjs.get("1").unwrap().headsign.as_deref(), None);
            assert_eq!(vjs.get("2").unwrap().headsign.as_deref(), Some("headsign2"));
            assert_eq!(vjs.get("3").unwrap().headsign.as_deref(), Some("3333"));
            assert_eq!(vjs.get("4").unwrap().headsign.as_deref(), Some("4444"));

            assert_eq!(vjs.get("1").unwrap().short_name.as_deref(), None);
            assert_eq!(vjs.get("2").unwrap().short_name.as_deref(), None);
            assert_eq!(vjs.get("3").unwrap().short_name.as_deref(), None);
            assert_eq!(vjs.get("4").unwrap().short_name.as_deref(), None);

            // NTFS trip headsign = GTFS trip headsign
            // NTFS trip short name = GTFS trip short name
            super::read_routes(&mut handler, &mut collections, false, true).unwrap();

            let vjs = &collections.vehicle_journeys;
            assert_eq!(vjs.get("1").unwrap().headsign.as_deref(), None);
            assert_eq!(vjs.get("2").unwrap().headsign.as_deref(), Some("headsign2"));
            assert_eq!(vjs.get("3").unwrap().headsign.as_deref(), None);
            assert_eq!(vjs.get("4").unwrap().headsign.as_deref(), Some("headsign4"));

            assert_eq!(vjs.get("1").unwrap().short_name.as_deref(), None);
            assert_eq!(vjs.get("2").unwrap().short_name.as_deref(), None);
            assert_eq!(vjs.get("3").unwrap().short_name.as_deref(), Some("3333"));
            assert_eq!(vjs.get("4").unwrap().short_name.as_deref(), Some("4444"));
        });
    }

    #[test]
    fn gtfs_routes_without_agency_id_as_line() {
        let agency_content = "agency_id,agency_name,agency_url,agency_timezone\n\
                              id_agency,My agency,http://my-agency_url.com,Europe/London";

        let routes_content =
            "route_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
             route_1,1,My line 1,3,8F7A32,FFFFFF\n\
             route_2,,My line 2,2,7BC142,000000\n\
             route_3,3,My line 3,8,,\n\
             route_4,3,My line 3 for agency 3,8,,";

        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,,service_1,,\n\
             2,route_1,1,service_1,,\n\
             3,route_2,0,service_2,,\n\
             4,route_3,0,service_3,,\n\
             5,route_4,0,service_4,,";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "agency.txt", agency_content);
            create_file_with_content(path, "routes.txt", routes_content);
            create_file_with_content(path, "trips.txt", trips_content);

            let mut collections = Collections::default();
            let (networks, _) = super::read_agency(&mut handler).unwrap();
            collections.networks = networks;
            let (contributor, dataset, _) = read_config(None::<&str>).unwrap();
            collections.contributors = CollectionWithId::new(vec![contributor]).unwrap();
            collections.datasets = CollectionWithId::new(vec![dataset]).unwrap();
            super::read_routes(&mut handler, &mut collections, false, false).unwrap();
            assert_eq!(3, collections.lines.len());

            assert_eq!(5, collections.routes.len());

            assert_eq!(
                vec!["id_agency", "id_agency", "id_agency"],
                extract(|l| &l.network_id, &collections.lines)
            );
        });
    }

    #[test]
    fn gtfs_routes_with_wrong_colors() {
        let agency_content = "agency_id,agency_name,agency_url,agency_timezone\n\
                              id_agency1,My agency 1,http://my-agency_url1.com,Europe/London";
        let routes_content =
            "route_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
             route_1,1,My line 1,3,0,FFFFFF\n\
             route_2,,My line 2,2,7BC142,0\n\
             route_3,3,My line 3,8,FFFFFF,000000\n\
             route_4,3,My line 3 for agency 3,8,FFFFAF,FAFFFF";

        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,,service_1,,\n\
             2,route_1,1,service_1,,\n\
             3,route_2,0,service_2,,\n\
             4,route_3,0,service_3,,\n\
             5,route_4,0,service_4,,";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "agency.txt", agency_content);
            create_file_with_content(path, "routes.txt", routes_content);
            create_file_with_content(path, "trips.txt", trips_content);

            let mut collections = Collections::default();
            let (networks, _) = super::read_agency(&mut handler).unwrap();
            collections.networks = networks;
            let (contributor, dataset, _) = read_config(None::<&str>).unwrap();
            collections.contributors = CollectionWithId::new(vec![contributor]).unwrap();
            collections.datasets = CollectionWithId::new(vec![dataset]).unwrap();
            super::read_routes(&mut handler, &mut collections, false, false).unwrap();
            assert_eq!(3, collections.lines.len());
            assert_eq!(
                vec![
                    (
                        &None,
                        &Some(Rgb {
                            red: 255,
                            green: 255,
                            blue: 255
                        })
                    ),
                    (
                        &Some(Rgb {
                            red: 123,
                            green: 193,
                            blue: 66
                        }),
                        &None
                    ),
                    (
                        &Some(Rgb {
                            red: 255,
                            green: 255,
                            blue: 255
                        }),
                        &Some(Rgb {
                            red: 0,
                            green: 0,
                            blue: 0
                        })
                    )
                ],
                extract(|l| (&l.color, &l.text_color), &collections.lines)
            );
        });
    }

    #[test]
    #[should_panic(expected = "Impossible to get agency id, several networks found")]
    fn gtfs_routes_without_agency_id_as_line_and_2_agencies() {
        let agency_content = "agency_id,agency_name,agency_url,agency_timezone\n\
                              id_agency1,My agency 1,http://my-agency_url1.com,Europe/London\n\
                              id_agency2,My agency 2,http://my-agency_url2.com,Europe/London";

        let routes_content =
            "route_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
             route_1,1,My line 1,3,8F7A32,FFFFFF\n\
             route_2,,My line 2,2,7BC142,000000\n\
             route_3,3,My line 3,8,,\n\
             route_4,3,My line 3 for agency 3,8,,";

        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,,service_1,,\n\
             2,route_1,1,service_1,,\n\
             3,route_2,0,service_2,,\n\
             4,route_3,0,service_3,,\n\
             5,route_4,0,service_4,,";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "agency.txt", agency_content);
            create_file_with_content(path, "routes.txt", routes_content);
            create_file_with_content(path, "trips.txt", trips_content);

            let mut collections = Collections::default();
            let (networks, _) = super::read_agency(&mut handler).unwrap();
            collections.networks = networks;
            let (contributor, dataset, _) = read_config(None::<&str>).unwrap();
            collections.contributors = CollectionWithId::new(vec![contributor]).unwrap();
            collections.datasets = CollectionWithId::new(vec![dataset]).unwrap();
            super::read_routes(&mut handler, &mut collections, false, false).unwrap();
        });
    }

    #[test]
    #[should_panic(expected = "Impossible to get agency id, no network found")]
    fn gtfs_routes_without_agency_id_as_line_and_0_agencies() {
        let routes_content =
            "route_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
             route_1,1,My line 1,3,8F7A32,FFFFFF\n\
             route_2,,My line 2,2,7BC142,000000\n\
             route_3,3,My line 3,8,,\n\
             route_4,3,My line 3 for agency 3,8,,";

        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,,service_1,,\n\
             2,route_1,1,service_1,,\n\
             3,route_2,0,service_2,,\n\
             4,route_3,0,service_3,,\n\
             5,route_4,0,service_4,,";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "routes.txt", routes_content);
            create_file_with_content(path, "trips.txt", trips_content);

            let mut collections = Collections::default();
            let (contributor, dataset, _) = read_config(None::<&str>).unwrap();
            collections.contributors = CollectionWithId::new(vec![contributor]).unwrap();
            collections.datasets = CollectionWithId::new(vec![dataset]).unwrap();
            super::read_routes(&mut handler, &mut collections, false, false).unwrap();
        });
    }

    #[test]
    fn gtfs_routes_as_route() {
        let agency_content = "agency_id,agency_name,agency_url,agency_timezone\n\
                              id_agency,My agency,http://my-agency_url.com,Europe/London";

        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1A,3,8F7A32,FFFFFF\n\
                              route_2,agency_1,1,My line 1B,3,8F7A32,FFFFFF\n\
                              route_4,agency_2,1,My line 1B,3,8F7A32,FFFFFF\n\
                              route_3,agency_2,1,My line 1B,3,8F7A32,FFFFFF\n\
                              route_5,,1,My line 1C,3,8F7A32,FFFFFF";

        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,0,service_1,,\n\
             2,route_2,0,service_1,,\n\
             3,route_3,0,service_2,,\n\
             4,route_4,0,service_2,,\n\
             5,route_5,0,service_3,,";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "agency.txt", agency_content);
            create_file_with_content(path, "routes.txt", routes_content);
            create_file_with_content(path, "trips.txt", trips_content);
            let mut collections = Collections::default();
            let (networks, _) = super::read_agency(&mut handler).unwrap();
            collections.networks = networks;
            let (contributor, dataset, _) = read_config(None::<&str>).unwrap();
            collections.contributors = CollectionWithId::new(vec![contributor]).unwrap();
            collections.datasets = CollectionWithId::new(vec![dataset]).unwrap();
            super::read_routes(&mut handler, &mut collections, false, false).unwrap();

            assert_eq!(3, collections.lines.len());
            assert_eq!(
                vec!["agency_1", "agency_2", "id_agency"],
                extract(|l| &l.network_id, &collections.lines)
            );
            assert_eq!(
                vec!["route_1", "route_3", "route_5"],
                extract_ids(&collections.lines)
            );
            assert_eq!(5, collections.routes.len());

            assert_eq!(
                vec!["route_1", "route_1", "route_3", "route_3", "route_5"],
                extract(|r| &r.line_id, &collections.routes)
            );
        });
    }

    #[test]
    fn gtfs_routes_as_route_with_backward_trips() {
        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1A,3,8F7A32,FFFFFF\n\
                              route_2,agency_1,1,My line 1B,3,8F7A32,FFFFFF\n\
                              route_3,agency_2,,My line 2,2,7BC142,000000";

        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,0,service_1,,\n\
             2,route_1,1,service_1,,\n\
             3,route_2,0,service_2,,\n
             4,route_3,0,service_3,,\n\
             5,route_3,1,service_3,,";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "routes.txt", routes_content);
            create_file_with_content(path, "trips.txt", trips_content);
            let mut collections = Collections::default();
            let (contributor, dataset, _) = read_config(None::<&str>).unwrap();
            collections.contributors = CollectionWithId::new(vec![contributor]).unwrap();
            collections.datasets = CollectionWithId::new(vec![dataset]).unwrap();
            super::read_routes(&mut handler, &mut collections, false, false).unwrap();

            assert_eq!(2, collections.lines.len());

            assert_eq!(5, collections.routes.len());
            assert_eq!(
                vec!["route_1", "route_1_R", "route_2", "route_3", "route_3_R"],
                extract_ids(&collections.routes)
            );
        });
    }

    #[test]
    fn gtfs_routes_as_route_same_name_different_agency() {
        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1A,3,8F7A32,FFFFFF\n\
                              route_2,agency_1,1,My line 1B,3,8F7A32,FFFFFF\n\
                              route_3,agency_2,1,My line 1 for agency 2,3,8F7A32,FFFFFF";

        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,0,service_1,,\n\
             2,route_2,0,service_2,,\n
             3,route_3,0,service_3,,";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "routes.txt", routes_content);
            create_file_with_content(path, "trips.txt", trips_content);
            let mut collections = Collections::default();
            let (contributor, dataset, _) = read_config(None::<&str>).unwrap();
            collections.contributors = CollectionWithId::new(vec![contributor]).unwrap();
            collections.datasets = CollectionWithId::new(vec![dataset]).unwrap();
            super::read_routes(&mut handler, &mut collections, false, false).unwrap();

            assert_eq!(2, collections.lines.len());
            assert_eq!(vec!["route_1", "route_3"], extract_ids(&collections.lines));
            assert_eq!(
                vec!["route_1", "route_2", "route_3"],
                extract_ids(&collections.routes)
            );

            assert_eq!(
                vec!["route_1", "route_1", "route_3"],
                extract(|r| &r.line_id, &collections.routes)
            );
        });
    }

    #[test]
    fn gtfs_routes_with_no_trips() {
        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1,3,8F7A32,FFFFFF\n\
                              route_2,agency_2,2,My line 2,3,8F7A32,FFFFFF";
        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,0,service_1,,";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "routes.txt", routes_content);
            create_file_with_content(path, "trips.txt", trips_content);

            let mut collections = Collections::default();
            let (contributor, dataset, _) = read_config(None::<&str>).unwrap();
            collections.contributors = CollectionWithId::new(vec![contributor]).unwrap();
            collections.datasets = CollectionWithId::new(vec![dataset]).unwrap();
            super::read_routes(&mut handler, &mut collections, false, false).unwrap();
            assert_eq!(1, collections.lines.len());
            assert_eq!(1, collections.routes.len());
        });
    }

    #[test]
    fn prefix_on_all_pt_object_id() {
        let stops_content =
            "stop_id,stop_name,stop_desc,stop_lat,stop_lon,location_type,parent_station,wheelchair_boarding\n\
             sp:01,my stop point name,my first desc,0.1,1.2,0,,1\n\
             sp:02,my stop point name child,,0.2,1.5,0,sp:01,2\n\
             sa:03,my stop area name,my second desc,0.3,2.2,1,,1";
        let agency_content = "agency_id,agency_name,agency_url,agency_timezone,agency_lang\n\
                              584,TAM,http://whatever.canaltp.fr/,Europe/Paris,fr\n\
                              285,Phébus,http://plop.kisio.com/,Europe/London,en";

        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color,destination_id\n\
                              route_1,agency_1,1,My line 1A,3,8F7A32,FFFFFF,\n\
                              route_2,agency_1,2,My line 1B,3,8F7A32,FFFFFF,sp:01";

        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed,shape_id\n\
             1,route_1,0,service_1,,,1\n\
             2,route_2,1,service_2,1,2,2";

        let transfers_content = "from_stop_id,to_stop_id,transfer_type,min_transfer_time\n\
                                 sp:01,sp:01,1,\n\
                                 sp:01,sp:02,0,\n\
                                 sp:02,sp:01,0,\n\
                                 sp:02,sp:02,1,";

        let shapes_content = "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\n\
                              1,4.4,3.3,2\n\
                              2,6.6,5.5,1";

        let calendar = "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
                       1,0,0,0,0,0,1,1,20180501,20180508\n\
                       2,1,0,0,0,0,0,0,20180502,20180506";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "stops.txt", stops_content);
            create_file_with_content(path, "agency.txt", agency_content);
            create_file_with_content(path, "routes.txt", routes_content);
            create_file_with_content(path, "trips.txt", trips_content);
            create_file_with_content(path, "transfers.txt", transfers_content);
            create_file_with_content(path, "shapes.txt", shapes_content);
            create_file_with_content(path, "calendar.txt", calendar);

            let mut collections = Collections::default();

            let mut comments: CollectionWithId<Comment> = CollectionWithId::default();
            let mut equipments = EquipmentList::default();
            let (contributor, dataset, _) = read_config(None::<&str>).unwrap();
            collections.contributors = CollectionWithId::new(vec![contributor]).unwrap();
            collections.datasets = CollectionWithId::new(vec![dataset]).unwrap();
            let (stop_areas, stop_points, stop_locations) =
                super::read_stops(&mut handler, &mut comments, &mut equipments).unwrap();
            collections.equipments = CollectionWithId::new(equipments.into_equipments()).unwrap();
            collections.transfers =
                super::read_transfers(&mut handler, &stop_points, &stop_areas).unwrap();
            collections.stop_areas = stop_areas;
            collections.stop_points = stop_points;
            collections.stop_locations = stop_locations;

            let (networks, companies) = super::read_agency(&mut handler).unwrap();
            collections.networks = networks;
            collections.companies = companies;
            collections.comments = comments;
            super::read_routes(&mut handler, &mut collections, false, false).unwrap();
            super::manage_shapes(&mut collections, &mut handler).unwrap();
            calendars::manage_calendars(&mut handler, &mut collections).unwrap();

            let mut prefix_conf = PrefixConfiguration::default();
            prefix_conf.set_data_prefix("my_prefix");
            collections.prefix(&prefix_conf);

            assert_eq!(
                vec!["my_prefix:285", "my_prefix:584"],
                extract_ids(&collections.companies)
            );
            assert_eq!(
                vec!["my_prefix:285", "my_prefix:584"],
                extract_ids(&collections.networks)
            );
            assert_eq!(
                vec![
                    ("my_prefix:Navitia:sp:01", None),
                    ("my_prefix:sa:03", Some("my_prefix:0")),
                ],
                extract(
                    |obj| (obj.id.as_str(), obj.equipment_id.as_deref()),
                    &collections.stop_areas,
                )
            );
            assert_eq!(
                vec![
                    (
                        "my_prefix:sp:01",
                        "my_prefix:Navitia:sp:01",
                        Some("my_prefix:0")
                    ),
                    ("my_prefix:sp:02", "my_prefix:sp:01", Some("my_prefix:1")),
                ],
                extract(
                    |obj| (
                        obj.id.as_str(),
                        obj.stop_area_id.as_str(),
                        obj.equipment_id.as_deref()
                    ),
                    &collections.stop_points,
                )
            );
            assert_eq!(
                vec![
                    ("my_prefix:route_1", "my_prefix:agency_1", "Bus"),
                    ("my_prefix:route_2", "my_prefix:agency_1", "Bus"),
                ],
                extract(
                    |obj| (
                        obj.id.as_str(),
                        obj.network_id.as_str(),
                        obj.commercial_mode_id.as_str(),
                    ),
                    &collections.lines,
                )
            );
            assert_eq!(
                vec![
                    ("my_prefix:route_1", "my_prefix:route_1", None),
                    ("my_prefix:route_2_R", "my_prefix:route_2", None),
                ],
                extract(
                    |obj| (
                        obj.id.as_str(),
                        obj.line_id.as_str(),
                        obj.destination_id.as_deref()
                    ),
                    &collections.routes,
                )
            );
            assert_eq!(
                vec!["my_prefix:1"],
                extract_ids(&collections.trip_properties)
            );
            assert_eq!(
                vec!["my_prefix:stop:sa:03", "my_prefix:stop:sp:01"],
                extract_ids(&collections.comments)
            );
            assert_eq!(vec!["Bus"], extract_ids(&collections.commercial_modes));
            assert_eq!(
                vec![
                    ("my_prefix:sp:01", "my_prefix:sp:01"),
                    ("my_prefix:sp:01", "my_prefix:sp:02"),
                    ("my_prefix:sp:02", "my_prefix:sp:01"),
                    ("my_prefix:sp:02", "my_prefix:sp:02"),
                ],
                extract(
                    |sp| (sp.from_stop_id.as_str(), sp.to_stop_id.as_str()),
                    &collections.transfers,
                )
            );
            assert_eq!(
                vec!["my_prefix:default_contributor"],
                extract_ids(&collections.contributors)
            );
            assert_eq!(
                vec![("my_prefix:default_dataset", "my_prefix:default_contributor")],
                extract(
                    |obj| (obj.id.as_str(), obj.contributor_id.as_str()),
                    &collections.datasets,
                )
            );
            assert_eq!(
                vec![
                    (
                        "my_prefix:1",
                        "my_prefix:route_1",
                        "my_prefix:default_dataset",
                        "my_prefix:service_1",
                        Some("my_prefix:1"),
                    ),
                    (
                        "my_prefix:2",
                        "my_prefix:route_2_R",
                        "my_prefix:default_dataset",
                        "my_prefix:service_2",
                        Some("my_prefix:2"),
                    ),
                ],
                extract(
                    |obj| (
                        obj.id.as_str(),
                        obj.route_id.as_str(),
                        obj.dataset_id.as_str(),
                        obj.service_id.as_str(),
                        obj.geometry_id.as_deref()
                    ),
                    &collections.vehicle_journeys,
                )
            );
            assert_eq!(
                vec!["my_prefix:0", "my_prefix:1"],
                extract_ids(&collections.equipments)
            );
            assert_eq!(
                vec!["my_prefix:1", "my_prefix:2"],
                extract_ids(&collections.geometries)
            );
            assert_eq!(vec!["my_prefix:1"], extract_ids(&collections.calendars));
        });
    }

    #[test]
    fn gtfs_trips() {
        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1,3,8F7A32,FFFFFF\n\
                              route_2,agency_2,2,My line 2,3,8F7A32,FFFFFF\n\
                              route_3,agency_3,3,My line 3,3,8F7A32,FFFFFF";
        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,0,service_1,,\n\
             2,route_2,0,service_1,1,2\n\
             3,route_3,0,service_1,1,2
             4,unknown_route,0,service_1,1,2";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "routes.txt", routes_content);
            create_file_with_content(path, "trips.txt", trips_content);

            let mut collections = Collections::default();
            let (contributor, dataset, _) = read_config(None::<&str>).unwrap();
            collections.contributors = CollectionWithId::new(vec![contributor]).unwrap();
            collections.datasets = CollectionWithId::new(vec![dataset]).unwrap();

            super::read_routes(&mut handler, &mut collections, false, false).unwrap();
            assert_eq!(3, collections.lines.len());
            assert_eq!(3, collections.routes.len());
            assert_eq!(3, collections.vehicle_journeys.len());
            assert_eq!(
                vec!["agency_1", "agency_2", "agency_3"],
                extract(|vj| &vj.company_id, &collections.vehicle_journeys)
            );
            assert_eq!(1, collections.trip_properties.len());
        });
    }

    #[test]
    fn gtfs_trips_with_routes_without_agency_id() {
        let agency_content = "agency_id,agency_name,agency_url,agency_timezone\n\
                              id_agency,My agency,http://my-agency_url.com,Europe/London";

        let routes_content =
            "route_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
             route_1,1,My line 1,3,8F7A32,FFFFFF\n\
             route_2,2,My line 2,3,8F7A32,FFFFFF\n\
             route_3,3,My line 3,3,8F7A32,FFFFFF";
        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,0,service_1,,\n\
             2,route_2,0,service_1,1,2\n\
             3,route_3,0,service_1,1,2";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "agency.txt", agency_content);
            create_file_with_content(path, "routes.txt", routes_content);
            create_file_with_content(path, "trips.txt", trips_content);

            let mut collections = Collections::default();
            let (networks, _) = super::read_agency(&mut handler).unwrap();
            collections.networks = networks;
            let (contributor, dataset, _) = read_config(None::<&str>).unwrap();
            collections.contributors = CollectionWithId::new(vec![contributor]).unwrap();
            collections.datasets = CollectionWithId::new(vec![dataset]).unwrap();

            super::read_routes(&mut handler, &mut collections, false, false).unwrap();
            assert_eq!(3, collections.lines.len());
            assert_eq!(3, collections.routes.len());
            assert_eq!(3, collections.vehicle_journeys.len());
            assert_eq!(
                vec!["id_agency", "id_agency", "id_agency"],
                extract(|vj| &vj.company_id, &collections.vehicle_journeys)
            );
            assert_eq!(1, collections.trip_properties.len());
        });
    }

    #[test]
    fn gtfs_trips_no_direction_id() {
        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1,3,8F7A32,FFFFFF\n\
                              route_2,agency_2,2,My line 2,3,8F7A32,FFFFFF\n\
                              route_3,agency_3,3,My line 3,3,8F7A32,FFFFFF";
        let trips_content = "trip_id,route_id,service_id,wheelchair_accessible,bikes_allowed\n\
                             1,route_1,service_1,,\n\
                             2,route_2,service_1,1,2\n\
                             3,route_3,service_1,1,2";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "routes.txt", routes_content);
            create_file_with_content(path, "trips.txt", trips_content);

            let mut collections = Collections::default();
            let (contributor, dataset, _) = read_config(None::<&str>).unwrap();
            collections.contributors = CollectionWithId::new(vec![contributor]).unwrap();
            collections.datasets = CollectionWithId::new(vec![dataset]).unwrap();

            super::read_routes(&mut handler, &mut collections, false, false).unwrap();
            assert_eq!(3, collections.lines.len());
            assert_eq!(3, collections.routes.len());

            assert_eq!(
                vec![
                    &Some("forward".to_string()),
                    &Some("forward".to_string()),
                    &Some("forward".to_string())
                ],
                extract(|r| &r.direction_type, &collections.routes)
            );
        });
    }

    #[test]
    fn gtfs_trips_with_no_accessibility_information() {
        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1,3,8F7A32,FFFFFF";
        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,0,service_1,,\n\
             2,route_1,0,service_2,,";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "routes.txt", routes_content);
            create_file_with_content(path, "trips.txt", trips_content);

            let mut collections = Collections::default();
            let (contributor, dataset, _) = read_config(None::<&str>).unwrap();
            collections.contributors = CollectionWithId::new(vec![contributor]).unwrap();
            collections.datasets = CollectionWithId::new(vec![dataset]).unwrap();

            super::read_routes(&mut handler, &mut collections, false, false).unwrap();
            assert_eq!(2, collections.vehicle_journeys.len());
            assert_eq!(0, collections.trip_properties.len());
            for vj in collections.vehicle_journeys.values() {
                assert!(vj.trip_property_id.is_none());
            }
        });
    }

    #[test]
    fn push_on_collection() {
        let mut c = CollectionWithId::default();
        c.push(Comment {
            id: "foo".into(),
            name: "toto".into(),
            comment_type: CommentType::Information,
            url: None,
            label: None,
        })
        .unwrap();
        assert!(c
            .push(Comment {
                id: "foo".into(),
                name: "tata".into(),
                comment_type: CommentType::Information,
                url: None,
                label: None,
            })
            .is_err());
        let id = c.get_idx("foo").unwrap();
        assert_eq!(c.iter().next().unwrap().0, id);
    }

    #[test]
    fn stops_generates_equipments() {
        let stops_content = "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station,wheelchair_boarding\n\
                             sp:01,my stop point name,0.1,1.2,0,,1\n\
                             sp:02,my stop point name child,0.2,1.5,0,sp:01,\n\
                             sa:03,my stop area name,0.3,2.2,1,,2";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "stops.txt", stops_content);

            let mut comments: CollectionWithId<Comment> = CollectionWithId::default();
            let mut equipments = EquipmentList::default();
            let (stop_areas, stop_points, _) =
                super::read_stops(&mut handler, &mut comments, &mut equipments).unwrap();
            let equipments_collection =
                CollectionWithId::new(equipments.into_equipments()).unwrap();
            assert_eq!(2, stop_areas.len());
            assert_eq!(2, stop_points.len());
            assert_eq!(2, equipments_collection.len());

            let mut stop_point_equipment_ids: Vec<Option<String>> = stop_points
                .iter()
                .map(|(_, stop_point)| stop_point.equipment_id.clone())
                .collect();
            stop_point_equipment_ids.sort();
            assert_eq!(vec![None, Some("0".to_string())], stop_point_equipment_ids);

            assert_eq!(
                vec![&None, &Some("1".to_string())],
                extract(|sa| &sa.equipment_id, &stop_areas)
            );
            use objects::Availability::*;
            assert_eq!(
                vec![
                    Equipment {
                        id: "0".to_string(),
                        wheelchair_boarding: Available,
                        sheltered: InformationNotAvailable,
                        elevator: InformationNotAvailable,
                        escalator: InformationNotAvailable,
                        bike_accepted: InformationNotAvailable,
                        bike_depot: InformationNotAvailable,
                        visual_announcement: InformationNotAvailable,
                        audible_announcement: InformationNotAvailable,
                        appropriate_escort: InformationNotAvailable,
                        appropriate_signage: InformationNotAvailable,
                    },
                    Equipment {
                        id: "1".to_string(),
                        wheelchair_boarding: NotAvailable,
                        sheltered: InformationNotAvailable,
                        elevator: InformationNotAvailable,
                        escalator: InformationNotAvailable,
                        bike_accepted: InformationNotAvailable,
                        bike_depot: InformationNotAvailable,
                        visual_announcement: InformationNotAvailable,
                        audible_announcement: InformationNotAvailable,
                        appropriate_escort: InformationNotAvailable,
                        appropriate_signage: InformationNotAvailable,
                    },
                ],
                equipments_collection.into_vec()
            );
        });
    }

    #[test]
    fn stops_do_not_generate_duplicate_equipments() {
        let stops_content = "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station,wheelchair_boarding\n\
                             sp:01,my stop point name 1,0.1,1.2,0,,1\n\
                             sp:02,my stop point name 2,0.2,1.5,0,,1";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "stops.txt", stops_content);

            let mut comments: CollectionWithId<Comment> = CollectionWithId::default();
            let mut equipments = EquipmentList::default();
            let (_, stop_points, _) =
                super::read_stops(&mut handler, &mut comments, &mut equipments).unwrap();
            let equipments_collection =
                CollectionWithId::new(equipments.into_equipments()).unwrap();
            assert_eq!(2, stop_points.len());
            assert_eq!(1, equipments_collection.len());

            let mut stop_point_equipment_ids: Vec<Option<String>> = stop_points
                .iter()
                .map(|(_, stop_point)| stop_point.equipment_id.clone())
                .collect();
            stop_point_equipment_ids.sort();
            assert_eq!(
                vec![Some("0".to_string()), Some("0".to_string())],
                stop_point_equipment_ids
            );

            use objects::Availability::*;
            assert_eq!(
                vec![Equipment {
                    id: "0".to_string(),
                    wheelchair_boarding: Available,
                    sheltered: InformationNotAvailable,
                    elevator: InformationNotAvailable,
                    escalator: InformationNotAvailable,
                    bike_accepted: InformationNotAvailable,
                    bike_depot: InformationNotAvailable,
                    visual_announcement: InformationNotAvailable,
                    audible_announcement: InformationNotAvailable,
                    appropriate_escort: InformationNotAvailable,
                    appropriate_signage: InformationNotAvailable,
                }],
                equipments_collection.into_vec()
            );
        });
    }

    #[test]
    fn gtfs_stop_times_estimated() {
        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1,3,8F7A32,FFFFFF";

        let stops_content =
            "stop_id,stop_name,stop_desc,stop_lat,stop_lon,location_type,parent_station\n\
             sp:01,my stop point name 1,my first desc,0.1,1.2,0,\n\
             sp:02,my stop point name 2,,0.2,1.5,0,\n\
             sp:03,my stop point name 2,,0.2,1.5,0,";

        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,0,service_1,,";

        let stop_times_content = "trip_id,arrival_time,departure_time,stop_id,stop_sequence,stop_headsign,pickup_type,drop_off_type,shape_dist_traveled,timepoint\n\
                                  1,06:00:00,06:00:00,sp:01,1,over there,,,,0\n\
                                  1,06:06:27,06:06:27,sp:02,2,,2,1,,1\n\
                                  1,06:06:27,06:06:27,sp:03,3,,2,1,,";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "routes.txt", routes_content);
            create_file_with_content(path, "trips.txt", trips_content);
            create_file_with_content(path, "stop_times.txt", stop_times_content);
            create_file_with_content(path, "stops.txt", stops_content);

            let mut collections = Collections::default();
            let (contributor, dataset, _) = read_config(None::<&str>).unwrap();
            collections.contributors = CollectionWithId::new(vec![contributor]).unwrap();
            collections.datasets = CollectionWithId::new(vec![dataset]).unwrap();

            let mut comments: CollectionWithId<Comment> = CollectionWithId::default();
            let mut equipments = EquipmentList::default();
            let (_, stop_points, _) =
                super::read_stops(&mut handler, &mut comments, &mut equipments).unwrap();
            collections.stop_points = stop_points;

            super::read_routes(&mut handler, &mut collections, false, false).unwrap();
            let location_groups = HashMap::new();
            super::manage_stop_times(
                &mut collections,
                &mut handler,
                false,
                None,
                &location_groups,
            )
            .unwrap();

            assert_eq!(
                vec![
                    StopTime {
                        stop_point_idx: collections.stop_points.get_idx("sp:01").unwrap(),
                        sequence: 1,
                        arrival_time: Some(Time::new(6, 0, 0)),
                        departure_time: Some(Time::new(6, 0, 0)),
                        start_pickup_drop_off_window: None,
                        end_pickup_drop_off_window: None,
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 0,
                        drop_off_type: 0,
                        local_zone_id: None,
                        precision: Some(StopTimePrecision::Approximate),
                    },
                    StopTime {
                        stop_point_idx: collections.stop_points.get_idx("sp:02").unwrap(),
                        sequence: 2,
                        arrival_time: Some(Time::new(6, 6, 27)),
                        departure_time: Some(Time::new(6, 6, 27)),
                        start_pickup_drop_off_window: None,
                        end_pickup_drop_off_window: None,
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 2,
                        drop_off_type: 1,
                        local_zone_id: None,
                        precision: Some(StopTimePrecision::Exact),
                    },
                    StopTime {
                        stop_point_idx: collections.stop_points.get_idx("sp:03").unwrap(),
                        sequence: 3,
                        arrival_time: Some(Time::new(6, 6, 27)),
                        departure_time: Some(Time::new(6, 6, 27)),
                        start_pickup_drop_off_window: None,
                        end_pickup_drop_off_window: None,
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 2,
                        drop_off_type: 1,
                        local_zone_id: None,
                        precision: Some(StopTimePrecision::Exact),
                    },
                ],
                collections.vehicle_journeys.into_vec()[0].stop_times
            );
        });
    }

    #[test]
    fn gtfs_stop_times_deduplicated() {
        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1,3,8F7A32,FFFFFF";

        let stops_content =
            "stop_id,stop_name,stop_desc,stop_lat,stop_lon,location_type,parent_station\n\
             sp:01,my stop point name 1,,0.1,1.1,0,\n\
             sp:02,my stop point name 2,,0.2,1.2,0,\n\
             sp:03,my stop point name 3,,0.3,1.3,0,\n\
             sp:04,my stop point name 4,,0.4,1.4,0,\n\
             sp:05,my stop point name 5,,0.5,1.5,0,";

        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,0,service_1,,";

        // Duplicated stops sequences 1 and 2
        // Duplicates should not be taken into account ("first come" rule)
        // Uniqueness is on trip_id / stop_sequence
        let stop_times_content = "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
                                  1,06:00:00,06:00:00,sp:01,1\n\
                                  1,06:00:10,06:00:10,sp:04,1\n\
                                  1,06:11:00,06:11:00,sp:02,2\n\
                                  1,06:11:10,06:11:10,sp:05,2\n\
                                  1,06:22:00,06:22:00,sp:03,3";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "routes.txt", routes_content);
            create_file_with_content(path, "trips.txt", trips_content);
            create_file_with_content(path, "stop_times.txt", stop_times_content);
            create_file_with_content(path, "stops.txt", stops_content);

            let mut collections = Collections::default();
            let (contributor, dataset, _) = read_config(None::<&str>).unwrap();
            collections.contributors = CollectionWithId::new(vec![contributor]).unwrap();
            collections.datasets = CollectionWithId::new(vec![dataset]).unwrap();

            let mut comments: CollectionWithId<Comment> = CollectionWithId::default();
            let mut equipments = EquipmentList::default();
            let (_, stop_points, _) =
                super::read_stops(&mut handler, &mut comments, &mut equipments).unwrap();
            collections.stop_points = stop_points;

            super::read_routes(&mut handler, &mut collections, false, false).unwrap();
            let location_groups = HashMap::new();
            super::manage_stop_times(
                &mut collections,
                &mut handler,
                false,
                None,
                &location_groups,
            )
            .unwrap();

            assert_eq!(
                vec![
                    StopTime {
                        stop_point_idx: collections.stop_points.get_idx("sp:01").unwrap(),
                        sequence: 1,
                        arrival_time: Some(Time::new(6, 0, 0)),
                        departure_time: Some(Time::new(6, 0, 0)),
                        start_pickup_drop_off_window: None,
                        end_pickup_drop_off_window: None,
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 0,
                        drop_off_type: 0,
                        local_zone_id: None,
                        precision: Some(StopTimePrecision::Exact),
                    },
                    StopTime {
                        stop_point_idx: collections.stop_points.get_idx("sp:02").unwrap(),
                        sequence: 2,
                        arrival_time: Some(Time::new(6, 11, 0)),
                        departure_time: Some(Time::new(6, 11, 0)),
                        start_pickup_drop_off_window: None,
                        end_pickup_drop_off_window: None,
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 0,
                        drop_off_type: 0,
                        local_zone_id: None,
                        precision: Some(StopTimePrecision::Exact),
                    },
                    StopTime {
                        stop_point_idx: collections.stop_points.get_idx("sp:03").unwrap(),
                        sequence: 3,
                        arrival_time: Some(Time::new(6, 22, 0)),
                        departure_time: Some(Time::new(6, 22, 0)),
                        start_pickup_drop_off_window: None,
                        end_pickup_drop_off_window: None,
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 0,
                        drop_off_type: 0,
                        local_zone_id: None,
                        precision: Some(StopTimePrecision::Exact),
                    },
                ],
                collections.vehicle_journeys.into_vec()[0].stop_times
            );
        });
    }

    #[test]
    fn gtfs_stop_times() {
        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1,3,8F7A32,FFFFFF";

        let stops_content =
            "stop_id,stop_name,stop_desc,stop_lat,stop_lon,location_type,parent_station\n\
             sp:01,my stop point name 1,my first desc,0.1,1.2,0,\n\
             sp:02,my stop point name 2,,0.2,1.5,0,";

        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,0,service_1,,";

        let stop_times_content = "trip_id,arrival_time,departure_time,stop_id,stop_sequence,stop_headsign,pickup_type,drop_off_type,shape_dist_traveled\n\
                                  1,06:00:00,06:00:00,sp:01,1,over there,,,\n\
                                  1,06:06:27,06:06:27,sp:02,2,,2,1,";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "routes.txt", routes_content);
            create_file_with_content(path, "trips.txt", trips_content);
            create_file_with_content(path, "stop_times.txt", stop_times_content);
            create_file_with_content(path, "stops.txt", stops_content);

            let mut collections = Collections::default();
            let (contributor, dataset, _) = read_config(None::<&str>).unwrap();
            collections.contributors = CollectionWithId::new(vec![contributor]).unwrap();
            collections.datasets = CollectionWithId::new(vec![dataset]).unwrap();

            let mut comments: CollectionWithId<Comment> = CollectionWithId::default();
            let mut equipments = EquipmentList::default();
            let (_, stop_points, _) =
                super::read_stops(&mut handler, &mut comments, &mut equipments).unwrap();
            collections.stop_points = stop_points;

            super::read_routes(&mut handler, &mut collections, false, false).unwrap();
            let location_groups = HashMap::new();
            super::manage_stop_times(
                &mut collections,
                &mut handler,
                false,
                None,
                &location_groups,
            )
            .unwrap();

            assert_eq!(
                vec![
                    StopTime {
                        stop_point_idx: collections.stop_points.get_idx("sp:01").unwrap(),
                        sequence: 1,
                        arrival_time: Some(Time::new(6, 0, 0)),
                        departure_time: Some(Time::new(6, 0, 0)),
                        start_pickup_drop_off_window: None,
                        end_pickup_drop_off_window: None,
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 0,
                        drop_off_type: 0,
                        local_zone_id: None,
                        precision: Some(StopTimePrecision::Exact),
                    },
                    StopTime {
                        stop_point_idx: collections.stop_points.get_idx("sp:02").unwrap(),
                        sequence: 2,
                        arrival_time: Some(Time::new(6, 6, 27)),
                        departure_time: Some(Time::new(6, 6, 27)),
                        start_pickup_drop_off_window: None,
                        end_pickup_drop_off_window: None,
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 2,
                        drop_off_type: 1,
                        local_zone_id: None,
                        precision: Some(StopTimePrecision::Exact),
                    },
                ],
                collections.vehicle_journeys.into_vec()[0].stop_times
            );
            let headsigns: Vec<String> =
                collections.stop_time_headsigns.values().cloned().collect();
            assert_eq!(vec!["over there".to_string()], headsigns);
        });
    }

    #[test]
    fn read_tranfers() {
        let stops_content = "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station,wheelchair_boarding\n\
                             sp:01,my stop point name 1,48.857332,2.346331,0,,1\n\
                             sp:02,my stop point name 2,48.858195,2.347448,0,,1\n\
                             sp:03,my stop point name 3,48.859031,2.346958,0,,1";

        let transfers_content = "from_stop_id,to_stop_id,transfer_type,min_transfer_time\n\
                                 sp:01,sp:01,1,\n\
                                 sp:01,sp:02,0,\n\
                                 sp:01,sp:03,2,60\n\
                                 sp:02,sp:01,0,\n\
                                 sp:02,sp:02,1,\n\
                                 sp:02,sp:03,3,\n\
                                 sp:03,sp:01,0,\n\
                                 sp:03,sp:02,2,\n\
                                 sp:03,sp:03,0,";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "stops.txt", stops_content);
            create_file_with_content(path, "transfers.txt", transfers_content);

            let mut comments: CollectionWithId<Comment> = CollectionWithId::default();
            let mut equipments = EquipmentList::default();
            let (stop_areas, stop_points, _) =
                super::read_stops(&mut handler, &mut comments, &mut equipments).unwrap();

            let transfers = super::read_transfers(&mut handler, &stop_points, &stop_areas).unwrap();
            assert_eq!(
                vec![
                    &Transfer {
                        from_stop_id: "sp:01".to_string(),
                        to_stop_id: "sp:01".to_string(),
                        min_transfer_time: Some(0),
                        real_min_transfer_time: Some(0),
                        equipment_id: None,
                    },
                    &Transfer {
                        from_stop_id: "sp:01".to_string(),
                        to_stop_id: "sp:02".to_string(),
                        min_transfer_time: Some(160),
                        real_min_transfer_time: Some(280),
                        equipment_id: None,
                    },
                    &Transfer {
                        from_stop_id: "sp:01".to_string(),
                        to_stop_id: "sp:03".to_string(),
                        min_transfer_time: Some(60),
                        real_min_transfer_time: Some(60),
                        equipment_id: None,
                    },
                    &Transfer {
                        from_stop_id: "sp:02".to_string(),
                        to_stop_id: "sp:01".to_string(),
                        min_transfer_time: Some(160),
                        real_min_transfer_time: Some(280),
                        equipment_id: None,
                    },
                    &Transfer {
                        from_stop_id: "sp:02".to_string(),
                        to_stop_id: "sp:02".to_string(),
                        min_transfer_time: Some(0),
                        real_min_transfer_time: Some(0),
                        equipment_id: None,
                    },
                    &Transfer {
                        from_stop_id: "sp:02".to_string(),
                        to_stop_id: "sp:03".to_string(),
                        min_transfer_time: Some(86400),
                        real_min_transfer_time: Some(86400),
                        equipment_id: None,
                    },
                    &Transfer {
                        from_stop_id: "sp:03".to_string(),
                        to_stop_id: "sp:01".to_string(),
                        min_transfer_time: Some(247),
                        real_min_transfer_time: Some(367),
                        equipment_id: None,
                    },
                    &Transfer {
                        from_stop_id: "sp:03".to_string(),
                        to_stop_id: "sp:02".to_string(),
                        min_transfer_time: None,
                        real_min_transfer_time: None,
                        equipment_id: None,
                    },
                    &Transfer {
                        from_stop_id: "sp:03".to_string(),
                        to_stop_id: "sp:03".to_string(),
                        min_transfer_time: Some(0),
                        real_min_transfer_time: Some(120),
                        equipment_id: None,
                    },
                ],
                transfers.values().collect::<Vec<_>>()
            );
        });
    }

    #[test]
    fn gtfs_with_calendars_and_no_calendar_dates() {
        let content = "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
                       1,0,0,0,0,0,1,1,20180501,20180508\n\
                       2,1,0,0,0,0,0,0,20180502,20180506";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "calendar.txt", content);

            let mut collections = Collections::default();
            calendars::manage_calendars(&mut handler, &mut collections).unwrap();

            let mut dates = BTreeSet::new();
            dates.insert(chrono::NaiveDate::from_ymd_opt(2018, 5, 5).unwrap());
            dates.insert(chrono::NaiveDate::from_ymd_opt(2018, 5, 6).unwrap());
            assert_eq!(
                vec![Calendar {
                    id: "1".to_string(),
                    dates,
                },],
                collections.calendars.into_vec()
            );
        });
    }

    #[test]
    fn gtfs_with_calendars_dates_and_no_calendar() {
        let content = "service_id,date,exception_type\n\
                       1,20180212,1\n\
                       1,20180211,2\n\
                       2,20180211,2";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "calendar_dates.txt", content);

            let mut collections = Collections::default();
            calendars::manage_calendars(&mut handler, &mut collections).unwrap();

            let mut dates = BTreeSet::new();
            dates.insert(chrono::NaiveDate::from_ymd_opt(2018, 2, 12).unwrap());
            assert_eq!(
                vec![Calendar {
                    id: "1".to_string(),
                    dates,
                }],
                collections.calendars.into_vec()
            );
        });
    }

    #[test]
    fn gtfs_with_calendars_and_calendar_dates() {
        let calendars_content = "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
                                 1,0,0,0,0,0,1,1,20180501,20180508\n\
                                 2,0,0,0,0,0,0,1,20180502,20180506";

        let calendar_dates_content = "service_id,date,exception_type\n\
                                      1,20180507,1\n\
                                      1,20180505,2\n\
                                      2,20180506,2";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "calendar.txt", calendars_content);

            create_file_with_content(path, "calendar_dates.txt", calendar_dates_content);

            let mut collections = Collections::default();
            calendars::manage_calendars(&mut handler, &mut collections).unwrap();

            let mut dates = BTreeSet::new();
            dates.insert(chrono::NaiveDate::from_ymd_opt(2018, 5, 6).unwrap());
            dates.insert(chrono::NaiveDate::from_ymd_opt(2018, 5, 7).unwrap());
            assert_eq!(
                vec![
                    Calendar {
                        id: "1".to_string(),
                        dates,
                    },
                    Calendar {
                        id: "2".to_string(),
                        dates: BTreeSet::new(),
                    },
                ],
                collections.calendars.into_vec()
            );
        });
    }

    #[test]
    #[should_panic(expected = "calendar_dates.txt or calendar.txt not found")]
    fn gtfs_without_calendar_dates_or_calendar() {
        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            let mut collections = Collections::default();
            calendars::manage_calendars(&mut handler, &mut collections).unwrap();
        });
    }

    #[test]
    fn read_shapes() {
        let shapes_content = "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\n\
                              1,4.4,3.3,2\n\
                              1,2.2,1.1,1\n\
                              2,6.6,5.5,1\n\
                              2,,7.7,2\n\
                              2,8.8,,3\n\
                              2,,,4\n\
                              2,,,5\n\
                              3,,,1\n\
                              3,,,2";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "shapes.txt", shapes_content);

            let mut collections = Collections::default();
            super::manage_shapes(&mut collections, &mut handler).unwrap();
            let mut geometries = collections.geometries.into_vec();
            geometries.sort_unstable_by_key(|s| s.id.clone());

            assert_eq!(
                vec![
                    Geometry {
                        id: "1".to_string(),
                        geometry: line_string![(x: 1.1, y: 2.2), (x: 3.3, y: 4.4)].into(),
                    },
                    Geometry {
                        id: "2".to_string(),
                        geometry: line_string![(x: 5.5, y: 6.6)].into(),
                    },
                ],
                geometries
            );
        });
    }

    #[test]
    fn read_shapes_with_no_shapes_file() {
        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            let mut collections = Collections::default();
            super::manage_shapes(&mut collections, &mut handler).unwrap();
            let geometries = collections.geometries.into_vec();
            assert_eq!(Vec::<Geometry>::new(), geometries);
        });
    }

    #[test]
    fn deduplicate_funicular_physical_mode() {
        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_desc,route_type,route_url,route_color,route_text_color\n\
                                 route:1,agency:1,S1,S 1,,5,,ffea00,000000\n\
                                 route:2,agency:1,L2,L 2,,6,,ffea00,000000\n\
                                 route:3,agency:1,L3,L 3,,2,,ffea00,000000\n\
                                 route:4,agency:2,57,57,,7,,ffea00,000000";
        let trips_content = "route_id,service_id,trip_id,trip_headsign,direction_id,shape_id\n\
                             route:1,service:1,trip:1,pouet,0,\n\
                             route:2,service:1,trip:2,pouet,0,\n\
                             route:3,service:1,trip:3,pouet,0,\n\
                             route:4,service:1,trip:4,pouet,0,";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "routes.txt", routes_content);
            create_file_with_content(path, "trips.txt", trips_content);

            let mut collections = Collections::default();
            let (contributor, dataset, _) = read_config(None::<&str>).unwrap();
            collections.contributors = CollectionWithId::new(vec![contributor]).unwrap();
            collections.datasets = CollectionWithId::new(vec![dataset]).unwrap();

            super::read_routes(&mut handler, &mut collections, false, false).unwrap();
            // physical mode file should contain only three modes
            // (5,7 => funicular; 2 => train; 6 => suspended cable car)
            assert_eq!(4, collections.lines.len());
            assert_eq!(4, collections.commercial_modes.len());
            assert_eq!(
                vec!["Funicular", "SuspendedCableCar", "Train"],
                extract_ids(&collections.physical_modes)
            );
        });
    }

    #[test]
    fn location_type_default_value() {
        let stops_content = "stop_id,stop_name,stop_lat,stop_lon,location_type\n\
                             stop:1,Tornio pouet,65.843294,24.145138,";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "stops.txt", stops_content);
            let mut equipments = EquipmentList::default();
            let mut comments: CollectionWithId<Comment> = CollectionWithId::default();
            let (stop_areas, stop_points, _) =
                super::read_stops(&mut handler, &mut comments, &mut equipments).unwrap();
            assert_eq!(1, stop_points.len());
            assert_eq!(1, stop_areas.len());
            let stop_area = stop_areas.iter().next().unwrap().1;
            assert_eq!("Navitia:stop:1", stop_area.id);
            let stop_point = stop_points.iter().next().unwrap().1;
            assert_eq!("stop:1", stop_point.id);
        });
    }

    #[test]
    fn location_with_space_proof() {
        let stops_content = "stop_id,stop_name,stop_lat,stop_lon,location_type\n\
                             stop:1,plop, 65.444,24.156 ,0\n\
                             stop:2,plop 2, 66.666 , 26.123,0\n\
                             stop:3,invalid loc, ,25.558,0";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "stops.txt", stops_content);
            let mut equipments = EquipmentList::default();
            let mut comments: CollectionWithId<Comment> = CollectionWithId::default();
            let (_, stop_points, _) =
                super::read_stops(&mut handler, &mut comments, &mut equipments).unwrap();
            assert_eq!(3, stop_points.len());
            let longitudes: Vec<f64> = stop_points
                .values()
                .map(|sp| &sp.coord.lon)
                .cloned()
                .collect();
            assert_eq!(vec![24.156, 26.123, 25.558], longitudes);
            let latitudes: Vec<f64> = stop_points
                .values()
                .map(|sp| &sp.coord.lat)
                .cloned()
                .collect();
            assert_eq!(vec![65.444, 66.666, 0.00], latitudes);
        });
    }

    #[test]
    fn gtfs_undefined_stop_times() {
        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1,3,8F7A32,FFFFFF";

        let stops_content = r#"stop_id,stop_name,stop_desc,stop_lat,stop_lon,location_type,parent_station
             sp:01,my stop point name 1,my first desc,0.1,1.2,0,
             sp:02,my stop point name 2,my first desc,0.1,1.2,0,
             sp:03,my stop point name 3,my first desc,0.1,1.2,0,
             sp:04,my stop point name 4,my first desc,0.1,1.2,0,
             sp:05,my stop point name 5,my first desc,0.1,1.2,0,
             sp:06,my stop point name 6,my first desc,0.1,1.2,0,
             sp:07,my stop point name 7,my first desc,0.1,1.2,0,
             sp:08,my stop point name 8,my first desc,0.1,1.2,0,"#;

        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,0,service_1,,";

        let stop_times_content = "trip_id,arrival_time,departure_time,stop_id,stop_sequence,stop_headsign,pickup_type,drop_off_type,shape_dist_traveled\n\
                                  1,06:00:00,06:00:00,sp:01,1,,,,\n\
                                  1,07:00:00,07:00:00,sp:02,2,,,,\n\
                                  1,,,sp:03,3,,,,\n\
                                  1,,,sp:04,4,,,,\n\
                                  1,10:00:00,,sp:05,5,,,,\n\
                                  1,,,sp:06,6,,,,\n\
                                  1,,12:00:00,sp:07,7,,,,\n\
                                  1,13:00:00,13:00:00,sp:08,8,,,,\n\
                                  ";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "routes.txt", routes_content);
            create_file_with_content(path, "trips.txt", trips_content);
            create_file_with_content(path, "stop_times.txt", stop_times_content);
            create_file_with_content(path, "stops.txt", stops_content);

            let mut collections = Collections::default();
            let (contributor, dataset, _) = read_config(None::<&str>).unwrap();
            collections.contributors = CollectionWithId::new(vec![contributor]).unwrap();
            collections.datasets = CollectionWithId::new(vec![dataset]).unwrap();

            let mut comments: CollectionWithId<Comment> = CollectionWithId::default();
            let mut equipments = EquipmentList::default();
            let (_, stop_points, _) =
                super::read_stops(&mut handler, &mut comments, &mut equipments).unwrap();
            collections.stop_points = stop_points;

            super::read_routes(&mut handler, &mut collections, false, false).unwrap();
            let location_groups = HashMap::new();
            super::manage_stop_times(
                &mut collections,
                &mut handler,
                false,
                None,
                &location_groups,
            )
            .unwrap();

            assert_eq!(
                vec![
                    (Time::new(6, 0, 0), Time::new(6, 0, 0)),
                    (Time::new(7, 0, 0), Time::new(7, 0, 0)),
                    (Time::new(8, 0, 0), Time::new(8, 0, 0)),
                    (Time::new(9, 0, 0), Time::new(9, 0, 0)),
                    (Time::new(10, 0, 0), Time::new(10, 0, 0)),
                    (Time::new(11, 0, 0), Time::new(11, 0, 0)),
                    (Time::new(12, 0, 0), Time::new(12, 0, 0)),
                    (Time::new(13, 0, 0), Time::new(13, 0, 0)),
                ],
                collections.vehicle_journeys.into_vec()[0]
                    .stop_times
                    .iter()
                    .map(|st| (st.arrival_time.unwrap(), st.departure_time.unwrap()))
                    .collect::<Vec<_>>()
            );
        });
    }

    #[test]
    fn gtfs_invalid_undefined_stop_times() {
        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1,3,8F7A32,FFFFFF";

        let stops_content = r#"stop_id,stop_name,stop_desc,stop_lat,stop_lon,location_type,parent_station
             sp:01,my stop point name 1,my first desc,0.1,1.2,0,
             sp:02,my stop point name 2,my first desc,0.1,1.2,0,
             sp:03,my stop point name 3,my first desc,0.1,1.2,0,
             sp:04,my stop point name 4,my first desc,0.1,1.2,0,
             sp:05,my stop point name 5,my first desc,0.1,1.2,0,
             sp:06,my stop point name 6,my first desc,0.1,1.2,0,
             sp:07,my stop point name 7,my first desc,0.1,1.2,0,
             sp:08,my stop point name 8,my first desc,0.1,1.2,0,"#;

        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,0,service_1,,";

        let stop_times_content = "trip_id,arrival_time,departure_time,stop_id,stop_sequence,stop_headsign,pickup_type,drop_off_type,shape_dist_traveled\n\
                                  1,,,sp:01,1,,,,\n\
                                  1,07:00:00,07:00:00,sp:02,2,,,,\n\
                                  1,,,sp:03,3,,,,\n\
                                  1,,,sp:04,4,,,,\n\
                                  1,10:00:00,10:00:00,sp:05,5,,,,\n\
                                  1,,,sp:06,6,,,,\n\
                                  1,12:00:00,12:00:00,sp:07,7,,,,\n\
                                  1,13:00:00,13:00:00,sp:08,8,,,,\n\
                                  ";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "routes.txt", routes_content);
            create_file_with_content(path, "trips.txt", trips_content);
            create_file_with_content(path, "stop_times.txt", stop_times_content);
            create_file_with_content(path, "stops.txt", stops_content);

            let mut collections = Collections::default();
            let (contributor, dataset, _) = read_config(None::<&str>).unwrap();
            collections.contributors = CollectionWithId::new(vec![contributor]).unwrap();
            collections.datasets = CollectionWithId::new(vec![dataset]).unwrap();

            let mut comments: CollectionWithId<Comment> = CollectionWithId::default();
            let mut equipments = EquipmentList::default();
            let (_, stop_points, _) =
                super::read_stops(&mut handler, &mut comments, &mut equipments).unwrap();
            collections.stop_points = stop_points;

            super::read_routes(&mut handler, &mut collections, false, false).unwrap();
            let location_groups = HashMap::new();
            let val = super::manage_stop_times(
                &mut collections,
                &mut handler,
                false,
                None,
                &location_groups,
            );

            // the first stop time of the vj has no departure/arrival, it's an error
            let err = val.unwrap_err();
            assert_eq!( "the first stop time of the vj '1' has no departure/arrival, the stop_times.txt file is not valid",format!("{}", err));
        });
    }
    #[test]
    fn stop_location_on_stops() {
        let stops_content =
            "stop_id,stop_code,stop_name,stop_lat,stop_lon,location_type,parent_station\n\
             stoppoint_id,1234,my stop name,0.1,1.2,0,stop_area_id\n\
             stoparea_id,5678,stop area name,0.1,1.2,1,\n\
             entrance_id,,entrance name,0.1,1.2,2,stop_area_id\n\
             node_id,,node name,0.1,1.2,3,stop_area_id\n\
             boarding_id,,boarding name,0.1,1.2,4,stoppoint_id";
        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "stops.txt", stops_content);
            let mut equipments = EquipmentList::default();
            let mut comments: CollectionWithId<Comment> = CollectionWithId::default();
            let (_, _, stop_locations) =
                super::read_stops(&mut handler, &mut comments, &mut equipments).unwrap();
            let stop_entrance = stop_locations
                .values()
                .filter(|sl| sl.stop_type == StopType::StopEntrance);
            assert_eq!(1, stop_entrance.count());
            let stop_node = stop_locations
                .values()
                .filter(|sl| sl.stop_type == StopType::GenericNode);
            assert_eq!(1, stop_node.count());
            let stop_boarding = stop_locations
                .values()
                .filter(|sl| sl.stop_type == StopType::GenericNode);
            assert_eq!(1, stop_boarding.count());
        });
    }
    #[test]
    fn filter_pathway() {
        let stops_content =
            "stop_id,stop_code,stop_name,stop_lat,stop_lon,location_type,parent_station,level_id\n\
             stoppoint_id,1234,my stop name,0.1,1.2,0,stop_area_id,2\n\
             stoparea_id,5678,stop area name,0.1,1.2,1,,\n\
             entrance_id,,entrance name,0.1,1.2,2,stop_area_id,1\n\
             node_id,,node name,0.1,1.2,3,stop_area_id,2\n\
             boarding_id,,boarding name,0.1,1.2,4,stoppoint_id,";
        let pathway_content = "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\n\
                               1;stoppoint_id,stoparea_id,8,0\n\
                               2,stoppoint_id,stoparea_id,1,3\n\
                               3,stoppoint_id,stoparea_id_0,2,0\n\
                               4,stoppoint_id,stoparea_id,1,0
                               5,node_id,boarding_id,1,0";
        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "stops.txt", stops_content);
            create_file_with_content(path, "pathways.txt", pathway_content);
            let mut collections = Collections::default();
            let mut equipments = EquipmentList::default();
            let (_, stop_points, stop_locations) =
                super::read_stops(&mut handler, &mut collections.comments, &mut equipments)
                    .unwrap();
            collections.stop_points = stop_points;
            collections.stop_locations = stop_locations;

            super::manage_pathways(&mut collections, &mut handler).unwrap();
            assert_eq!(1, collections.pathways.len());
        })
    }
    #[test]
    fn read_levels() {
        let stops_content =
            "stop_id,stop_code,stop_name,stop_lat,stop_lon,location_type,parent_station,level_id\n\
             stoppoint_id,1234,my stop name,0.1,1.2,0,stop_area_id,2\n\
             stoparea_id,5678,stop area name,0.1,1.2,1,,\n\
             entrance_id,,entrance name,0.1,1.2,2,stop_area_id,1\n\
             node_id,,node name,0.1,1.2,3,stop_area_id,2\n\
             boarding_id,,boarding name,0.1,1.2,4,stoppoint_id,";
        let level_content = "level_id,level_index\n\
                             1,0\n\
                             2,2\n\
                             3,1\n\
                             4,4";
        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "stops.txt", stops_content);
            create_file_with_content(path, "levels.txt", level_content);
            let levels: CollectionWithId<Level> =
                read_opt_collection(&mut handler, "levels.txt").unwrap();
            assert_eq!(4, levels.len());
        })
    }
    #[test]
    fn gtfs_stop_times_precision() {
        let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                              route_1,agency_1,1,My line 1,3,8F7A32,FFFFFF";

        let stops_content =
            "stop_id,stop_name,stop_desc,stop_lat,stop_lon,location_type,parent_station\n\
             sp:01,my stop point name 1,my first desc,0.1,1.2,0,\n\
             sp:02,my stop point name 2,,0.2,1.5,0,\n\
             sp:03,my stop point name 2,,0.2,1.5,0,";

        let trips_content =
            "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
             1,route_1,0,service_1,,";

        let stop_times_content = "trip_id,arrival_time,departure_time,stop_id,stop_sequence,stop_headsign,pickup_type,drop_off_type,shape_dist_traveled,timepoint\n\
                                  1,06:00:00,06:00:00,sp:01,1,over there,,,,0\n\
                                  1,06:06:27,06:06:27,sp:02,2,,2,1,,1\n\
                                  1,06:06:27,06:06:27,sp:03,3,,2,1,,";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "routes.txt", routes_content);
            create_file_with_content(path, "trips.txt", trips_content);
            create_file_with_content(path, "stop_times.txt", stop_times_content);
            create_file_with_content(path, "stops.txt", stops_content);

            let mut collections = Collections::default();
            let (contributor, dataset, _) = read_config(None::<&str>).unwrap();
            collections.contributors = CollectionWithId::new(vec![contributor]).unwrap();
            collections.datasets = CollectionWithId::new(vec![dataset]).unwrap();

            let mut comments: CollectionWithId<Comment> = CollectionWithId::default();
            let mut equipments = EquipmentList::default();
            let (_, stop_points, _) =
                super::read_stops(&mut handler, &mut comments, &mut equipments).unwrap();
            collections.stop_points = stop_points;

            super::read_routes(&mut handler, &mut collections, false, false).unwrap();
            let location_groups = HashMap::new();
            super::manage_stop_times(&mut collections, &mut handler, true, None, &location_groups)
                .unwrap();

            assert_eq!(
                vec![
                    StopTime {
                        stop_point_idx: collections.stop_points.get_idx("sp:01").unwrap(),
                        sequence: 1,
                        arrival_time: Some(Time::new(6, 0, 0)),
                        departure_time: Some(Time::new(6, 0, 0)),
                        start_pickup_drop_off_window: None,
                        end_pickup_drop_off_window: None,
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 0,
                        drop_off_type: 0,
                        local_zone_id: None,
                        precision: Some(StopTimePrecision::Estimated),
                    },
                    StopTime {
                        stop_point_idx: collections.stop_points.get_idx("sp:02").unwrap(),
                        sequence: 2,
                        arrival_time: Some(Time::new(6, 6, 27)),
                        departure_time: Some(Time::new(6, 6, 27)),
                        start_pickup_drop_off_window: None,
                        end_pickup_drop_off_window: None,
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 2,
                        drop_off_type: 1,
                        local_zone_id: None,
                        precision: Some(StopTimePrecision::Exact),
                    },
                    StopTime {
                        stop_point_idx: collections.stop_points.get_idx("sp:03").unwrap(),
                        sequence: 3,
                        arrival_time: Some(Time::new(6, 6, 27)),
                        departure_time: Some(Time::new(6, 6, 27)),
                        start_pickup_drop_off_window: None,
                        end_pickup_drop_off_window: None,
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 2,
                        drop_off_type: 1,
                        local_zone_id: None,
                        precision: Some(StopTimePrecision::Exact),
                    },
                ],
                collections.vehicle_journeys.into_vec()[0].stop_times
            );
        });
    }

    mod read_gtfs_routes {
        use super::*;
        use crate::{file_handler::PathFileHandler, model::Collections};
        use pretty_assertions::assert_eq;
        use std::path;

        fn get_collection(path: &path::Path, read_as_line: bool) -> Collections {
            let agency_content = "agency_id,agency_name,agency_url,agency_timezone\n\
            id_agency,My agency,http://my-agency_url.com,Europe/London";

            let routes_content = "route_id,agency_id,route_short_name,route_long_name,route_type,route_color,route_text_color\n\
                        route_1,agency_1,1,My line 1A,3,8F7A32,FFFFFF\n\
                        route_2,agency_1,1,My line 1B,3,8F7A32,FFFFFF\n\
                        route_4,agency_2,1,My line 1B,3,8F7A32,FFFFFF\n\
                        route_3,agency_2,1,My line 1B,3,8F7A32,FFFFFF\n\
                        route_5,,1,My line 1C,3,8F7A32,FFFFFF";

            let trips_content =
                "trip_id,route_id,direction_id,service_id,wheelchair_accessible,bikes_allowed\n\
            1,route_1,0,service_1,,\n\
            2,route_2,0,service_1,,\n\
            3,route_3,0,service_2,,\n\
            4,route_4,0,service_2,,\n\
            5,route_5,0,service_2,,\n\
            6,route_5,1,service_2,,";
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "agency.txt", agency_content);
            create_file_with_content(path, "routes.txt", routes_content);
            create_file_with_content(path, "trips.txt", trips_content);
            let mut collections = Collections::default();
            let (networks, _) = super::read_agency(&mut handler).unwrap();
            collections.networks = networks;
            let (contributor, dataset, _) = read_config(None::<&str>).unwrap();
            collections.contributors = CollectionWithId::new(vec![contributor]).unwrap();
            collections.datasets = CollectionWithId::new(vec![dataset]).unwrap();
            super::read_routes(&mut handler, &mut collections, read_as_line, false).unwrap();
            collections
        }

        #[test]
        fn read_gtfs_routes_as_route() {
            test_in_tmp_dir(|path| {
                // read route as route
                let collections = get_collection(path, false);

                assert_eq!(3, collections.lines.len());
                assert_eq!(
                    vec!["agency_1", "agency_2", "id_agency"],
                    extract(|l| &l.network_id, &collections.lines)
                );
                assert_eq!(
                    vec!["route_1", "route_3", "route_5"],
                    extract_ids(&collections.lines)
                );
                assert_eq!(6, collections.routes.len());
                assert_eq!(
                    vec!["route_1", "route_1", "route_3", "route_3", "route_5", "route_5"],
                    extract(|r| &r.line_id, &collections.routes)
                );
                assert_eq!(
                    vec![
                        "route_1",
                        "route_2",
                        "route_3",
                        "route_4",
                        "route_5",
                        "route_5_R"
                    ],
                    extract(|r| &r.id, &collections.routes)
                );
            });
        }

        #[test]
        fn read_gtfs_routes_as_line() {
            test_in_tmp_dir(|path| {
                // read route as line
                let collections = get_collection(path, true);

                assert_eq!(5, collections.lines.len());
                assert_eq!(
                    vec!["agency_1", "agency_1", "agency_2", "agency_2", "id_agency"],
                    extract(|l| &l.network_id, &collections.lines)
                );
                assert_eq!(
                    vec!["route_1", "route_2", "route_3", "route_4", "route_5"],
                    extract_ids(&collections.lines)
                );
                assert_eq!(6, collections.routes.len());
                assert_eq!(
                    vec!["route_1", "route_2", "route_3", "route_4", "route_5", "route_5"],
                    extract(|r| &r.line_id, &collections.routes)
                );
                assert_eq!(
                    vec![
                        "route_1",
                        "route_2",
                        "route_3",
                        "route_4",
                        "route_5",
                        "route_5_R"
                    ],
                    extract(|r| &r.id, &collections.routes)
                );
            });
        }
    }

    #[test]
    fn test_group_stop_times_by_type() {
        // Test empty input
        assert_eq!(group_stop_times_by_type(&[]).len(), 0);

        // regular stop times
        let stop_time_1 = GtfsStopTime {
            trip_id: "1".into(),
            stop_sequence: 1,
            stop_id: Some("stop1".into()),
            arrival_time: Some(Time::new(8, 0, 0)),
            departure_time: Some(Time::new(8, 0, 0)),
            pickup_type: 0,
            drop_off_type: 0,
            timepoint: true,
            location_group_id: None,
            ..Default::default()
        };
        let stop_time_2 = GtfsStopTime {
            trip_id: "1".into(),
            stop_sequence: 1,
            stop_id: Some("stop1".into()),
            arrival_time: Some(Time::new(8, 10, 0)),
            departure_time: Some(Time::new(8, 10, 0)),
            pickup_type: 0,
            drop_off_type: 0,
            timepoint: true,
            ..Default::default()
        };

        //  zonal on-demand stop times
        let stop_time_3 = GtfsStopTime {
            trip_id: "1".into(),
            stop_sequence: 1,
            pickup_type: 2,
            drop_off_type: 1,
            timepoint: true,
            start_pickup_drop_off_window: Some(Time::new(9, 0, 0)),
            end_pickup_drop_off_window: Some(Time::new(20, 0, 0)),
            location_group_id: Some("zone1".into()),
            ..Default::default()
        };
        let stop_time_4 = GtfsStopTime {
            trip_id: "1".into(),
            stop_sequence: 1,
            pickup_type: 1,
            drop_off_type: 2,
            timepoint: true,
            start_pickup_drop_off_window: Some(Time::new(9, 0, 0)),
            end_pickup_drop_off_window: Some(Time::new(20, 0, 0)),
            location_group_id: Some("zone1".into()),
            ..Default::default()
        };

        // regular stop time
        let stop_time_5 = GtfsStopTime {
            trip_id: "1".into(),
            stop_sequence: 1,
            stop_id: Some("stop1".into()),
            arrival_time: Some(Time::new(8, 20, 0)),
            departure_time: Some(Time::new(8, 20, 0)),
            pickup_type: 0,
            drop_off_type: 0,
            timepoint: true,
            ..Default::default()
        };

        let stop_times = [
            stop_time_1,
            stop_time_2,
            stop_time_3,
            stop_time_4,
            stop_time_5,
        ];
        let result = group_stop_times_by_type(&stop_times);
        assert_eq!(result.len(), 3);

        match &result[0] {
            StopTimeType::NoPickupDropOffWindow(stops) => assert_eq!(stops.len(), 2),
            StopTimeType::WithPickupDropOffWindow(_) => {
                panic!("Expected stop times without pickup/drop-off window")
            }
        }

        match &result[1] {
            StopTimeType::NoPickupDropOffWindow(_) => {
                panic!("Expected stop times with pickup/drop-off window")
            }
            StopTimeType::WithPickupDropOffWindow(stops) => assert_eq!(stops.len(), 2),
        }

        match &result[2] {
            StopTimeType::NoPickupDropOffWindow(stops) => assert_eq!(stops.len(), 1),
            StopTimeType::WithPickupDropOffWindow(_) => {
                panic!("Expected stop times without pickup/drop-off window")
            }
        }
    }

    #[test]
    fn test_read_attributions_with_only_required_fields() {
        let attributions = "organization_name\n
             Organization A";

        test_in_tmp_dir(|path| {
            create_file_with_content(path, "attributions.txt", attributions);
            let mut handler = PathFileHandler::new(path.to_path_buf());

            let attributions: Vec<AttributionRule> =
                read_attributions(&mut handler, "attributions.txt").unwrap();
            assert_eq!(0, attributions.len());
        })
    }
}
