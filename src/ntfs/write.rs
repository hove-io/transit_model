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

use std::collections::HashMap;
use std::path;
use csv;
use collection::{Collection, CollectionWithId, Id};
use serde;
use objects::*;
use Collections;
use super::{CalendarDate, Code, CommentLink, Stop, StopTime};

pub fn write_feed_infos(path: &path::Path, feed_infos: &HashMap<String, String>) {
    info!("Writing feed_infos.txt");
    let mut wtr = csv::Writer::from_path(&path.join("feed_infos.txt")).unwrap();
    wtr.write_record(&["feed_info_param", "feed_info_value"])
        .unwrap();
    for feed_info in feed_infos {
        wtr.serialize(feed_info).unwrap();
    }
    wtr.flush().unwrap();
}

pub fn write_vehicle_journeys_and_stop_times(
    path: &path::Path,
    vehicle_journeys: &CollectionWithId<VehicleJourney>,
    stop_points: &CollectionWithId<StopPoint>,
) {
    info!("Writing trips.txt and stop_times.txt");
    let mut vj_wtr = csv::Writer::from_path(&path.join("trips.txt")).unwrap();
    let mut st_wtr = csv::Writer::from_path(&path.join("stop_times.txt")).unwrap();
    for (_, vj) in vehicle_journeys.iter() {
        vj_wtr.serialize(vj).unwrap();

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
                .unwrap();
        }
    }
    st_wtr.flush().unwrap();
    vj_wtr.flush().unwrap();
}

pub fn write_collection_with_id<T>(path: &path::Path, file: &str, collection: &CollectionWithId<T>)
where
    T: Id<T>,
    T: serde::Serialize,
{
    info!("Writing {}", file);
    let mut wtr = csv::Writer::from_path(&path.join(file)).unwrap();
    for (_, obj) in collection.iter() {
        wtr.serialize(obj).unwrap();
    }
    wtr.flush().unwrap();
}

pub fn write_collection<T>(path: &path::Path, file: &str, collection: &Collection<T>)
where
    T: serde::Serialize,
{
    info!("Writing {}", file);
    let mut wtr = csv::Writer::from_path(&path.join(file)).unwrap();
    for (_, obj) in collection.iter() {
        wtr.serialize(obj).unwrap();
    }
    wtr.flush().unwrap();
}

pub fn write_calendar_and_calendar_dates(
    path: &path::Path,
    calendars: &CollectionWithId<Calendar>,
) {
    info!("Writing calendar.txt and calendar_dates.txt");
    let mut c_wtr = csv::Writer::from_path(&path.join("calendar.txt")).unwrap();
    let mut cd_wtr = csv::Writer::from_path(&path.join("calendar_dates.txt")).unwrap();
    for (_, c) in calendars.iter() {
        c_wtr.serialize(c).unwrap();
        for cd in &c.calendar_dates {
            cd_wtr
                .serialize(CalendarDate {
                    service_id: c.id.clone(),
                    date: cd.0,
                    exception_type: cd.1.clone(),
                })
                .unwrap();
        }
    }
    cd_wtr.flush().unwrap();
    c_wtr.flush().unwrap();
}

pub fn write_stops(
    path: &path::Path,
    stop_points: &CollectionWithId<StopPoint>,
    stop_areas: &CollectionWithId<StopArea>,
) {
    info!("Writing stops.txt");

    let mut wtr = csv::Writer::from_path(&path.join("stops.txt")).unwrap();
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
        }).unwrap();
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
            }).unwrap();
        }
    }
    wtr.flush().unwrap();
}

fn write_comment_links_from_collection_with_id<W, T>(
    wtr: &mut csv::Writer<W>,
    collections: &CollectionWithId<T>,
    object_type: &str,
) where
    T: Id<T> + CommentLinks,
    W: ::std::io::Write,
{
    for (_, obj) in collections.iter() {
        for c_id in obj.comment_links() {
            wtr.serialize(CommentLink {
                object_id: obj.id().to_string(),
                object_type: object_type.to_string(),
                comment_id: c_id.clone(),
            }).unwrap();
        }
    }
}

pub fn write_comments(path: &path::Path, collections: &Collections) {
    info!("Writing stops.txt");

    let mut c_wtr = csv::Writer::from_path(&path.join("comments.txt")).unwrap();
    let mut cl_wtr = csv::Writer::from_path(&path.join("comment_links.txt")).unwrap();
    for (_, c) in collections.comments.iter() {
        c_wtr.serialize(c).unwrap();
    }

    write_comment_links_from_collection_with_id(&mut cl_wtr, &collections.stop_areas, "stop_area");
    write_comment_links_from_collection_with_id(
        &mut cl_wtr,
        &collections.stop_points,
        "stop_point",
    );
    write_comment_links_from_collection_with_id(&mut cl_wtr, &collections.lines, "line");
    write_comment_links_from_collection_with_id(&mut cl_wtr, &collections.routes, "route");
    write_comment_links_from_collection_with_id(&mut cl_wtr, &collections.vehicle_journeys, "trip");
    // TODO: add stop_times and line_groups

    cl_wtr.flush().unwrap();
    c_wtr.flush().unwrap();
}

fn write_codes_from_collection_with_id<W, T>(
    wtr: &mut csv::Writer<W>,
    collections: &CollectionWithId<T>,
    object_type: &str,
) where
    T: Id<T> + Codes,
    W: ::std::io::Write,
{
    for (_, obj) in collections.iter() {
        for c in obj.codes() {
            wtr.serialize(Code {
                object_id: obj.id().to_string(),
                object_type: object_type.to_string(),
                object_system: c.0.clone(),
                object_code: c.1.clone(),
            }).unwrap();
        }
    }
}

pub fn write_codes(path: &path::Path, collections: &Collections) {
    info!("Writing object_codes.txt");

    let mut wtr = csv::Writer::from_path(&path.join("object_codes.txt")).unwrap();
    write_codes_from_collection_with_id(&mut wtr, &collections.stop_areas, "stop_area");
    write_codes_from_collection_with_id(&mut wtr, &collections.stop_points, "stop_point");
    write_codes_from_collection_with_id(&mut wtr, &collections.networks, "network");
    write_codes_from_collection_with_id(&mut wtr, &collections.lines, "line");
    write_codes_from_collection_with_id(&mut wtr, &collections.routes, "route");
    write_codes_from_collection_with_id(&mut wtr, &collections.vehicle_journeys, "trip");

    wtr.flush().unwrap();
}
