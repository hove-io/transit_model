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
    Agency, BookingRule, DirectionType, Route, RouteType, Shape, Stop, StopLocationType,
    TicketingDeepLinks, Transfer, Trip,
};
use crate::gtfs::{Attribution, ExtendedRoute, StopTime};
use crate::model::{GetCorresponding, Model};
use crate::objects;
use crate::objects::Transfer as NtfsTransfer;
use crate::objects::*;
use crate::Result;
use anyhow::Context;
use geo::Geometry as GeoGeometry;
use relational_types::IdxSet;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path;
use tracing::{info, warn};
use typed_index_collection::{Collection, CollectionWithId, Id, Idx};

pub fn write_transfers(path: &path::Path, transfers: &Collection<NtfsTransfer>) -> Result<()> {
    if transfers.is_empty() {
        return Ok(());
    }
    let file = "transfers.txt";
    info!(file_name = %file, "Writing");
    let path = path.join(file);
    let mut wtr =
        csv::Writer::from_path(&path).with_context(|| format!("Error reading {path:?}"))?;
    for t in transfers.values() {
        if t.from_stop_id != t.to_stop_id {
            wtr.serialize(Transfer::from(t))
                .with_context(|| format!("Error reading {path:?}"))?;
        }
    }

    wtr.flush()
        .with_context(|| format!("Error reading {path:?}"))?;

    Ok(())
}

pub fn write_agencies(
    path: &path::Path,
    networks: &CollectionWithId<objects::Network>,
    ticketing_deep_links: &TicketingDeepLinks,
) -> Result<()> {
    let file = "agency.txt";
    info!(file_name = %file, "Writing");
    let path = path.join(file);
    let mut wtr =
        csv::Writer::from_path(&path).with_context(|| format!("Error reading {path:?}"))?;
    for network in networks.values() {
        let mut agency = Agency::from(network);
        if !ticketing_deep_links.is_empty() {
            agency.ticketing_deep_link_id = network
                .fare_url
                .as_ref()
                .and_then(|fare_url| {
                    ticketing_deep_links
                        .get(fare_url)
                        .map(|ticketing_deep_link| ticketing_deep_link.id.clone())
                })
                .or_else(|| Some(String::new()));
            // If there is at least one ticketing_deep_link_id then the other csv columns cannot be set to None.
            // See struct Agency -> skip_serializing_if on ticketing_deep_link_id
            // Since the number of serialized columns must be the same, agencies without ticketing_deep_link_id must be set to empty string
        }
        wtr.serialize(agency)
            .with_context(|| format!("Error reading {path:?}"))?;
    }

    wtr.flush()
        .with_context(|| format!("Error reading {path:?}"))?;

    Ok(())
}

pub fn write_ticketing_deep_links(
    path: &path::Path,
    ticketing_deep_links: &TicketingDeepLinks,
) -> Result<()> {
    if !ticketing_deep_links.is_empty() {
        let file = "ticketing_deep_links.txt";
        info!(file_name = %file, "Writing");
        let path = path.join(file);
        let mut wtr =
            csv::Writer::from_path(&path).with_context(|| format!("Error reading {path:?}"))?;
        for tdl in ticketing_deep_links.values() {
            wtr.serialize(tdl)
                .with_context(|| format!("Error serializing {path:?}"))?;
        }
        wtr.flush()
            .with_context(|| format!("Error reading {path:?}"))?;
    }
    Ok(())
}

/// get the first comment ordered by name
fn get_first_comment_name<T: objects::Links<Comment>>(
    obj: &T,
    comments: &CollectionWithId<objects::Comment>,
) -> Option<String> {
    obj.links()
        .iter()
        .filter_map(|comment_id| comments.get(comment_id))
        .map(|cmt| &cmt.name)
        .min()
        .cloned()
}

fn ntfs_stop_point_to_gtfs_stop(
    sp: &objects::StopPoint,
    comments: &CollectionWithId<objects::Comment>,
    equipments: &CollectionWithId<objects::Equipment>,
) -> Stop {
    let wheelchair = sp
        .equipment_id
        .clone()
        .and_then(|eq_id| equipments.get(&eq_id))
        .map(|eq| eq.wheelchair_boarding)
        .unwrap_or_default();
    Stop {
        id: sp.id.clone(),
        name: sp.name.clone(),
        lat: sp.coord.lat.to_string(),
        lon: sp.coord.lon.to_string(),
        fare_zone_id: sp.fare_zone_id.clone(),
        location_type: StopLocationType::StopPoint,
        parent_station: Some(sp.stop_area_id.clone()),
        code: sp.code.clone(),
        desc: get_first_comment_name(sp, comments),
        wheelchair_boarding: wheelchair,
        url: None,
        timezone: sp.timezone,
        level_id: sp.level_id.clone(),
        platform_code: sp.platform_code.clone(),
    }
}

fn ntfs_stop_area_to_gtfs_stop(
    sa: &objects::StopArea,
    comments: &CollectionWithId<objects::Comment>,
    equipments: &CollectionWithId<objects::Equipment>,
) -> Stop {
    let wheelchair = sa
        .equipment_id
        .clone()
        .and_then(|eq_id| equipments.get(&eq_id))
        .map(|eq| eq.wheelchair_boarding)
        .unwrap_or_default();
    Stop {
        id: sa.id.clone(),
        name: sa.name.clone(),
        lat: sa.coord.lat.to_string(),
        lon: sa.coord.lon.to_string(),
        fare_zone_id: None,
        location_type: StopLocationType::StopArea,
        parent_station: None,
        code: None,
        desc: get_first_comment_name(sa, comments),
        wheelchair_boarding: wheelchair,
        url: None,
        timezone: sa.timezone,
        level_id: sa.level_id.clone(),
        platform_code: None,
    }
}

fn ntfs_stop_location_to_gtfs_stop(
    sl: &objects::StopLocation,
    comments: &CollectionWithId<objects::Comment>,
    equipments: &CollectionWithId<objects::Equipment>,
) -> Stop {
    let wheelchair = sl
        .equipment_id
        .clone()
        .and_then(|eq_id| equipments.get(&eq_id))
        .map(|eq| eq.wheelchair_boarding)
        .unwrap_or_default();

    let (lon, lat) = sl.coord.into();
    Stop {
        id: sl.id.clone(),
        name: sl.name.clone(),
        lat,
        lon,
        fare_zone_id: None,
        location_type: StopLocationType::from(sl.stop_type.clone()),
        parent_station: sl.parent_id.clone(),
        code: sl.code.clone(),
        desc: get_first_comment_name(sl, comments),
        wheelchair_boarding: wheelchair,
        url: None,
        timezone: sl.timezone,
        level_id: sl.level_id.clone(),
        platform_code: None,
    }
}

