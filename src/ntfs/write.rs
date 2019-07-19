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

use super::{Code, CommentLink, ObjectProperty, Result, Stop, StopTime};
use crate::collection::{Collection, CollectionWithId, Id, Idx};
use crate::model::Collections;
use crate::ntfs::{has_fares_v1, has_fares_v2};
use crate::objects::*;
use crate::NTFS_VERSION;
use chrono::{Duration, NaiveDateTime};
use csv;
use failure::{bail, format_err, ResultExt};
use log::{info, warn};
use rust_decimal::{prelude::ToPrimitive, Decimal};
use serde;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::convert::TryFrom;
use std::path;

impl TryFrom<(&Ticket, &TicketPrice)> for PriceV1 {
    type Error = failure::Error;
    fn try_from((ticket, price): (&Ticket, &TicketPrice)) -> Result<Self> {
        let cents_price = price.price * Decimal::from(100);
        let cents_price = cents_price
            .round_dp(0)
            .to_u32()
            .ok_or_else(|| format_err!("Cannot convert price {:?} into a u32", price.price))?;
        let comment = ticket.comment.as_ref().cloned().unwrap_or_else(String::new);
        let price_v1 = Self {
            id: ticket.id.clone(),
            start_date: price.ticket_validity_start,
            end_date: price.ticket_validity_end + Duration::days(1),
            price: cents_price,
            name: ticket.name.clone(),
            ignored: String::new(),
            comment,
            currency_type: Some("centime".to_string()),
        };
        Ok(price_v1)
    }
}

pub fn write_feed_infos(
    path: &path::Path,
    feed_infos: &BTreeMap<String, String>,
    datasets: &CollectionWithId<Dataset>,
    current_datetime: NaiveDateTime,
) -> Result<()> {
    info!("Writing feed_infos.txt");
    let path = path.join("feed_infos.txt");
    let mut feed_infos = feed_infos.clone();
    feed_infos.insert(
        "feed_creation_date".to_string(),
        current_datetime.format("%Y%m%d").to_string(),
    );
    feed_infos.insert(
        "feed_creation_time".to_string(),
        current_datetime.format("%T").to_string(),
    );
    feed_infos.insert("ntfs_version".to_string(), NTFS_VERSION.to_string());
    if let Some(d) = datasets.values().min_by_key(|d| d.start_date) {
        feed_infos.insert(
            "feed_start_date".to_string(),
            d.start_date.format("%Y%m%d").to_string(),
        );
    }
    if let Some(d) = datasets.values().max_by_key(|d| d.end_date) {
        feed_infos.insert(
            "feed_end_date".to_string(),
            d.end_date.format("%Y%m%d").to_string(),
        );
    }

    let mut wtr = csv::Writer::from_path(&path).with_context(ctx_from_path!(path))?;
    wtr.write_record(&["feed_info_param", "feed_info_value"])
        .with_context(ctx_from_path!(path))?;
    for feed_info in feed_infos {
        wtr.serialize(feed_info)
            .with_context(ctx_from_path!(path))?;
    }
    wtr.flush().with_context(ctx_from_path!(path))?;
    Ok(())
}

