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

use super::{Code, CommentLink, ObjectProperty, Result, Stop, StopLocationType, StopTime};
use crate::model::Collections;
use crate::ntfs::{has_fares_v1, has_fares_v2};
use crate::objects::*;
use crate::NTFS_VERSION;
use chrono::{DateTime, Duration, FixedOffset};
use csv;
use csv::Writer;
use failure::{bail, format_err, ResultExt};
use log::{info, warn};
use rust_decimal::{prelude::ToPrimitive, Decimal};
use std::collections::{BTreeSet, HashMap};
use std::convert::TryFrom;
use std::fs::File;
use std::path;
use transit_model_collection::{Collection, CollectionWithId, Id, Idx};

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
    collections: &Collections,
    current_datetime: DateTime<FixedOffset>,
) -> Result<()> {
    info!("Writing feed_infos.txt");
    let path = path.join("feed_infos.txt");
    let mut feed_infos = collections.feed_infos.clone();
    feed_infos.insert(
        "feed_creation_date".to_string(),
        current_datetime.format("%Y%m%d").to_string(),
    );
    feed_infos.insert(
        "feed_creation_time".to_string(),
        current_datetime.format("%T").to_string(),
    );
    feed_infos.insert(
        "feed_creation_datetime".to_string(),
        current_datetime.to_rfc3339(),
    );
    feed_infos.insert("ntfs_version".to_string(), NTFS_VERSION.to_string());
    let (start_date, end_date) = collections.calculate_validity_period()?;
    feed_infos.insert(
        "feed_start_date".to_string(),
        start_date.format("%Y%m%d").to_string(),
    );
    feed_infos.insert(
        "feed_end_date".to_string(),
        end_date.format("%Y%m%d").to_string(),
    );

    let mut wtr =
        csv::Writer::from_path(&path).with_context(|_| format!("Error reading {:?}", path))?;
    wtr.write_record(&["feed_info_param", "feed_info_value"])
        .with_context(|_| format!("Error reading {:?}", path))?;
    for feed_info in feed_infos {
        wtr.serialize(feed_info)
            .with_context(|_| format!("Error reading {:?}", path))?;
    }
    wtr.flush()
        .with_context(|_| format!("Error reading {:?}", path))?;
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
    let mut vj_wtr = csv::Writer::from_path(&trip_path)
        .with_context(|_| format!("Error reading {:?}", trip_path))?;
    let mut st_wtr = csv::Writer::from_path(&stop_times_path)
        .with_context(|_| format!("Error reading {:?}", stop_times_path))?;
    for (vj_idx, vj) in vehicle_journeys.iter() {
        vj_wtr
            .serialize(vj)
            .with_context(|_| format!("Error reading {:?}", trip_path))?;

        for st in &vj.stop_times {
            let precision = st.precision.clone().or_else(|| {
                if st.datetime_estimated {
                    Some(StopTimePrecision::Estimated)
                } else {
                    Some(StopTimePrecision::Exact)
                }
            });
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
                    precision,
                })
                .with_context(|_| format!("Error reading {:?}", st_wtr))?;
        }
    }
    st_wtr
        .flush()
        .with_context(|_| format!("Error reading {:?}", stop_times_path))?;
    vj_wtr
        .flush()
        .with_context(|_| format!("Error reading {:?}", trip_path))?;

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
        .with_context(|_| format!("Error reading {:?}", path))?;
    for price_v1 in prices_v1.values() {
        prices_wtr
            .serialize(price_v1)
            .with_context(|_| format!("Error reading {:?}", path))?;
    }
    prices_wtr
        .flush()
        .with_context(|_| format!("Error reading {:?}", path))?;

    builder.has_headers(true);

    info!("Writing {}", file_od_fares);
    let path = base_path.join(file_od_fares);
    let mut od_fares_wtr = builder
        .from_path(&path)
        .with_context(|_| format!("Error reading {:?}", path))?;
    for od_fare_v1 in od_fares_v1.values() {
        od_fares_wtr
            .serialize(od_fare_v1)
            .with_context(|_| format!("Error reading {:?}", path))?;
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
    od_fares_wtr
        .flush()
        .with_context(|_| format!("Error reading {:?}", path))?;

    if fares_v1.is_empty() {
        info!("Writing skipped {}", file_fares);
        return Ok(());
    }

    info!("Writing {}", file_fares);
    let path = base_path.join(file_fares);
    let mut fares_wtr = builder
        .from_path(&path)
        .with_context(|_| format!("Error reading {:?}", path))?;
    for fare_v1 in fares_v1.values() {
        fares_wtr
            .serialize(fare_v1)
            .with_context(|_| format!("Error reading {:?}", path))?;
    }
    fares_wtr
        .flush()
        .with_context(|_| format!("Error reading {:?}", path))?;

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
        .filter(|ticket_price| ticket_price.ticket_id == ticket_id)
        .filter(|ticket_price| ticket_price.currency == "EUR")
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
            ticket_id: ticket.id.clone(),
            ..Default::default()
        });
        fares_v1.insert(FareV1 {
            before_change: format!("network=network:{}", ticket_use_perimeter.object_id),
            after_change: format!("network=network:{}", ticket_use_perimeter.object_id),
            ..Default::default()
        });
        for connected_perimeter in fares
            .ticket_use_perimeters
            .values()
            .filter(|p| p.object_type == ObjectType::Network)
            .filter(|p| p.perimeter_action == PerimeterAction::Included)
            .filter(|p| p.ticket_use_id == ticket_use_perimeter.ticket_use_id)
            .filter(|p| p.object_id != ticket_use_perimeter.object_id)
        {
            fares_v1.insert(FareV1 {
                before_change: format!("network=network:{}", ticket_use_perimeter.object_id),
                after_change: format!("network=network:{}", connected_perimeter.object_id),
                ..Default::default()
            });
        }
    }
    Ok(())
}