pub fn write_stops(
    path: &path::Path,
    stop_points: &CollectionWithId<objects::StopPoint>,
    stop_areas: &CollectionWithId<objects::StopArea>,
    stop_locations: &CollectionWithId<objects::StopLocation>,
    comments: &CollectionWithId<objects::Comment>,
    equipments: &CollectionWithId<objects::Equipment>,
) -> Result<()> {
    let file = "stops.txt";
    info!(file_name = %file, "Writing");
    let path = path.join(file);
    let mut wtr =
        csv::Writer::from_path(&path).with_context(|| format!("Error reading {path:?}"))?;
    info!("Writing {} from StopPoint", file);
    for sp in stop_points.values() {
        wtr.serialize(ntfs_stop_point_to_gtfs_stop(sp, comments, equipments))
            .with_context(|| format!("Error reading {path:?}"))?;
    }
    info!("Writing {} from StopArea", file);
    for sa in stop_areas.values() {
        wtr.serialize(ntfs_stop_area_to_gtfs_stop(sa, comments, equipments))
            .with_context(|| format!("Error reading {path:?}"))?;
    }
    info!("Writing {} from StopLocation", file);
    for sl in stop_locations.values() {
        wtr.serialize(ntfs_stop_location_to_gtfs_stop(sl, comments, equipments))
            .with_context(|| format!("Error reading {path:?}"))?;
    }

    wtr.flush()
        .with_context(|| format!("Error reading {path:?}"))?;

    Ok(())
}

fn get_gtfs_direction_id_from_ntfs_route(route: &objects::Route) -> DirectionType {
    match route.direction_type.as_deref() {
        Some("forward") | Some("clockwise") | Some("inbound") => DirectionType::Forward,
        _ => DirectionType::Backward,
    }
}

fn make_gtfs_trip_from_ntfs_vj(vj: &objects::VehicleJourney, model: &Model) -> Trip {
    let mut wheelchair_and_bike = (Availability::default(), Availability::default());
    if let Some(tp_id) = &vj.trip_property_id {
        if let Some(tp) = &model.trip_properties.get(tp_id) {
            wheelchair_and_bike = (tp.wheelchair_accessible, tp.bike_accepted);
        };
    }
    let route = &model.routes.get(&vj.route_id).unwrap();
    let line_idx = &model.lines.get_idx(&route.line_id).unwrap();
    let route_id = &get_line_physical_modes(*line_idx, &model.physical_modes, model)
        .into_iter()
        .find(|pmo| pmo.inner.id == vj.physical_mode_id)
        .map(|pm| get_gtfs_route_id_from_ntfs_line_id(&route.line_id, &pm))
        .unwrap();

    Trip {
        route_id: route_id.to_string(),
        service_id: vj.service_id.clone(),
        id: vj.id.clone(),
        headsign: vj.headsign.clone(),
        short_name: vj.short_name.clone(),
        direction: get_gtfs_direction_id_from_ntfs_route(route),
        block_id: vj.block_id.clone(),
        shape_id: vj.geometry_id.clone(),
        wheelchair_accessible: wheelchair_and_bike.0,
        bikes_allowed: wheelchair_and_bike.1,
    }
}

pub fn write_trips<'a>(
    path: &'a path::Path,
    model: &'a Model,
) -> Result<HashMap<String, Vec<&'a VehicleJourney>>> {
    let file = "trips.txt";
    info!(file_name = %file, "Writing");
    let path = path.join(file);
    let mut wtr =
        csv::Writer::from_path(&path).with_context(|| format!("Error reading {path:?}"))?;
    let mut vjs_by_route_gtfs_id: HashMap<String, Vec<&VehicleJourney>> = HashMap::new();
    for vj in model.vehicle_journeys.values() {
        let trip = make_gtfs_trip_from_ntfs_vj(vj, model);
        vjs_by_route_gtfs_id
            .entry(trip.route_id.clone())
            .or_default()
            .push(vj);

        wtr.serialize(trip)
            .with_context(|| format!("Error reading {path:?}"))?;
    }

    wtr.flush()
        .with_context(|| format!("Error reading {path:?}"))?;

    Ok(vjs_by_route_gtfs_id)
}

pub fn write_attributions(
    path: &path::Path,
    companies: &CollectionWithId<objects::Company>,
    gtfs_trips: HashMap<String, Vec<&VehicleJourney>>,
) -> Result<()> {
    let mut attributions: Vec<Attribution> = Vec::new();

    for (route_id, vjs) in gtfs_trips {
        let company_ids = vjs
            .iter()
            .map(|vj| vj.company_id.clone())
            .collect::<HashSet<_>>();
        if company_ids.len() == 1 {
            let company_id = company_ids.iter().next().expect("An error occurred");
            if let Some(company) = companies.get(company_id) {
                if company.role == objects::CompanyRole::Operator {
                    attributions.push(Attribution {
                        route_id: Some(route_id.clone()),
                        is_operator: Some(true),
                        organization_name: company.name.clone(),
                        attribution_url: company.url.clone(),
                        attribution_email: company.mail.clone(),
                        attribution_phone: company.phone.clone(),
                        ..Default::default()
                    });
                }
            }
        } else {
            for vj in vjs {
                if let Some(company) = companies.get(&vj.company_id) {
                    if company.role == objects::CompanyRole::Operator {
                        attributions.push(Attribution {
                            trip_id: Some(vj.id.clone()),
                            is_operator: Some(true),
                            organization_name: company.name.clone(),
                            attribution_url: company.url.clone(),
                            attribution_email: company.mail.clone(),
                            attribution_phone: company.phone.clone(),
                            ..Default::default()
                        });
                    }
                }
            }
        }
    }

    if !attributions.is_empty() {
        let file = "attributions.txt";
        info!(file_name = %file, "Writing file");
        let path = path.join(file);
        let mut wtr =
            csv::Writer::from_path(&path).with_context(|| format!("Error reading {path:?}"))?;
        for attribution in attributions {
            wtr.serialize(attribution)
                .with_context(|| format!("Error reading {path:?}"))?;
        }
        wtr.flush()
            .with_context(|| format!("Error reading {path:?}"))?;
    }

    Ok(())
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct StopExtension {
    #[serde(rename = "object_id")]
    id: String,
    #[serde(rename = "object_system")]
    name: String,
    #[serde(rename = "object_code")]
    code: String,
}

fn stop_extensions_from_collection_with_id<T>(
    collections: &CollectionWithId<T>,
) -> impl Iterator<Item = StopExtension> + '_
where
    T: Id<T> + Codes,
{
    collections
        .values()
        .flat_map(|obj| obj.codes().iter().map(move |c| (obj.id(), c)))
        .map(|(id, (name, code))| StopExtension {
            id: id.to_string(),
            name: name.to_string(),
            code: code.to_string(),
        })
}

pub fn write_stop_extensions(
    path: &path::Path,
    stop_points: &CollectionWithId<StopPoint>,
    stop_areas: &CollectionWithId<StopArea>,
) -> Result<()> {
    let mut stop_extensions = Vec::new();
    stop_extensions.extend(stop_extensions_from_collection_with_id(stop_points));
    stop_extensions.extend(stop_extensions_from_collection_with_id(stop_areas));
    if stop_extensions.is_empty() {
        return Ok(());
    }
    let file = "stop_extensions.txt";
    info!(file_name = %file, "Writing");

    let path = path.join(file);
    let mut wtr =
        csv::Writer::from_path(&path).with_context(|| format!("Error reading {path:?}"))?;
    for se in stop_extensions {
        wtr.serialize(se)
            .with_context(|| format!("Error reading {path:?}"))?;
    }
    wtr.flush()
        .with_context(|| format!("Error reading {path:?}"))?;

    Ok(())
}
#[derive(Debug)]
struct PhysicalModeWithOrder<'a> {
    inner: &'a objects::PhysicalMode,
    is_lowest: bool,
}

