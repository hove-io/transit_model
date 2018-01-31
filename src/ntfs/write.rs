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
use super::{CalendarDate, StopTime};

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