pub fn write_vehicle_journeys_and_stop_times(
    path: &path::Path,
    vehicle_journeys: &CollectionWithId<VehicleJourney>,
    stop_points: &CollectionWithId<StopPoint>,
    stop_time_headsigns: &HashMap<(Idx<VehicleJourney>, u32), String>,
    stop_time_ids: &HashMap<(Idx<VehicleJourney>, u32), String>,
) -> Result<()> {
    info!("Writing trips.txt and stop_times.txt");
    let trip_path = path.join("trips.txt");
    let stop_times_path = path.join("stop_times.txt");
    let mut vj_wtr = csv::Writer::from_path(&trip_path).with_context(ctx_from_path!(trip_path))?;
    let mut st_wtr =
        csv::Writer::from_path(&stop_times_path).with_context(ctx_from_path!(stop_times_path))?;
    for (vj_idx, vj) in vehicle_journeys.iter() {
        vj_wtr
            .serialize(vj)
            .with_context(ctx_from_path!(trip_path))?;

        for st in &vj.stop_times {
            st_wtr
                .serialize(StopTime {
                    stop_id: stop_points[st.stop_point_idx].id.clone(),
                    trip_id: vj.id.clone(),
                    stop_sequence: st.sequence,
                    arrival_time: st.arrival_time,
                    departure_time: st.departure_time,
                    boarding_duration: st.boarding_duration,
                    alighting_duration: st.alighting_duration,
                    pickup_type: st.pickup_type,
                    drop_off_type: st.drop_off_type,
                    datetime_estimated: Some(st.datetime_estimated as u8),
                    local_zone_id: st.local_zone_id,
                    stop_headsign: stop_time_headsigns.get(&(vj_idx, st.sequence)).cloned(),
                    stop_time_id: stop_time_ids.get(&(vj_idx, st.sequence)).cloned(),
                })
                .with_context(ctx_from_path!(st_wtr))?;
        }
    }
    st_wtr
        .flush()
        .with_context(ctx_from_path!(stop_times_path))?;
    vj_wtr.flush().with_context(ctx_from_path!(trip_path))?;

    Ok(())
}

fn do_write_fares_v1(
    base_path: &path::Path,
    prices_v1: &Collection<PriceV1>,
    od_fares_v1: &Collection<ODFareV1>,
    fares_v1: &Collection<FareV1>,
) -> Result<()> {
    let file_prices = "prices.csv";
    let file_od_fares = "od_fares.csv";
    let file_fares = "fares.csv";

    let mut builder = csv::WriterBuilder::new();
    builder.delimiter(b';');
    builder.has_headers(false);

    info!("Writing {}", file_prices);
    let path = base_path.join(file_prices);
    let mut prices_wtr = builder
        .from_path(&path)
        .with_context(ctx_from_path!(path))?;
    for price_v1 in prices_v1.values() {
        prices_wtr
            .serialize(price_v1)
            .with_context(ctx_from_path!(path))?;
    }
    prices_wtr.flush().with_context(ctx_from_path!(path))?;

    builder.has_headers(true);

    info!("Writing {}", file_od_fares);
    let path = base_path.join(file_od_fares);
    let mut od_fares_wtr = builder
        .from_path(&path)
        .with_context(ctx_from_path!(path))?;
    for od_fare_v1 in od_fares_v1.values() {
        od_fares_wtr
            .serialize(od_fare_v1)
            .with_context(ctx_from_path!(path))?;
    }
    // Write file header if collection is empty (normally done by serialize)
    if od_fares_v1.is_empty() {
        od_fares_wtr.write_record(&[
            "Origin ID",
            "Origin name",
            "Origin mode",
            "Destination ID",
            "Destination name",
            "Destination mode",
            "ticket_id",
        ])?;
    }
    od_fares_wtr.flush().with_context(ctx_from_path!(path))?;

    if fares_v1.is_empty() {
        info!("Writing skipped {}", file_fares);
        return Ok(());
    }

    info!("Writing {}", file_fares);
    let path = base_path.join(file_fares);
    let mut fares_wtr = builder
        .from_path(&path)
        .with_context(ctx_from_path!(path))?;
    for fare_v1 in fares_v1.values() {
        fares_wtr
            .serialize(fare_v1)
            .with_context(ctx_from_path!(path))?;
    }
    fares_wtr.flush().with_context(ctx_from_path!(path))?;

    Ok(())
}

struct Fares<'a> {
    tickets: &'a CollectionWithId<Ticket>,
    ticket_prices: &'a Collection<TicketPrice>,
    ticket_uses: &'a CollectionWithId<TicketUse>,
    ticket_use_perimeters: &'a Collection<TicketUsePerimeter>,
    ticket_use_restrictions: &'a Collection<TicketUseRestriction>,
}