fn construct_fare_v1_from_v2(
    fares: &Fares,
    prices_v1: &mut BTreeSet<PriceV1>,
    fares_v1: &mut BTreeSet<FareV1>,
) -> Result<()> {

    //we check that each ticket_use_id appears only once in ticket_uses
    {
        let mut unique_ids = BTreeSet::new();
        for ticket_use in fares.ticket_uses.values() {
            unique_ids.insert(ticket_use.id.as_str());
        }
        if unique_ids.len() != fares.ticket_uses.len() {
            let duplicated_ids : Vec<&str> = unique_ids.iter()
                                            .filter(|&unique_id| 
                                                fares.ticket_uses.values().filter(|ticket_use| ticket_use.id.as_str() == *unique_id).count() > 1 
                                            ).map(|&unique_id| unique_id)
                                            .collect();
            format_err!("ticket_uses.txt contains multiple time the same ticket_use_id, \
                        whereas a ticket_use_id must appears only once.\n\
                        Duplicated ticket_use_ids : {:?}", duplicated_ids);
        }
    };
    // we handle ticket_use one by one
    for ticket_use in fares.ticket_uses.values() {
        // let's recover the included and excluded perimeters
        // associated to our ticket_use_id
        let (included_networks, included_lines, excluded_lines) = {
            let mut included_networks = Vec::<&str>::new();
            let mut included_lines    = Vec::<&str>::new();
            let mut excluded_lines    = Vec::<&str>::new();
            for perimeter in fares.ticket_use_perimeters.values() {
                if perimeter.ticket_use_id != ticket_use.id {
                    continue;
                }
                match (&perimeter.object_type, &perimeter.perimeter_action) {
                    (ObjectType::Network, PerimeterAction::Included) => { included_networks.push(perimeter.object_id.as_str()); }
                    (ObjectType::Line, PerimeterAction::Included) => { included_lines.push(perimeter.object_id.as_str()); }
                    (ObjectType::Line, PerimeterAction::Excluded) => { excluded_lines.push(perimeter.object_id.as_str()); }
                     _ => {
                         bail!("Badly formed ticket_use_perimeter : \n {:?} \n\
                                Accepted forms : \n\
                                ticket_use_id, object_type, object_id, perimeter_action\n\
                                my_use_id    , network    , my_obj_id,  1 \n\
                                my_use_id    , line       , my_obj_id,  1 \n\
                                my_use_id    , line       , my_obj_id,  2 \n"                           
                                , perimeter);
                     }
                }
            }
            (included_networks, included_lines, excluded_lines)
        };

        // Now the restrictions for our ticket_use_id
        let restrictions : Vec<&TicketUseRestriction> = fares.ticket_use_restrictions.values()
                            .filter(|restriction| restriction.ticket_use_id.as_str() == ticket_use.id )
                            .collect();

        // Now the ticket for our ticket_use_id.
        //  there cannot exists two Ticket with the same ticket_id in fares.tickets
        //  thus it is sufficient to check if one ticket exists with the requested ticket_id
        let ticket = fares.tickets.get(&ticket_use.ticket_id)
                                    .ok_or_else(|| format_err!("The ticket_id {:?} was not found in tickets.txt"
                                                                , ticket_use.ticket_id)
                                                )?;

        // Now the price of the ticket for our ticket_use_id
        let price = {
            let mut has_price = None;
            for ticket_price in fares.ticket_prices.values() {
                if ticket_price.ticket_id == ticket_use.ticket_id {
                    if has_price.is_some() {
                        bail!("ticket_prices.txt contains multiple times the same ticket_id, \
                                whereas a  ticket_id can appears only once.\n\
                                Duplicated ticket_id : {:?}", ticket_use.ticket_id );
                    }
                    has_price = Some(ticket_price);
                }
            }
            has_price.ok_or_else(|| 
                format_err!("The ticket_id {:?} was not found in ticket_prices.txt", ticket_use.ticket_id)
            )?
        };

        //We have everything, so let's fill the fare v1 data !

        //first  prices_v1 
        // Note :  ticket_use_id of fare v2 plays the role of ticket_id in fare v1
        // so  price_v1.id = ticket_use.id 
        {
            if prices_v1.iter().find(|price_v1| price_v1.id == ticket_use.id).is_some() {
                bail!("There is two prices with the same id {:?}", ticket_use.id);
            }
            let cents_price = price.price * Decimal::from(100);
            let cents_price = cents_price
                .round_dp(0)
                .to_u32()
                .ok_or_else(|| format_err!("Cannot convert price {:?} into a u32", cents_price))?;
            let comment = ticket.comment.as_ref().cloned().unwrap_or_else(String::new);
            let price_v1 = PriceV1 {
                id: ticket_use.id.clone(),
                start_date: price.ticket_validity_start,
                end_date: price.ticket_validity_end + Duration::days(1),
                price: cents_price,
                name: ticket.name.clone(),
                ignored: String::new(),
                comment,
                currency_type: Some(price.currency.clone()),
            };
            prices_v1.insert(price_v1);
        }

        //now fares_v1
        {
            let states = included_networks
                                .iter()
                                .map(|network| format!("network=network:{}", network))
                                .chain(included_lines
                                        .iter()
                                        .map(|line| format!("line=line:{}", line))
                                );
                        
            // will yield a sequence of String
            // each  corresponds to a start_trip condition
            //  in FareV1
            // these conditions must appears in all transitions (i.e. lines of fares.txt)
            //  used to model this ticket_use_id                                
            let mandatory_start_conditions = 
                    excluded_lines
                    .iter()
                    .map( |line| 
                            format!("line!=line:{}", line)
                    ).chain(
                        ticket_use.max_transfers.iter().map(|nb_max_transfers|
                            format!("nb_changes<{}", nb_max_transfers + 1)
                        )
                    ).chain(
                        ticket_use.boarding_time_limit.iter().map(|time_limit|
                            format!("duration<{}", time_limit + 1)
                        )
                    );

            // will yield a sequence of String
            // each  corresponds to a end_trip condition
            //  in FareV1
            // these conditions must appears in all transitions (i.e. lines of fares.txt)
            //  used to model this ticket_use_id 
            let mandatory_end_condition = 
                ticket_use.alighting_time_limit
                .iter()
                .map(|time_limit|
                    format!("duration<{}", time_limit + 1)
                );
            

            let insert_one_ticket = | extra_start_condition : Option<String>,
                                      extra_end_condition : Option<String> ,
                                      fares : &mut BTreeSet<FareV1>
                                    | {
                let start_condition_string = 
                        extra_start_condition
                        .into_iter()
                        .chain(mandatory_start_conditions.clone())
                        .collect::<Vec<String>>()
                        .join("&");
                let end_condition_string =
                        extra_end_condition
                        .into_iter()
                        .chain(mandatory_end_condition.clone())
                        .collect::<Vec<String>>()
                        .join("&");
                for state in states.clone() {
                    fares.insert(FareV1 {
                        before_change : "*".to_owned(),
                        after_change : state.clone(),
                        start_trip : start_condition_string.clone(),
                        end_trip : end_condition_string.clone(),
                        global_condition : String::new(),
                        ticket_id : ticket_use.id.clone(),
                    });

                    for state2 in states.clone() {
                        fares.insert(FareV1 {
                            before_change : state.clone(),
                            after_change : state2.clone(),
                            start_trip : format!("ticket={}&{}", ticket_use.id, start_condition_string) ,
                            end_trip : end_condition_string.clone(),
                            global_condition : String::new(),
                            ticket_id : String::new(),
                        });
                        // print state;state2;
                        //        mandatory_start_conditions + choice_start_condition + ticket=ticket_use_id;
                        //        mandatory_end_condition + choice_end_conditions;
                        //        

                    }
                }
            };

            if restrictions.is_empty() {
                insert_one_ticket(None, None, fares_v1);
            }
            else {
                for restriction in restrictions {
                    let (extra_start_cond, extra_end_cond) = {
                        match &restriction.restriction_type {
                            RestrictionType::Zone => {
                                (   Some(format!("zone={:?}", restriction.use_origin)),
                                    Some(format!("zone={:?}", restriction.use_destination))
                                )                                        
                            }
                            RestrictionType::OriginDestination => {
                                (   Some(format!("stoparea={:?}", restriction.use_origin)),
                                    Some(format!("stoparea={:?}", restriction.use_destination))
                                )
                            }
                        }
                    };

                    insert_one_ticket(extra_start_cond, extra_end_cond, fares_v1);

                }               
            }            
        }
    }    
    Ok(())
}


