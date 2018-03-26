// Copyright 2017-2018 Kisio Digital and/or its affiliates.
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
use collection::{Collection, CollectionWithId, Id};
use common_format::CalendarDate;
use csv;
use failure::ResultExt;
use model::Collections;
use objects::*;
use serde;
use std::collections::HashMap;
use std::path;

pub fn write_feed_infos(path: &path::Path, feed_infos: &HashMap<String, String>) -> Result<()> {
    info!("Writing feed_infos.txt");
    let path = path.join("feed_infos.txt");
    let mut wtr = csv::Writer::from_path(&path).with_context(ctx_from_path!(path))?;
    wtr.write_record(&["feed_info_param", "feed_info_value"])
        .with_context(ctx_from_path!(path))?;
    for feed_info in feed_infos {
        wtr.serialize(feed_info).with_context(ctx_from_path!(path))?;
    }
    wtr.flush().with_context(ctx_from_path!(path))?;
    Ok(())
}

pub fn write_vehicle_journeys_and_stop_times(
    path: &path::Path,
    vehicle_journeys: &CollectionWithId<VehicleJourney>,
    stop_points: &CollectionWithId<StopPoint>,
) -> Result<()> {
    info!("Writing trips.txt and stop_times.txt");
    let trip_path = path.join("trips.txt");
    let stop_times_path = path.join("stop_times.txt");
    let mut vj_wtr = csv::Writer::from_path(&trip_path).with_context(ctx_from_path!(trip_path))?;
    let mut st_wtr =
        csv::Writer::from_path(&stop_times_path).with_context(ctx_from_path!(stop_times_path))?;
    for (_, vj) in vehicle_journeys.iter() {
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
                    dropoff_type: st.dropoff_type,
                    datetime_estimated: st.datetime_estimated,
                    local_zone_id: st.local_zone_id,
                    // TODO: Add headsign and stop_time_ids
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

pub fn write_collection_with_id<T>(
    path: &path::Path,
    file: &str,
    collection: &CollectionWithId<T>,
) -> Result<()>
where
    T: Id<T>,
    T: serde::Serialize,
{
    info!("Writing {}", file);
    let path = path.join(file);
    let mut wtr = csv::Writer::from_path(&path).with_context(ctx_from_path!(path))?;
    for (_, obj) in collection.iter() {
        wtr.serialize(obj).with_context(ctx_from_path!(path))?;
    }
    wtr.flush().with_context(ctx_from_path!(path))?;

    Ok(())
}

pub fn write_collection<T>(path: &path::Path, file: &str, collection: &Collection<T>) -> Result<()>
where
    T: serde::Serialize,
{
    info!("Writing {}", file);
    let path = path.join(file);
    let mut wtr = csv::Writer::from_path(&path).with_context(ctx_from_path!(path))?;
    for (_, obj) in collection.iter() {
        wtr.serialize(obj).with_context(ctx_from_path!(path))?;
    }
    wtr.flush().with_context(ctx_from_path!(path))?;

    Ok(())
}

pub fn write_calendar_and_calendar_dates(
    path: &path::Path,
    calendars: &CollectionWithId<Calendar>,
) -> Result<()> {
    info!("Writing calendar.txt and calendar_dates.txt");
    let calendar_path = path.join("calendar.txt");
    let calendar_dates_path = path.join("calendar_dates.txt");
    let mut c_wtr =
        csv::Writer::from_path(&calendar_path).with_context(ctx_from_path!(calendar_path))?;
    let mut cd_wtr = csv::Writer::from_path(&calendar_dates_path)
        .with_context(ctx_from_path!(calendar_dates_path))?;
    for (_, c) in calendars.iter() {
        c_wtr
            .serialize(c)
            .with_context(ctx_from_path!(calendar_path))?;
        for cd in &c.calendar_dates {
            cd_wtr
                .serialize(CalendarDate {
                    service_id: c.id.clone(),
                    date: cd.0,
                    exception_type: cd.1.clone(),
                })
                .with_context(ctx_from_path!(calendar_dates_path))?;
        }
    }
    cd_wtr
        .flush()
        .with_context(ctx_from_path!(calendar_dates_path))?;
    c_wtr.flush().with_context(ctx_from_path!(calendar_path))?;

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
    for (_, st) in stop_points.iter() {
        wtr.serialize(Stop {
            id: st.id.clone(),
            visible: st.visible,
            name: st.name.clone(),
            lat: st.coord.lat,
            lon: st.coord.lon,
            fare_zone_id: st.fare_zone_id.clone(),
            location_type: 0,
            parent_station: stop_areas.get(&st.stop_area_id).map(|sa| sa.id.clone()),
            timezone: st.timezone.clone(),
            equipment_id: st.equipment_id.clone(),
            geometry_id: st.geometry_id.clone(),
        }).with_context(ctx_from_path!(path))?;
    }

    for (_, sa) in stop_areas.iter() {
        if !sa.id.starts_with("Navitia:") {
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
            }).with_context(ctx_from_path!(path))?;
        }
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
    for (_, obj) in collection.iter() {
        for comment in comments.iter_from(obj.comment_links()) {
            wtr.serialize(CommentLink {
                object_id: obj.id().to_string(),
                object_type: T::get_object_type(),
                comment_id: comment.id.to_string(),
            }).with_context(ctx_from_path!(path))?;
        }
    }
    Ok(())
}

pub fn write_comments(path: &path::Path, collections: &Collections) -> Result<()> {
    info!("Writing comments.txt and comment_links.txt");

    let comments_path = path.join("comments.txt");
    let comment_links_path = path.join("comment_links.txt");

    let mut c_wtr =
        csv::Writer::from_path(&comments_path).with_context(ctx_from_path!(comments_path))?;
    let mut cl_wtr = csv::Writer::from_path(&comment_links_path)
        .with_context(ctx_from_path!(comment_links_path))?;
    for (_, c) in collections.comments.iter() {
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
    // TODO: add stop_times and line_groups

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
    for (_, obj) in collections.iter() {
        for c in obj.codes() {
            wtr.serialize(Code {
                object_id: obj.id().to_string(),
                object_type: T::get_object_type(),
                object_system: c.0.clone(),
                object_code: c.1.clone(),
            }).with_context(ctx_from_path!(path))?;
        }
    }

    Ok(())
}

pub fn write_codes(path: &path::Path, collections: &Collections) -> Result<()> {
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
    collections: &CollectionWithId<T>,
    path: &path::Path,
) -> Result<()>
where
    T: Id<T> + ObjectProperties + GetObjectType,
    W: ::std::io::Write,
{
    for (_, obj) in collections.iter() {
        for c in obj.object_properties() {
            wtr.serialize(ObjectProperty {
                object_id: obj.id().to_string(),
                object_type: T::get_object_type(),
                object_property_name: c.0.clone(),
                object_property_value: c.1.clone(),
            }).with_context(ctx_from_path!(path))?;
        }
    }

    Ok(())
}

pub fn write_object_properties(path: &path::Path, collections: &Collections) -> Result<()> {
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