fn has_constraints(ticket_use: &TicketUse) -> bool {
    ticket_use
        .max_transfers
        .filter(|&mt| mt != 0)
        .or(ticket_use.boarding_time_limit)
        .or(ticket_use.alighting_time_limit)
        .is_some()
}

fn get_prices<'a>(
    ticket_prices: &'a Collection<TicketPrice>,
    ticket_id: &str,
) -> Vec<&'a TicketPrice> {
    ticket_prices
        .values()
        .filter(|ticket_price| &ticket_price.ticket_id == ticket_id)
        .filter(|ticket_price| &ticket_price.currency == "EUR")
        .collect()
}

// Conversion of OD fares on specific lines
// https://github.com/CanalTP/transit_model/blob/master/src/documentation/ntfs_fare_conversion_v2_to_V1.md#conversion-of-an-od-fare-on-a-specific-line
fn insert_od_specific_line_as_fare_v1(
    fares: &Fares,
    prices_v1: &mut BTreeSet<PriceV1>,
    fares_v1: &mut BTreeSet<FareV1>,
) -> Result<()> {
    let ticket_use_restrictions = fares
        .ticket_use_restrictions
        .values()
        .filter(|ticket_use_restriction| {
            ticket_use_restriction.restriction_type == RestrictionType::OriginDestination
        })
        .filter(|ticket_use_restriction| {
            fares
                .ticket_uses
                .get(&ticket_use_restriction.ticket_use_id)
                .map(|ticket_use| !has_constraints(ticket_use))
                .unwrap_or(false)
        })
        .filter_map(|ticket_use_restriction| {
            fares
                .ticket_uses
                .get(&ticket_use_restriction.ticket_use_id)
                .and_then(|ticket_use| fares.tickets.get(&ticket_use.ticket_id))
                .map(|ticket| (ticket, ticket_use_restriction))
                .or_else(|| {
                    warn!(
                        "Failed to find Ticket for TicketUseId {:?}",
                        ticket_use_restriction.ticket_use_id
                    );
                    None
                })
        });

    for (ticket, ticket_use_restriction) in ticket_use_restrictions {
        let ticket_use_id = &ticket_use_restriction.ticket_use_id;

        let perimeters: Vec<&TicketUsePerimeter> = fares
            .ticket_use_perimeters
            .values()
            .filter(|ticket_use_perimeter| &ticket_use_perimeter.ticket_use_id == ticket_use_id)
            .filter(|ticket_use_perimeter| ticket_use_perimeter.object_type == ObjectType::Line)
            .filter(|ticket_use_perimeter| {
                ticket_use_perimeter.perimeter_action == PerimeterAction::Included
            })
            .collect();

        if perimeters.is_empty() {
            warn!(
                "Failed to find TicketUsePerimeter for TicketUse {:?}",
                ticket_use_id
            );
            continue;
        }

        let prices = get_prices(fares.ticket_prices, &ticket.id);
        if prices.is_empty() {
            warn!("Failed to find TicketPrice for Ticket {:?}", ticket.id);
            continue;
        }

        for price in prices {
            prices_v1.insert(PriceV1::try_from((ticket, price))?);
        }

        for perimeter in perimeters {
            fares_v1.insert(FareV1 {
                before_change: "*".to_string(),
                after_change: format!("line=line:{}", perimeter.object_id),
                start_trip: format!("stoparea=stop_area:{}", ticket_use_restriction.use_origin),
                end_trip: format!(
                    "stoparea=stop_area:{}",
                    ticket_use_restriction.use_destination
                ),
                global_condition: String::new(),
                ticket_id: ticket.id.clone(),
            });
        }
    }
    Ok(())
}