fn do_write_fares_v1_from_v2(base_path: &path::Path, fares: &Fares) -> Result<()> {
    let mut prices_v1: BTreeSet<PriceV1> = BTreeSet::new();
    let mut fares_v1: BTreeSet<FareV1> = BTreeSet::new();

    //insert_od_specific_line_as_fare_v1(fares, &mut prices_v1, &mut fares_v1)?;
    //insert_flat_fare_as_fare_v1(fares, &mut prices_v1, &mut fares_v1)?;
    construct_fare_v1_from_v2(fares, & mut prices_v1, & mut fares_v1)?;

    if prices_v1.is_empty() || fares_v1.is_empty() {
        bail!("Cannot convert Fares V2 to V1. Prices or fares are empty.")
    }
    do_write_fares_v1(
        base_path,
        &Collection::new(prices_v1.into_iter().collect()),
        &Collection::default(),
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

pub fn write_stops(
    path: &path::Path,
    stop_points: &CollectionWithId<StopPoint>,
    stop_areas: &CollectionWithId<StopArea>,
    stop_locations: &CollectionWithId<StopLocation>,
) -> Result<()> {
    fn write_stop_locations(
        wtr: &mut Writer<File>,
        stop_locations: &CollectionWithId<StopLocation>,
    ) -> Result<()> {
        for sl in stop_locations.values() {
            let (lon, lat) = sl.coord.into();
            wtr.serialize(Stop {
                id: sl.id.clone(),
                visible: sl.visible,
                name: sl.name.clone(),
                code: sl.code.clone(),
                lat,
                lon,
                fare_zone_id: None,
                location_type: StopLocationType::from(sl.stop_type.clone()),
                parent_station: sl.parent_id.clone(),
                timezone: sl.timezone.clone(),
                equipment_id: sl.equipment_id.clone(),
                geometry_id: sl.geometry_id.clone(),
                level_id: sl.level_id.clone(),
                platform_code: None,
            })?;
        }
        Ok(())
    }
    let file = "stops.txt";
    info!("Writing {}", file);
    let path = path.join(file);
    let mut wtr =
        csv::Writer::from_path(&path).with_context(|_| format!("Error reading {:?}", path))?;
    for st in stop_points.values() {
        let location_type = if st.stop_type == StopType::Zone {
            StopLocationType::GeographicArea
        } else {
            StopLocationType::from(st.stop_type.clone())
        };
        wtr.serialize(Stop {
            id: st.id.clone(),
            visible: st.visible,
            name: st.name.clone(),
            code: st.code.clone(),
            lat: st.coord.lat.to_string(),
            lon: st.coord.lon.to_string(),
            fare_zone_id: st.fare_zone_id.clone(),
            location_type,
            parent_station: stop_areas.get(&st.stop_area_id).map(|sa| sa.id.clone()),
            timezone: st.timezone.clone(),
            equipment_id: st.equipment_id.clone(),
            geometry_id: st.geometry_id.clone(),
            level_id: st.level_id.clone(),
            platform_code: st.platform_code.clone(),
        })
        .with_context(|_| format!("Error reading {:?}", path))?;
    }

    for sa in stop_areas.values() {
        wtr.serialize(Stop {
            id: sa.id.clone(),
            visible: sa.visible,
            name: sa.name.clone(),
            code: None,
            lat: sa.coord.lat.to_string(),
            lon: sa.coord.lon.to_string(),
            fare_zone_id: None,
            location_type: StopLocationType::StopArea,
            parent_station: None,
            timezone: sa.timezone.clone(),
            equipment_id: sa.equipment_id.clone(),
            geometry_id: sa.geometry_id.clone(),
            level_id: sa.level_id.clone(),
            platform_code: None,
        })
        .with_context(|_| format!("Error reading {:?}", path))?;
    }
    write_stop_locations(&mut wtr, stop_locations)
        .with_context(|_| format!("Error reading {:?}", path))?;
    wtr.flush()
        .with_context(|_| format!("Error reading {:?}", path))?;

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
            .with_context(|_| format!("Error reading {:?}", path))?;
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
        .with_context(|_| format!("Error reading {:?}", path))?;
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

    let mut c_wtr = csv::Writer::from_path(&comments_path)
        .with_context(|_| format!("Error reading {:?}", comments_path))?;
    let mut cl_wtr = csv::Writer::from_path(&comment_links_path)
        .with_context(|_| format!("Error reading {:?}", comment_links_path))?;
    for c in collections.comments.values() {
        c_wtr
            .serialize(c)
            .with_context(|_| format!("Error reading {:?}", comments_path))?;
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
        .with_context(|_| format!("Error reading {:?}", comment_links_path))?;
    c_wtr
        .flush()
        .with_context(|_| format!("Error reading {:?}", comments_path))?;

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
            .with_context(|_| format!("Error reading {:?}", path))?;
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

    let mut wtr =
        csv::Writer::from_path(&path).with_context(|_| format!("Error reading {:?}", path))?;
    write_codes_from_collection_with_id(&mut wtr, &collections.stop_areas, &path)?;
    write_codes_from_collection_with_id(&mut wtr, &collections.stop_points, &path)?;
    write_codes_from_collection_with_id(&mut wtr, &collections.networks, &path)?;
    write_codes_from_collection_with_id(&mut wtr, &collections.lines, &path)?;
    write_codes_from_collection_with_id(&mut wtr, &collections.routes, &path)?;
    write_codes_from_collection_with_id(&mut wtr, &collections.vehicle_journeys, &path)?;

    wtr.flush()
        .with_context(|_| format!("Error reading {:?}", path))?;

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
            .with_context(|_| format!("Error reading {:?}", path))?;
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

    let mut wtr =
        csv::Writer::from_path(&path).with_context(|_| format!("Error reading {:?}", path))?;
    write_object_properties_from_collection_with_id(&mut wtr, &collections.stop_areas, &path)?;
    write_object_properties_from_collection_with_id(&mut wtr, &collections.stop_points, &path)?;
    write_object_properties_from_collection_with_id(&mut wtr, &collections.lines, &path)?;
    write_object_properties_from_collection_with_id(&mut wtr, &collections.routes, &path)?;
    write_object_properties_from_collection_with_id(
        &mut wtr,
        &collections.vehicle_journeys,
        &path,
    )?;

    wtr.flush()
        .with_context(|_| format!("Error reading {:?}", path))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    mod has_constraints {
        use super::*;
        use pretty_assertions::assert_eq;

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
            assert_eq!(false, has_constraints(&ticket_use));
        }

        #[test]
        fn no_constraints_with_zero_transfers() {
            let mut ticket_use = TicketUse::default();
            ticket_use.max_transfers = Some(0);
            assert_eq!(false, has_constraints(&ticket_use));
        }

        #[test]
        fn transfer_constraint() {
            let mut ticket_use = TicketUse::default();
            ticket_use.max_transfers = Some(1);
            assert_eq!(true, has_constraints(&ticket_use));
        }

        #[test]
        fn boarding_constraint() {
            let mut ticket_use = TicketUse::default();
            ticket_use.boarding_time_limit = Some(666);
            assert_eq!(true, has_constraints(&ticket_use));
        }
    }
}