fn get_line_physical_modes<'a>(
    idx: Idx<objects::Line>,
    collection: &'a CollectionWithId<objects::PhysicalMode>,
    model: &Model,
) -> Vec<PhysicalModeWithOrder<'a>>
where
    IdxSet<objects::Line>: GetCorresponding<objects::PhysicalMode>,
{
    let mut pms: Vec<&objects::PhysicalMode> = model
        .get_corresponding_from_idx(idx)
        .into_iter()
        .map(move |idx| &collection[idx])
        .collect();
    pms.sort_unstable_by_key(|pm| get_physical_mode_order(pm));

    pms.iter()
        .enumerate()
        .map(|(i, pm)| PhysicalModeWithOrder {
            inner: pm,
            is_lowest: i == 0,
        })
        .collect()
}

impl From<&objects::PhysicalMode> for RouteType {
    fn from(obj: &objects::PhysicalMode) -> RouteType {
        match obj.id.as_str() {
            "RailShuttle" | "Tramway" => RouteType::Tramway,
            "Metro" => RouteType::Metro,
            "LocalTrain" | "LongDistanceTrain" | "RapidTransit" | "Train" => RouteType::Train,
            "Bus" | "BusRapidTransit" => RouteType::Bus,
            "Coach" => RouteType::Coach,
            "Boat" | "Ferry" => RouteType::Ferry,
            "Funicular" | "Shuttle" => RouteType::Funicular,
            "SuspendedCableCar" => RouteType::SuspendedCableCar,
            "Air" => RouteType::Air,
            "Taxi" => RouteType::Taxi,
            _ => RouteType::UnknownMode,
        }
    }
}

fn get_gtfs_route_id_from_ntfs_line_id(line_id: &str, pm: &PhysicalModeWithOrder<'_>) -> String {
    if pm.is_lowest {
        line_id.to_string()
    } else {
        line_id.to_string() + ":" + &pm.inner.id
    }
}

fn get_physical_mode_order(pm: &objects::PhysicalMode) -> u8 {
    match pm.id.as_str() {
        "Tramway" => 1,
        "RailShuttle" => 2,
        "Metro" => 3,
        "LocalTrain" => 4,
        "LongDistanceTrain" => 5,
        "RapidTransit" => 6,
        "Train" => 7,
        "BusRapidTransit" => 8,
        "Bus" => 9,
        "Coach" => 10,
        "Boat" => 11,
        "Ferry" => 12,
        "Funicular" => 13,
        "Shuttle" => 14,
        "SuspendedCableCar" => 15,
        "Air" => 16,
        "Taxi" => 17,
        _ => 18,
    }
}

fn make_gtfs_route_from_ntfs_line(line: &objects::Line, pm: &PhysicalModeWithOrder<'_>) -> Route {
    Route {
        id: get_gtfs_route_id_from_ntfs_line_id(&line.id, pm),
        agency_id: Some(line.network_id.clone()),
        short_name: line.code.clone().unwrap_or_default(),
        long_name: line.name.clone(),
        desc: None,
        route_type: RouteType::from(pm.inner),
        url: None,
        color: line.color.clone(),
        text_color: line.text_color.clone(),
        sort_order: line.sort_order,
    }
}

pub fn write_routes(path: &path::Path, model: &Model, extend_route_type: bool) -> Result<()> {
    let file = "routes.txt";
    info!(file_name = %file, "Writing");
    let path = path.join(file);
    let mut wtr =
        csv::Writer::from_path(&path).with_context(|| format!("Error reading {path:?}"))?;
    for (from, l) in &model.lines {
        for pm in &get_line_physical_modes(from, &model.physical_modes, model) {
            let route = make_gtfs_route_from_ntfs_line(l, pm);
            if extend_route_type {
                wtr.serialize(ExtendedRoute::from(route))
                    .with_context(|| format!("Error reading {path:?}"))?;
            } else {
                wtr.serialize(route)
                    .with_context(|| format!("Error reading {path:?}"))?;
            }
        }
    }

    wtr.flush()
        .with_context(|| format!("Error reading {path:?}"))?;

    Ok(())
}

pub fn write_stop_times(
    path: &path::Path,
    vehicle_journeys: &CollectionWithId<VehicleJourney>,
    stop_points: &CollectionWithId<StopPoint>,
    stop_times_headsigns: &HashMap<(String, u32), String>,
) -> Result<()> {
    let file = "stop_times.txt";
    info!(file_name = %file, "Writing");
    let stop_times_path = path.join(file);
    let mut st_wtr = csv::Writer::from_path(&stop_times_path)
        .with_context(|| format!("Error reading {stop_times_path:?}"))?;
    for (vj_idx, vj) in vehicle_journeys {
        for st in &vj.stop_times {
            // Notes :
            // 1 - In ntm, a vj can have n booking_rules. In gtfs it's only one. So we take the first one.
            // 2 - In ntm (for the moment), a booking_rule is on line or trip/vj, not stoptime.
            // So we apply the same booking_rule on all stoptimes if pickup_type or drop_off_type is =2 (odt).
            // Not all stoptimes need to have booking_rule, as some may be in regular service (pickup_type/drop_off_type 0).
            let booking_rule_id_opt = vj
                .booking_rule_links
                .first()
                .filter(|_| st.pickup_type == 2u8 || st.drop_off_type == 2u8)
                .cloned();
            st_wtr
                .serialize(StopTime {
                    stop_id: Some(stop_points[st.stop_point_idx].id.clone()),
                    location_group_id: None, // arbitrary value, this field is not serialized in GTFS yet
                    trip_id: vj.id.clone(),
                    stop_sequence: st.sequence,
                    arrival_time: st.arrival_time,
                    departure_time: st.departure_time,
                    start_pickup_drop_off_window: st.start_pickup_drop_off_window,
                    end_pickup_drop_off_window: st.end_pickup_drop_off_window,
                    pickup_type: st.pickup_type,
                    drop_off_type: st.drop_off_type,
                    local_zone_id: st.local_zone_id,
                    stop_headsign: stop_times_headsigns
                        .get(&(vehicle_journeys[vj_idx].id.clone(), st.sequence))
                        .cloned(),
                    timepoint: matches!(st.precision, None | Some(StopTimePrecision::Exact)),
                    pickup_booking_rule_id: booking_rule_id_opt.clone(),
                    drop_off_booking_rule_id: booking_rule_id_opt,
                })
                .with_context(|| format!("Error reading {st_wtr:?}"))?;
        }
    }
    st_wtr
        .flush()
        .with_context(|| format!("Error reading {stop_times_path:?}"))?;
    Ok(())
}