// Conversion of a flat fare on a specific network
// https://github.com/CanalTP/transit_model/blob/master/src/documentation/ntfs_fare_conversion_v2_to_V1.md#conversion-of-a-flat-fare-on-a-specific-network
fn insert_flat_fare_as_fare_v1(
    fares: &Fares,
    prices_v1: &mut BTreeSet<PriceV1>,
    fares_v1: &mut BTreeSet<FareV1>,
) -> Result<()> {
    let ticket_use_perimeters = fares
        .ticket_use_perimeters
        .values()
        .filter(|p| p.object_type == ObjectType::Network)
        .filter(|p| p.perimeter_action == PerimeterAction::Included)
        .filter_map(|ticket_use_perimeter| {
            fares
                .ticket_uses
                .get(&ticket_use_perimeter.ticket_use_id)
                .and_then(|ticket_use| fares.tickets.get(&ticket_use.ticket_id))
                .map(|ticket| (ticket, ticket_use_perimeter))
                .or_else(|| {
                    warn!(
                        "Failed to find Ticket for TicketUseId {:?}",
                        ticket_use_perimeter.ticket_use_id
                    );
                    None
                })
        });
    for (ticket, ticket_use_perimeter) in ticket_use_perimeters {
        let prices = get_prices(fares.ticket_prices, &ticket.id);
        if prices.is_empty() {
            warn!("Failed to find TicketPrice for Ticket {:?}", ticket.id);
            continue;
        }

        for price in prices {
            prices_v1.insert(PriceV1::try_from((ticket, price))?);
        }

        fares_v1.insert(FareV1 {
            before_change: "*".to_string(),
            after_change: format!("network=network:{}", ticket_use_perimeter.object_id),
            start_trip: String::new(),
            end_trip: String::new(),
            global_condition: String::new(),
            ticket_id: ticket.id.clone(),
        });
        fares_v1.insert(FareV1 {
            before_change: format!("network=network:{}", ticket_use_perimeter.object_id),
            after_change: format!("network=network:{}", ticket_use_perimeter.object_id),
            start_trip: String::new(),
            end_trip: String::new(),
            global_condition: String::new(),
            ticket_id: ticket.id.clone(),
        });
    }
    Ok(())
}

fn do_write_fares_v1_from_v2(base_path: &path::Path, fares: &Fares) -> Result<()> {
    let mut prices_v1: BTreeSet<PriceV1> = BTreeSet::new();
    let mut fares_v1: BTreeSet<FareV1> = BTreeSet::new();

    insert_od_specific_line_as_fare_v1(fares, &mut prices_v1, &mut fares_v1)?;
    insert_flat_fare_as_fare_v1(fares, &mut prices_v1, &mut fares_v1)?;

    if prices_v1.is_empty() || fares_v1.is_empty() {
        bail!("Cannot convert Fares V2 to V1. Prices or fares are empty.")
    }
    do_write_fares_v1(
        base_path,
        &Collection::new(prices_v1.into_iter().collect()),
        &Collection::new(vec![]),
        &Collection::new(fares_v1.into_iter().collect()),
    )
}

pub fn write_fares_v1(base_path: &path::Path, collections: &Collections) -> Result<()> {
    if has_fares_v2(collections) {
        return do_write_fares_v1_from_v2(
            base_path,
            &Fares {
                tickets: &collections.tickets,
                ticket_prices: &collections.ticket_prices,
                ticket_uses: &collections.ticket_uses,
                ticket_use_perimeters: &collections.ticket_use_perimeters,
                ticket_use_restrictions: &collections.ticket_use_restrictions,
            },
        );
    }
    if has_fares_v1(collections) {
        return do_write_fares_v1(
            base_path,
            &collections.prices_v1,
            &collections.od_fares_v1,
            &collections.fares_v1,
        );
    }
    Ok(())
}

pub fn write_collection_with_id<T>(
    path: &path::Path,
    file: &str,
    collection: &CollectionWithId<T>,
) -> Result<()>
where
    T: Id<T>,
    T: serde::Serialize,
{
    if collection.is_empty() {
        return Ok(());
    }
    info!("Writing {}", file);
    let path = path.join(file);
    let mut wtr = csv::Writer::from_path(&path).with_context(ctx_from_path!(path))?;
    for obj in collection.values() {
        wtr.serialize(obj).with_context(ctx_from_path!(path))?;
    }
    wtr.flush().with_context(ctx_from_path!(path))?;

    Ok(())
}

pub fn write_collection<T>(path: &path::Path, file: &str, collection: &Collection<T>) -> Result<()>
where
    T: serde::Serialize,
{
    if collection.is_empty() {
        return Ok(());
    }
    info!("Writing {}", file);
    let path = path.join(file);
    let mut wtr = csv::Writer::from_path(&path).with_context(ctx_from_path!(path))?;
    for obj in collection.values() {
        wtr.serialize(obj).with_context(ctx_from_path!(path))?;
    }
    wtr.flush().with_context(ctx_from_path!(path))?;

    Ok(())
}

pub fn write_stops(
    path: &path::Path,
    stop_points: &CollectionWithId<StopPoint>,
    stop_areas: &CollectionWithId<StopArea>,
) -> Result<()> {
    info!("Writing stops.txt");
    let path = path.join("stops.txt");
    let mut wtr = csv::Writer::from_path(&path).with_context(ctx_from_path!(path))?;
    for st in stop_points.values() {
        let location_type = match st.stop_type {
            StopType::Point => 0,
            StopType::Zone => 2,
        };
        wtr.serialize(Stop {
            id: st.id.clone(),
            visible: st.visible,
            name: st.name.clone(),
            lat: st.coord.lat,
            lon: st.coord.lon,
            fare_zone_id: st.fare_zone_id.clone(),
            location_type,
            parent_station: stop_areas.get(&st.stop_area_id).map(|sa| sa.id.clone()),
            timezone: st.timezone.clone(),
            equipment_id: st.equipment_id.clone(),
            geometry_id: st.geometry_id.clone(),
            platform_code: st.platform_code.clone(),
        })
        .with_context(ctx_from_path!(path))?;
    }

    for sa in stop_areas.values() {
        wtr.serialize(Stop {
            id: sa.id.clone(),
            visible: sa.visible,
            name: sa.name.clone(),
            lat: sa.coord.lat,
            lon: sa.coord.lon,
            fare_zone_id: None,
            location_type: 1,
            parent_station: None,
            timezone: sa.timezone.clone(),
            equipment_id: sa.equipment_id.clone(),
            geometry_id: sa.geometry_id.clone(),
            platform_code: None,
        })
        .with_context(ctx_from_path!(path))?;
    }
    wtr.flush().with_context(ctx_from_path!(path))?;

    Ok(())
}

fn write_comment_links_from_collection_with_id<W, T>(
    wtr: &mut csv::Writer<W>,
    collection: &CollectionWithId<T>,
    comments: &CollectionWithId<Comment>,
    path: &path::Path,
) -> Result<()>
where
    T: Id<T> + CommentLinks + GetObjectType,
    W: ::std::io::Write,
{
    for obj in collection.values() {
        for comment in comments.iter_from(obj.comment_links()) {
            wtr.serialize(CommentLink {
                object_id: obj.id().to_string(),
                object_type: T::get_object_type(),
                comment_id: comment.id.to_string(),
            })
            .with_context(ctx_from_path!(path))?;
        }
    }
    Ok(())
}