pub fn write_booking_rules(
    path: &path::Path,
    booking_rules: &CollectionWithId<objects::BookingRule>,
) -> Result<()> {
    if booking_rules.is_empty() {
        return Ok(());
    }
    let file = "booking_rules.txt";
    info!(file_name = %file, "Writing");
    let path = path.join(file);
    let mut wtr =
        csv::Writer::from_path(&path).with_context(|| format!("Error opening {path:?}"))?;
    for br in booking_rules.values() {
        wtr.serialize(BookingRule::from(br))
            .with_context(|| format!("Error writing {path:?}"))?;
    }
    wtr.flush()
        .with_context(|| format!("Error writing {path:?}"))?;
    Ok(())
}

fn ntfs_geometry_to_gtfs_shapes(g: &objects::Geometry) -> impl Iterator<Item = Shape> + '_ {
    let points = match g.geometry {
        GeoGeometry::LineString(ref linestring) => &linestring.0[..],
        _ => {
            warn!(
                "Geometry {} is not exported, only LINESTRING geometries are exported",
                g.id
            );
            &[]
        }
    };

    points.iter().enumerate().map(move |(i, p)| Shape {
        id: g.id.clone(),
        lat: p.y,
        lon: p.x,
        sequence: i as u32,
    })
}

pub fn write_shapes(
    path: &path::Path,
    geometries: &CollectionWithId<objects::Geometry>,
    vehicle_journeys: &CollectionWithId<VehicleJourney>,
) -> Result<()> {
    let mut used_geometries = HashSet::new();
    let shapes: Vec<_> = vehicle_journeys
        .values()
        .filter_map(|vj| vj.geometry_id.as_ref())
        .filter(|&geometry_id| used_geometries.insert(geometry_id))
        .filter_map(|geometry_id| geometries.get(geometry_id))
        .flat_map(ntfs_geometry_to_gtfs_shapes)
        .collect();
    if !shapes.is_empty() {
        let file = "shapes.txt";
        info!(file_name = %file, "Writing");
        let path = path.join(file);
        let mut wtr =
            csv::Writer::from_path(&path).with_context(|| format!("Error reading {path:?}"))?;
        wtr.flush()
            .with_context(|| format!("Error reading {path:?}"))?;
        for shape in shapes {
            wtr.serialize(shape)
                .with_context(|| format!("Error reading {path:?}"))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        calendars::write_calendar_dates,
        gtfs::{Route, RouteType, StopLocationType, Transfer, TransferType},
        model::Collections,
        objects::{Calendar, Coord, LinksT, StopPoint, StopTime, Transfer as NtfsTransfer},
    };
    use geo::{line_string, point};
    use pretty_assertions::assert_eq;
    use std::{collections::BTreeSet, fs::File, io::Read};
    use tempfile::tempdir;

    #[test]
    fn write_agency() {
        let agency = Agency::from(&objects::Network {
            id: "OIF:101".to_string(),
            name: "SAVAC".to_string(),
            url: Some("http://www.vianavigo.com".to_string()),
            timezone: Some(chrono_tz::Europe::Madrid),
            lang: Some("fr".to_string()),
            phone: Some("0123456789".to_string()),
            address: Some("somewhere".to_string()),
            fare_url: Some("http://www.vianavigo.com/tickets".to_string()),
            sort_order: Some(1),
            codes: Default::default(),
        });

        let expected_agency = Agency {
            id: Some("OIF:101".to_string()),
            name: "SAVAC".to_string(),
            url: "http://www.vianavigo.com".to_string(),
            timezone: chrono_tz::Europe::Madrid,
            lang: Some("fr".to_string()),
            phone: Some("0123456789".to_string()),
            email: None,
            fare_url: Some("http://www.vianavigo.com/tickets".to_string()),
            ticketing_deep_link_id: None,
        };

        assert_eq!(expected_agency, agency);
    }

    #[test]
    fn write_agency_with_default_values() {
        let agency = Agency::from(&objects::Network {
            id: "OIF:101".to_string(),
            name: "SAVAC".to_string(),
            url: None,
            timezone: None,
            lang: None,
            phone: None,
            address: None,
            fare_url: None,
            sort_order: None,
            codes: Default::default(),
        });

        let expected_agency = Agency {
            id: Some("OIF:101".to_string()),
            name: "SAVAC".to_string(),
            url: "http://www.navitia.io/".to_string(),
            timezone: chrono_tz::Europe::Paris,
            lang: None,
            phone: None,
            email: None,
            fare_url: None,
            ticketing_deep_link_id: None,
        };

        assert_eq!(expected_agency, agency);
    }

    #[test]
    fn test_ntfs_stop_point_to_gtfs_stop() {
        let comments = CollectionWithId::new(vec![
            objects::Comment {
                id: "1".into(),
                name: "foo".into(),
                comment_type: objects::CommentType::Information,
                url: None,
                label: None,
            },
            objects::Comment {
                id: "2".into(),
                name: "bar".into(),
                comment_type: objects::CommentType::Information,
                url: None,
                label: None,
            },
        ])
        .unwrap();

        let equipments = CollectionWithId::from(objects::Equipment {
            id: "1".to_string(),
            wheelchair_boarding: Availability::Available,
            sheltered: Availability::InformationNotAvailable,
            elevator: Availability::Available,
            escalator: Availability::Available,
            bike_accepted: Availability::Available,
            bike_depot: Availability::Available,
            visual_announcement: Availability::Available,
            audible_announcement: Availability::Available,
            appropriate_escort: Availability::Available,
            appropriate_signage: Availability::Available,
        });

        let mut comment_links = BTreeSet::new();
        comment_links.insert("1".to_string());
        comment_links.insert("2".to_string());

        let stop = objects::StopPoint {
            id: "sp_1".to_string(),
            name: "sp_name_1".to_string(),
            code: Some("1234".to_string()),
            codes: vec![
                ("object_system:2".to_string(), "object_code:2".to_string()),
                ("gtfs_stop_code".to_string(), "1234".to_string()),
                ("gtfs_stop_code".to_string(), "5678".to_string()),
            ]
            .into_iter()
            .collect(),
            comment_links,
            visible: true,
            coord: objects::Coord {
                lon: 2.073_034,
                lat: 48.799_115,
            },
            stop_area_id: "OIF:SA:8739322".to_string(),
            timezone: Some(chrono_tz::Europe::Paris),
            equipment_id: Some("1".to_string()),
            fare_zone_id: Some("1".to_string()),
            stop_type: StopType::Point,
            ..Default::default()
        };

        let expected = Stop {
            id: "sp_1".to_string(),
            name: "sp_name_1".to_string(),
            lat: 48.799_115.to_string(),
            lon: 2.073_034.to_string(),
            fare_zone_id: Some("1".to_string()),
            location_type: StopLocationType::StopPoint,
            parent_station: Some("OIF:SA:8739322".to_string()),
            code: Some("1234".to_string()),
            desc: Some("bar".to_string()),
            wheelchair_boarding: Availability::Available,
            url: None,
            timezone: Some(chrono_tz::Europe::Paris),
            level_id: None,
            platform_code: None,
        };

        assert_eq!(
            expected,
            ntfs_stop_point_to_gtfs_stop(&stop, &comments, &equipments)
        );
    }

    #[test]
    fn test_ntfs_minimal_stop_point_to_gtfs_stop() {
        let stop = objects::StopPoint {
            id: "sp_1".to_string(),
            name: "sp_name_1".to_string(),
            visible: true,
            coord: objects::Coord {
                lon: 2.073_034,
                lat: 48.799_115,
            },
            stop_area_id: "OIF:SA:8739322".to_string(),
            stop_type: StopType::Point,
            level_id: Some("level1".to_string()),
            ..Default::default()
        };

        let expected = Stop {
            id: "sp_1".to_string(),
            name: "sp_name_1".to_string(),
            lat: 48.799_115.to_string(),
            lon: 2.073_034.to_string(),
            fare_zone_id: None,
            location_type: StopLocationType::StopPoint,
            parent_station: Some("OIF:SA:8739322".to_string()),
            code: None,
            desc: None,
            wheelchair_boarding: Availability::InformationNotAvailable,
            url: None,
            timezone: None,
            level_id: Some("level1".to_string()),
            platform_code: None,
        };

        let comments = CollectionWithId::default();
        let equipments = CollectionWithId::default();
        assert_eq!(
            expected,
            ntfs_stop_point_to_gtfs_stop(&stop, &comments, &equipments)
        );
    }

    #[test]
    fn test_ntfs_stop_area_to_gtfs_stop() {
        let comments = CollectionWithId::new(vec![
            objects::Comment {
                id: "1".into(),
                name: "foo".into(),
                comment_type: objects::CommentType::Information,
                url: None,
                label: None,
            },
            objects::Comment {
                id: "2".into(),
                name: "bar".into(),
                comment_type: objects::CommentType::Information,
                url: None,
                label: None,
            },
        ])
        .unwrap();

        let equipments = CollectionWithId::from(objects::Equipment {
            id: "1".to_string(),
            wheelchair_boarding: Availability::NotAvailable,
            sheltered: Availability::InformationNotAvailable,
            elevator: Availability::Available,
            escalator: Availability::Available,
            bike_accepted: Availability::Available,
            bike_depot: Availability::Available,
            visual_announcement: Availability::Available,
            audible_announcement: Availability::Available,
            appropriate_escort: Availability::Available,
            appropriate_signage: Availability::Available,
        });

        let mut comment_links = BTreeSet::new();
        comment_links.insert("1".to_string());
        comment_links.insert("2".to_string());

        let stop = objects::StopArea {
            id: "sa_1".to_string(),
            name: "sa_name_1".to_string(),
            codes: vec![
                ("object_system:2".to_string(), "object_code:2".to_string()),
                ("gtfs_stop_code".to_string(), "5678".to_string()),
                ("gtfs_stop_code".to_string(), "1234".to_string()),
            ]
            .into_iter()
            .collect(),
            object_properties: PropertiesMap::default(),
            comment_links,
            visible: true,
            coord: objects::Coord {
                lon: 2.073_034,
                lat: 48.799_115,
            },
            timezone: Some(chrono_tz::Europe::Paris),
            geometry_id: None,
            equipment_id: Some("1".to_string()),
            level_id: None,
            address_id: None,
        };

        let expected = Stop {
            id: "sa_1".to_string(),
            name: "sa_name_1".to_string(),
            lat: 48.799_115.to_string(),
            lon: 2.073_034.to_string(),
            fare_zone_id: None,
            location_type: StopLocationType::StopArea,
            parent_station: None,
            code: None,
            desc: Some("bar".to_string()),
            wheelchair_boarding: Availability::NotAvailable,
            url: None,
            timezone: Some(chrono_tz::Europe::Paris),
            level_id: None,
            platform_code: None,
        };

        assert_eq!(
            expected,
            ntfs_stop_area_to_gtfs_stop(&stop, &comments, &equipments)
        );
    }

    #[test]
    fn write_trip() {
        let mut collections = Collections::default();
        collections
            .stop_points
            .push(objects::StopPoint {
                id: "OIF:SP:36:2085".to_string(),
                ..Default::default()
            })
            .unwrap();
        collections
            .stop_areas
            .push(objects::StopArea {
                ..Default::default()
            })
            .unwrap();
        collections
            .stop_points
            .push(objects::StopPoint {
                id: "OIF:SP:36:2127".to_string(),
                ..Default::default()
            })
            .unwrap();
        collections
            .networks
            .push(objects::Network {
                ..Default::default()
            })
            .unwrap();
        collections
            .commercial_modes
            .push(objects::CommercialMode {
                ..Default::default()
            })
            .unwrap();
        collections
            .lines
            .push(objects::Line {
                id: "OIF:002002002:BDEOIF829".to_string(),
                ..Default::default()
            })
            .unwrap();
        collections
            .routes
            .push(objects::Route {
                id: "OIF:078078001:1".to_string(),
                line_id: "OIF:002002002:BDEOIF829".to_string(),
                ..Default::default()
            })
            .unwrap();
        collections
            .trip_properties
            .push(objects::TripProperty {
                id: "1".to_string(),
                wheelchair_accessible: Availability::Available,
                bike_accepted: Availability::NotAvailable,
                air_conditioned: Availability::InformationNotAvailable,
                visual_announcement: Availability::Available,
                audible_announcement: Availability::Available,
                appropriate_escort: Availability::Available,
                appropriate_signage: Availability::Available,
                school_vehicle_type: objects::TransportType::Regular,
            })
            .unwrap();
        let mut dates = BTreeSet::new();
        dates.insert(chrono::NaiveDate::from_ymd_opt(2018, 5, 6).unwrap());
        collections
            .calendars
            .push(objects::Calendar {
                id: "2".to_string(),
                dates,
            })
            .unwrap();
        collections
            .physical_modes
            .push(objects::PhysicalMode {
                id: "Bus".to_string(),
                name: "Bus".to_string(),
                co2_emission: None,
            })
            .unwrap();
        collections
            .physical_modes
            .push(objects::PhysicalMode {
                id: "Coach".to_string(),
                name: "Coach".to_string(),
                co2_emission: None,
            })
            .unwrap();
        collections
            .contributors
            .push(objects::Contributor {
                ..Default::default()
            })
            .unwrap();
        collections
            .datasets
            .push(objects::Dataset {
                ..Default::default()
            })
            .unwrap();
        collections
            .companies
            .push(objects::Company {
                ..Default::default()
            })
            .unwrap();
        let vj = objects::VehicleJourney {
            id: "OIF:87604986-1_11595-1".to_string(),
            codes: BTreeSet::default(),
            object_properties: PropertiesMap::default(),
            comment_links: BTreeSet::default(),
            route_id: "OIF:078078001:1".to_string(),
            physical_mode_id: "Bus".to_string(),
            service_id: "2".to_string(),
            headsign: Some("2005".to_string()),
            short_name: Some("42".to_string()),
            block_id: Some("PLOI".to_string()),
            trip_property_id: Some("1".to_string()),
            geometry_id: Some("Geometry:Line:Relation:6883353".to_string()),
            stop_times: vec![
                objects::StopTime {
                    stop_point_idx: collections.stop_points.get_idx("OIF:SP:36:2085").unwrap(),
                    sequence: 0,
                    arrival_time: Some(objects::Time::new(14, 40, 0)),
                    departure_time: Some(objects::Time::new(14, 40, 0)),
                    start_pickup_drop_off_window: None,
                    end_pickup_drop_off_window: None,
                    boarding_duration: 0,
                    alighting_duration: 0,
                    pickup_type: 0,
                    drop_off_type: 1,
                    local_zone_id: None,
                    precision: None,
                },
                objects::StopTime {
                    stop_point_idx: collections.stop_points.get_idx("OIF:SP:36:2127").unwrap(),
                    sequence: 1,
                    arrival_time: Some(objects::Time::new(14, 42, 0)),
                    departure_time: Some(objects::Time::new(14, 42, 0)),
                    start_pickup_drop_off_window: None,
                    end_pickup_drop_off_window: None,
                    boarding_duration: 0,
                    alighting_duration: 0,
                    pickup_type: 0,
                    drop_off_type: 0,
                    local_zone_id: None,
                    precision: None,
                },
            ],
            journey_pattern_id: Some(String::from("OIF:JP:1")),
            ..Default::default()
        };
        collections.vehicle_journeys.push(vj.clone()).unwrap();
        let vj_coach = objects::VehicleJourney {
            id: "OIF:87604986-1_11595-1:Coach".to_string(),
            codes: BTreeSet::default(),
            object_properties: PropertiesMap::default(),
            comment_links: BTreeSet::default(),
            route_id: "OIF:078078001:1".to_string(),
            physical_mode_id: "Coach".to_string(),
            service_id: "2".to_string(),
            headsign: Some("2005".to_string()),
            short_name: Some("42".to_string()),
            block_id: Some("PLOI".to_string()),
            trip_property_id: Some("1".to_string()),
            geometry_id: Some("Geometry:Line:Relation:6883353".to_string()),
            stop_times: vec![
                objects::StopTime {
                    stop_point_idx: collections.stop_points.get_idx("OIF:SP:36:2085").unwrap(),
                    sequence: 0,
                    arrival_time: Some(objects::Time::new(14, 40, 0)),
                    departure_time: Some(objects::Time::new(14, 40, 0)),
                    start_pickup_drop_off_window: None,
                    end_pickup_drop_off_window: None,
                    boarding_duration: 0,
                    alighting_duration: 0,
                    pickup_type: 0,
                    drop_off_type: 1,
                    local_zone_id: None,
                    precision: None,
                },
                objects::StopTime {
                    stop_point_idx: collections.stop_points.get_idx("OIF:SP:36:2127").unwrap(),
                    sequence: 1,
                    arrival_time: Some(objects::Time::new(14, 42, 0)),
                    departure_time: Some(objects::Time::new(14, 42, 0)),
                    start_pickup_drop_off_window: None,
                    end_pickup_drop_off_window: None,
                    boarding_duration: 0,
                    alighting_duration: 0,
                    pickup_type: 0,
                    drop_off_type: 0,
                    local_zone_id: None,
                    precision: None,
                },
            ],
            journey_pattern_id: Some(String::from("OIF:JP:1")),
            ..Default::default()
        };
        collections.vehicle_journeys.push(vj_coach.clone()).unwrap();

        let mut expected = Trip {
            route_id: "OIF:002002002:BDEOIF829".to_string(),
            service_id: vj.service_id.clone(),
            id: "OIF:87604986-1_11595-1".to_string(),
            headsign: Some("2005".to_string()),
            short_name: Some("42".to_string()),
            direction: DirectionType::Forward,
            block_id: Some("PLOI".to_string()),
            shape_id: vj.geometry_id.clone(),
            wheelchair_accessible: Availability::Available,
            bikes_allowed: Availability::NotAvailable,
        };
        let model = Model::new(collections).unwrap();
        assert_eq!(expected, make_gtfs_trip_from_ntfs_vj(&vj, &model));

        expected.route_id = "OIF:002002002:BDEOIF829:Coach".to_string();
        expected.id = "OIF:87604986-1_11595-1:Coach".to_string();
        assert_eq!(expected, make_gtfs_trip_from_ntfs_vj(&vj_coach, &model));
    }

    #[test]
    fn ntfs_object_code_to_stop_extensions() {
        let mut sa_codes: BTreeSet<(String, String)> = BTreeSet::new();
        sa_codes.insert(("sa name 1".to_string(), "sa_code_1".to_string()));
        sa_codes.insert(("sa name 2".to_string(), "sa_code_2".to_string()));
        let stop_areas = CollectionWithId::from(StopArea {
            id: "sa:01".to_string(),
            name: "sa:01".to_string(),
            codes: sa_codes,
            object_properties: PropertiesMap::default(),
            comment_links: LinksT::default(),
            visible: true,
            coord: Coord {
                lon: 2.073,
                lat: 48.799,
            },
            timezone: None,
            geometry_id: None,
            level_id: Some("level0".to_string()),
            equipment_id: None,
            address_id: None,
        });
        let mut sp_codes: BTreeSet<(String, String)> = BTreeSet::new();
        sp_codes.insert(("sp name 1".to_string(), "sp_code_1".to_string()));
        sp_codes.insert(("sp name 2".to_string(), "sp_code_2".to_string()));
        sp_codes.insert(("sp name 3".to_string(), "sp_code_3".to_string()));
        let stop_points = CollectionWithId::from(StopPoint {
            id: "sp:01".to_string(),
            name: "sp:01".to_string(),
            codes: sp_codes,
            visible: true,
            coord: Coord {
                lon: 2.073,
                lat: 48.799,
            },
            stop_area_id: "sa:01".to_string(),
            stop_type: StopType::Point,
            ..Default::default()
        });
        let tmp_dir = tempdir().expect("create temp dir");
        write_stop_extensions(tmp_dir.path(), &stop_points, &stop_areas).unwrap();
        let output_file_path = tmp_dir.path().join("stop_extensions.txt");
        let mut output_file = File::open(output_file_path.clone())
            .unwrap_or_else(|_| panic!("file {:?} not found", output_file_path));
        let mut output_contents = String::new();
        output_file.read_to_string(&mut output_contents).unwrap();
        assert_eq!(
            "object_id,object_system,object_code\n\
             sp:01,sp name 1,sp_code_1\n\
             sp:01,sp name 2,sp_code_2\n\
             sp:01,sp name 3,sp_code_3\n\
             sa:01,sa name 1,sa_code_1\n\
             sa:01,sa name 2,sa_code_2\n",
            output_contents
        );
        tmp_dir.close().expect("delete temp dir");
    }

    #[test]
    fn ntfs_object_code_to_stop_extensions_nothing_generated() {
        let stop_areas = CollectionWithId::default();
        let stop_points = CollectionWithId::default();
        let tmp_dir = tempdir().expect("create temp dir");
        write_stop_extensions(tmp_dir.path(), &stop_points, &stop_areas).unwrap();
        let output_file_path = tmp_dir.path().join("stop_extensions.txt");
        assert!(!output_file_path.exists());
        tmp_dir.close().expect("delete temp dir");
    }

    #[test]
    fn ntfs_geometry_linestring_exported() {
        let geo = objects::Geometry {
            id: "1".to_string(),
            geometry: line_string![(x: 1.1, y: 2.2), (x: 3.3, y: 4.4)].into(),
        };

        let expected = vec![
            Shape {
                id: "1".to_string(),
                lon: 1.1,
                lat: 2.2,
                sequence: 0,
            },
            Shape {
                id: "1".to_string(),
                lon: 3.3,
                lat: 4.4,
                sequence: 1,
            },
        ];

        assert_eq!(
            expected,
            ntfs_geometry_to_gtfs_shapes(&geo).collect::<Vec<Shape>>()
        );
    }

    #[test]
    fn ntfs_geometry_not_linestring_not_exported() {
        let geo = objects::Geometry {
            id: "1".to_string(),
            geometry: point!(x: 1.1, y: 2.2).into(),
        };

        assert!(ntfs_geometry_to_gtfs_shapes(&geo).next().is_none());
    }

    #[test]
    fn ntfs_transfers_to_gtfs_transfers() {
        let transfer = Transfer::from(&NtfsTransfer {
            from_stop_id: "sp:01".to_string(),
            to_stop_id: "sp:02".to_string(),
            min_transfer_time: Some(42),
            real_min_transfer_time: None,
            equipment_id: None,
        });

        let expected = Transfer {
            from_stop_id: "sp:01".to_string(),
            to_stop_id: "sp:02".to_string(),
            transfer_type: TransferType::WithTransferTime,
            min_transfer_time: Some(42),
        };

        assert_eq!(expected, transfer);
    }

    #[test]
    fn write_calendar_file_from_calendar() {
        let mut dates = BTreeSet::new();
        //saturday
        dates.insert(chrono::NaiveDate::from_ymd_opt(2018, 5, 5).unwrap());
        //sunday
        dates.insert(chrono::NaiveDate::from_ymd_opt(2018, 5, 6).unwrap());
        let calendar = CollectionWithId::new(vec![
            Calendar {
                id: "1".to_string(),
                dates,
            },
            Calendar {
                id: "2".to_string(),
                dates: BTreeSet::new(),
            },
        ])
        .unwrap();
        let tmp_dir = tempdir().expect("create temp dir");
        write_calendar_dates(tmp_dir.path(), &calendar).unwrap();
        assert!(!tmp_dir.path().join("calendar_dates.txt").exists());

        let output_file_path = tmp_dir.path().join("calendar.txt");
        let mut output_file = File::open(output_file_path.clone())
            .unwrap_or_else(|_| panic!("file {:?} not found", output_file_path));
        let mut output_contents = String::new();
        output_file.read_to_string(&mut output_contents).unwrap();
        assert_eq!(
            "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
                1,0,0,0,0,0,1,1,20180505,20180506\n",
            output_contents
        );

        tmp_dir.close().expect("delete temp dir");
    }

    #[test]
    fn ntfs_vehicle_journeys_to_stop_times() {
        let stop_points = CollectionWithId::from(StopPoint {
            id: "sp:01".to_string(),
            name: "sp_name_1".to_string(),
            visible: true,
            coord: Coord {
                lon: 2.37,
                lat: 48.84,
            },
            stop_area_id: "sa_1".to_string(),
            stop_type: StopType::Point,
            ..Default::default()
        });
        let stop_times_vec = vec![
            StopTime {
                stop_point_idx: stop_points.get_idx("sp:01").unwrap(),
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
                precision: None,
            },
            StopTime {
                stop_point_idx: stop_points.get_idx("sp:01").unwrap(),
                sequence: 2,
                arrival_time: Some(Time::new(6, 6, 27)),
                departure_time: Some(Time::new(6, 6, 27)),
                start_pickup_drop_off_window: None,
                end_pickup_drop_off_window: None,
                boarding_duration: 0,
                alighting_duration: 0,
                pickup_type: 2,
                drop_off_type: 1,
                local_zone_id: Some(3),
                precision: Some(StopTimePrecision::Estimated),
            },
        ];
        let vehicle_journeys = CollectionWithId::from(VehicleJourney {
            id: "vj:01".to_string(),
            codes: BTreeSet::new(),
            object_properties: PropertiesMap::default(),
            comment_links: LinksT::default(),
            booking_rule_links: LinksT::default(),
            route_id: "r:01".to_string(),
            physical_mode_id: "pm:01".to_string(),
            dataset_id: "ds:01".to_string(),
            service_id: "sv:01".to_string(),
            headsign: None,
            short_name: None,
            block_id: None,
            company_id: "c:01".to_string(),
            trip_property_id: None,
            geometry_id: None,
            stop_times: stop_times_vec,
            journey_pattern_id: Some(String::from("jp:01")),
        });
        let mut stop_times_headsigns = HashMap::new();
        stop_times_headsigns.insert(("vj:01".to_string(), 1), "somewhere".to_string());
        let tmp_dir = tempdir().expect("create temp dir");
        write_stop_times(
            tmp_dir.path(),
            &vehicle_journeys,
            &stop_points,
            &stop_times_headsigns,
        )
        .unwrap();
        let output_file_path = tmp_dir.path().join("stop_times.txt");
        let mut output_file = File::open(output_file_path.clone())
            .unwrap_or_else(|_| panic!("file {:?} not found", output_file_path));
        let mut output_contents = String::new();
        output_file.read_to_string(&mut output_contents).unwrap();
        assert_eq!(
            "trip_id,arrival_time,departure_time,start_pickup_drop_off_window,end_pickup_drop_off_window,stop_id,stop_sequence,pickup_type,drop_off_type,local_zone_id,stop_headsign,timepoint,pickup_booking_rule_id,drop_off_booking_rule_id\n\
            vj:01,06:00:00,06:00:00,,,sp:01,1,0,0,,somewhere,1,,\n\
            vj:01,06:06:27,06:06:27,,,sp:01,2,2,1,3,,0,,\n",
            output_contents
        );
        tmp_dir.close().expect("delete temp dir");
    }

    #[test]
    fn ntfs_physical_mode_to_gtfs_route_type() {
        let route_type = RouteType::from(&objects::PhysicalMode {
            id: "Bus".to_string(),
            name: "Bus".to_string(),
            co2_emission: Some(6.2),
        });

        assert_eq!(RouteType::Bus, route_type);

        let route_type = RouteType::from(&objects::PhysicalMode {
            id: "Other".to_string(),
            name: "Other".to_string(),
            co2_emission: None,
        });

        assert_eq!(RouteType::UnknownMode, route_type);
    }

    #[test]
    fn ntfs_minial_line_to_gtfs_route() {
        let pm = PhysicalModeWithOrder {
            inner: &objects::PhysicalMode {
                id: "Bus".to_string(),
                name: "Bus".to_string(),
                co2_emission: Some(6.2),
            },
            is_lowest: true,
        };

        let line = objects::Line {
            id: "OIF:002002003:3OIF829".to_string(),
            name: "3".to_string(),
            code: None,
            codes: BTreeSet::default(),
            object_properties: PropertiesMap::default(),
            comment_links: LinksT::default(),
            booking_rule_links: LinksT::default(),
            forward_name: None,
            backward_name: None,
            color: None,
            text_color: None,
            sort_order: None,
            network_id: "OIF:829".to_string(),
            commercial_mode_id: "bus".to_string(),
            geometry_id: None,
            opening_time: None,
            closing_time: None,
        };

        let expected = Route {
            id: "OIF:002002003:3OIF829".to_string(),
            agency_id: Some("OIF:829".to_string()),
            short_name: "".to_string(),
            long_name: "3".to_string(),
            desc: None,
            route_type: RouteType::Bus,
            url: None,
            color: None,
            text_color: None,
            sort_order: None,
        };

        assert_eq!(expected, make_gtfs_route_from_ntfs_line(&line, &pm));
    }

    #[test]
    fn ntfs_line_with_unknown_mode_to_gtfs_route() {
        let pm = PhysicalModeWithOrder {
            inner: &objects::PhysicalMode {
                id: "Unknown".to_string(),
                name: "unknown".to_string(),
                co2_emission: Some(6.2),
            },
            is_lowest: false,
        };

        let line = objects::Line {
            id: "OIF:002002002:BDEOIF829".to_string(),
            name: "DEF".to_string(),
            code: Some("DEF".to_string()),
            codes: BTreeSet::default(),
            object_properties: PropertiesMap::default(),
            comment_links: LinksT::default(),
            booking_rule_links: LinksT::default(),
            forward_name: Some("Hôtels - Hôtels".to_string()),
            backward_name: Some("Hôtels - Hôtels".to_string()),
            color: Some(objects::Rgb {
                red: 155,
                green: 12,
                blue: 89,
            }),
            text_color: Some(objects::Rgb {
                red: 10,
                green: 0,
                blue: 45,
            }),
            sort_order: Some(1342),
            network_id: "OIF:829".to_string(),
            commercial_mode_id: "unknown".to_string(),
            geometry_id: Some("Geometry:Line:Relation:6883353".to_string()),
            opening_time: Some(objects::Time::new(9, 0, 0)),
            closing_time: Some(objects::Time::new(18, 0, 0)),
        };

        let expected = Route {
            id: "OIF:002002002:BDEOIF829:Unknown".to_string(),
            agency_id: Some("OIF:829".to_string()),
            short_name: "DEF".to_string(),
            long_name: "DEF".to_string(),
            desc: None,
            route_type: RouteType::UnknownMode,
            url: None,
            color: Some(objects::Rgb {
                red: 155,
                green: 12,
                blue: 89,
            }),
            text_color: Some(objects::Rgb {
                red: 10,
                green: 0,
                blue: 45,
            }),
            sort_order: Some(1342),
        };

        assert_eq!(expected, make_gtfs_route_from_ntfs_line(&line, &pm));
    }

    #[test]
    fn ntfs_tranfers_at_same_stop_point() {
        let tmp_dir = tempdir().expect("create temp dir");

        let transfers = Collection::new(vec![
            NtfsTransfer {
                from_stop_id: String::from("101937"),
                to_stop_id: String::from("101937"),
                min_transfer_time: None,
                real_min_transfer_time: None,
                equipment_id: None,
            },
            NtfsTransfer {
                from_stop_id: String::from("101938"),
                to_stop_id: String::from("101938"),
                min_transfer_time: None,
                real_min_transfer_time: None,
                equipment_id: None,
            },
            NtfsTransfer {
                from_stop_id: String::from("101937"),
                to_stop_id: String::from("101938"),
                min_transfer_time: None,
                real_min_transfer_time: None,
                equipment_id: None,
            },
            NtfsTransfer {
                from_stop_id: String::from("101938"),
                to_stop_id: String::from("101937"),
                min_transfer_time: None,
                real_min_transfer_time: None,
                equipment_id: None,
            },
        ]);

        write_transfers(tmp_dir.path(), &transfers).unwrap();
        let output_file_path = tmp_dir.path().join("transfers.txt");
        let mut output_file = File::open(output_file_path.clone())
            .unwrap_or_else(|_| panic!("file {:?} not found", output_file_path));
        let mut output_contents = String::new();
        output_file.read_to_string(&mut output_contents).unwrap();
        assert_eq!(
            "from_stop_id,to_stop_id,transfer_type,min_transfer_time\n\
            101937,101938,2,\n\
            101938,101937,2,\n",
            output_contents
        );
        tmp_dir.close().expect("delete temp dir");
    }
}