fn write_stop_time_comment_links<W>(
    wtr: &mut csv::Writer<W>,
    stop_time_ids: &HashMap<(Idx<VehicleJourney>, u32), String>,
    stop_time_comments: &HashMap<(Idx<VehicleJourney>, u32), Idx<Comment>>,
    comments: &CollectionWithId<Comment>,
    path: &path::Path,
) -> Result<()>
where
    W: ::std::io::Write,
{
    for (idx_sequence, idx_comment) in stop_time_comments {
        let comment = &comments[*idx_comment];
        let st_id = &stop_time_ids[idx_sequence];

        wtr.serialize(CommentLink {
            object_id: st_id.to_string(),
            object_type: ObjectType::StopTime,
            comment_id: comment.id.to_string(),
        })
        .with_context(ctx_from_path!(path))?;
    }

    Ok(())
}

pub fn write_comments(path: &path::Path, collections: &Collections) -> Result<()> {
    if collections.comments.is_empty() {
        return Ok(());
    }
    info!("Writing comments.txt and comment_links.txt");

    let comments_path = path.join("comments.txt");
    let comment_links_path = path.join("comment_links.txt");

    let mut c_wtr =
        csv::Writer::from_path(&comments_path).with_context(ctx_from_path!(comments_path))?;
    let mut cl_wtr = csv::Writer::from_path(&comment_links_path)
        .with_context(ctx_from_path!(comment_links_path))?;
    for c in collections.comments.values() {
        c_wtr
            .serialize(c)
            .with_context(ctx_from_path!(comments_path))?;
    }

    write_comment_links_from_collection_with_id(
        &mut cl_wtr,
        &collections.stop_areas,
        &collections.comments,
        &comment_links_path,
    )?;
    write_comment_links_from_collection_with_id(
        &mut cl_wtr,
        &collections.stop_points,
        &collections.comments,
        &comment_links_path,
    )?;
    write_comment_links_from_collection_with_id(
        &mut cl_wtr,
        &collections.lines,
        &collections.comments,
        &comment_links_path,
    )?;
    write_comment_links_from_collection_with_id(
        &mut cl_wtr,
        &collections.routes,
        &collections.comments,
        &comment_links_path,
    )?;
    write_comment_links_from_collection_with_id(
        &mut cl_wtr,
        &collections.vehicle_journeys,
        &collections.comments,
        &comment_links_path,
    )?;

    write_stop_time_comment_links(
        &mut cl_wtr,
        &collections.stop_time_ids,
        &collections.stop_time_comments,
        &collections.comments,
        &comment_links_path,
    )?;

    // TODO: add line_groups

    cl_wtr
        .flush()
        .with_context(ctx_from_path!(comment_links_path))?;
    c_wtr.flush().with_context(ctx_from_path!(comments_path))?;

    Ok(())
}

fn write_codes_from_collection_with_id<W, T>(
    wtr: &mut csv::Writer<W>,
    collections: &CollectionWithId<T>,
    path: &path::Path,
) -> Result<()>
where
    T: Id<T> + Codes + GetObjectType,
    W: ::std::io::Write,
{
    for obj in collections.values() {
        for c in obj.codes() {
            wtr.serialize(Code {
                object_id: obj.id().to_string(),
                object_type: T::get_object_type(),
                object_system: c.0.clone(),
                object_code: c.1.clone(),
            })
            .with_context(ctx_from_path!(path))?;
        }
    }

    Ok(())
}

pub fn write_codes(path: &path::Path, collections: &Collections) -> Result<()> {
    fn collection_has_no_codes<T: Codes>(collection: &CollectionWithId<T>) -> bool {
        collection.values().all(|c| c.codes().is_empty())
    }
    if collection_has_no_codes(&collections.stop_areas)
        && collection_has_no_codes(&collections.stop_points)
        && collection_has_no_codes(&collections.networks)
        && collection_has_no_codes(&collections.lines)
        && collection_has_no_codes(&collections.routes)
        && collection_has_no_codes(&collections.vehicle_journeys)
    {
        return Ok(());
    }

    info!("Writing object_codes.txt");

    let path = path.join("object_codes.txt");

    let mut wtr = csv::Writer::from_path(&path).with_context(ctx_from_path!(path))?;
    write_codes_from_collection_with_id(&mut wtr, &collections.stop_areas, &path)?;
    write_codes_from_collection_with_id(&mut wtr, &collections.stop_points, &path)?;
    write_codes_from_collection_with_id(&mut wtr, &collections.networks, &path)?;
    write_codes_from_collection_with_id(&mut wtr, &collections.lines, &path)?;
    write_codes_from_collection_with_id(&mut wtr, &collections.routes, &path)?;
    write_codes_from_collection_with_id(&mut wtr, &collections.vehicle_journeys, &path)?;

    wtr.flush().with_context(ctx_from_path!(path))?;

    Ok(())
}

fn write_object_properties_from_collection_with_id<W, T>(
    wtr: &mut csv::Writer<W>,
    collection: &CollectionWithId<T>,
    path: &path::Path,
) -> Result<()>
where
    T: Id<T> + Properties + GetObjectType,
    W: ::std::io::Write,
{
    for obj in collection.values() {
        for c in obj.properties() {
            wtr.serialize(ObjectProperty {
                object_id: obj.id().to_string(),
                object_type: T::get_object_type(),
                object_property_name: c.0.clone(),
                object_property_value: c.1.clone(),
            })
            .with_context(ctx_from_path!(path))?;
        }
    }

    Ok(())
}

pub fn write_object_properties(path: &path::Path, collections: &Collections) -> Result<()> {
    fn collection_has_no_object_properties<T: Properties>(
        collection: &CollectionWithId<T>,
    ) -> bool {
        collection.values().all(|c| c.properties().is_empty())
    }
    if collection_has_no_object_properties(&collections.stop_areas)
        && collection_has_no_object_properties(&collections.stop_points)
        && collection_has_no_object_properties(&collections.lines)
        && collection_has_no_object_properties(&collections.routes)
        && collection_has_no_object_properties(&collections.vehicle_journeys)
    {
        return Ok(());
    }

    info!("Writing object_properties.txt");

    let path = path.join("object_properties.txt");

    let mut wtr = csv::Writer::from_path(&path).with_context(ctx_from_path!(path))?;
    write_object_properties_from_collection_with_id(&mut wtr, &collections.stop_areas, &path)?;
    write_object_properties_from_collection_with_id(&mut wtr, &collections.stop_points, &path)?;
    write_object_properties_from_collection_with_id(&mut wtr, &collections.lines, &path)?;
    write_object_properties_from_collection_with_id(&mut wtr, &collections.routes, &path)?;
    write_object_properties_from_collection_with_id(
        &mut wtr,
        &collections.vehicle_journeys,
        &path,
    )?;

    wtr.flush().with_context(ctx_from_path!(path))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    mod has_constraints {
        use super::*;
        use pretty_assertions::assert_eq;
        use std::default::Default;

        impl Default for TicketUse {
            fn default() -> Self {
                TicketUse {
                    id: String::from("ticket_use_id"),
                    ticket_id: String::from("ticket_id"),
                    max_transfers: None,
                    boarding_time_limit: None,
                    alighting_time_limit: None,
                }
            }
        }

        #[test]
        fn no_constraints() {
            let ticket_use = TicketUse::default();
            assert_eq!(has_constraints(&ticket_use), false);
        }

        #[test]
        fn no_constraints_with_zero_transfers() {
            let mut ticket_use = TicketUse::default();
            ticket_use.max_transfers = Some(0);
            assert_eq!(has_constraints(&ticket_use), false);
        }

        #[test]
        fn transfer_constraint() {
            let mut ticket_use = TicketUse::default();
            ticket_use.max_transfers = Some(1);
            assert_eq!(has_constraints(&ticket_use), true);
        }

        #[test]
        fn boarding_constraint() {
            let mut ticket_use = TicketUse::default();
            ticket_use.boarding_time_limit = Some(666);
            assert_eq!(has_constraints(&ticket_use), true);
        }
    }
}
