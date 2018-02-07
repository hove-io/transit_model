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
use serde;

use objects::*;
use collection::{Collection, CollectionWithId, Id, Idx};
use Collections;
use super::{CalendarDate, Code, CommentLink, ObjectProperty, Stop, StopTime};

pub fn make_collection_with_id<T>(path: &path::Path, file: &str) -> CollectionWithId<T>
where
    T: Id<T>,
    for<'de> T: serde::Deserialize<'de>,
{
    info!("Reading {}", file);
    let mut rdr = csv::Reader::from_path(path.join(file)).unwrap();
    let vec = rdr.deserialize().map(Result::unwrap).collect();
    CollectionWithId::new(vec)
}

pub fn make_collection<T>(path: &path::Path, file: &str) -> Collection<T>
where
    for<'de> T: serde::Deserialize<'de>,
{
    info!("Reading {}", file);
    if !path.join(file).exists() {
        panic!("file {} does not exist", file);
    }
    let mut rdr = csv::Reader::from_path(path.join(file)).unwrap();
    let vec = rdr.deserialize().map(Result::unwrap).collect();
    Collection::new(vec)
}

impl From<Stop> for StopArea {
    fn from(stop: Stop) -> StopArea {
        StopArea {
            id: stop.id,
            name: stop.name,
            codes: KeysValues::default(),
            object_properties: KeysValues::default(),
            comment_links: CommentLinksT::default(),
            visible: stop.visible,
            coord: Coord {
                lon: stop.lon,
                lat: stop.lat,
            },
            timezone: stop.timezone,
            geometry_id: stop.geometry_id,
            equipment_id: stop.equipment_id,
        }
    }
}
impl From<Stop> for StopPoint {
    fn from(stop: Stop) -> StopPoint {
        let id = stop.id;
        let stop_area_id = stop.parent_station.unwrap_or_else(|| id.clone());
        StopPoint {
            id: id,
            name: stop.name,
            codes: KeysValues::default(),
            object_properties: KeysValues::default(),
            comment_links: CommentLinksT::default(),
            visible: stop.visible,
            coord: Coord {
                lon: stop.lon,
                lat: stop.lat,
            },
            stop_area_id: stop_area_id,
            timezone: stop.timezone,
            geometry_id: stop.geometry_id,
            equipment_id: stop.equipment_id,
            fare_zone_id: stop.fare_zone_id,
        }
    }
}

pub fn manage_stops(collections: &mut Collections, path: &path::Path) {
    info!("Reading stops.txt");
    let mut rdr = csv::Reader::from_path(path.join("stops.txt")).unwrap();
    let mut stop_areas = vec![];
    let mut stop_points = vec![];
    for stop in rdr.deserialize().map(Result::unwrap) {
        let stop: Stop = stop;
        match stop.location_type {
            0 => {
                if stop.parent_station.is_none() {
                    let mut new_stop_area = stop.clone();
                    new_stop_area.id = format!("Navitia:{}", new_stop_area.id);
                    stop_areas.push(StopArea::from(new_stop_area));
                }
                stop_points.push(StopPoint::from(stop));
            }
            1 => stop_areas.push(StopArea::from(stop)),
            _ => (),
        }
    }
    collections.stop_areas = CollectionWithId::new(stop_areas);
    collections.stop_points = CollectionWithId::new(stop_points);
}

pub fn manage_stop_times(collections: &mut Collections, path: &path::Path) {
    info!("Reading stop_times.txt");
    let mut rdr = csv::Reader::from_path(path.join("stop_times.txt")).unwrap();
    for stop_time in rdr.deserialize().map(Result::unwrap) {
        let stop_time: StopTime = stop_time;
        let stop_point_idx = collections.stop_points.get_idx(&stop_time.stop_id).unwrap();
        let vj_idx = collections
            .vehicle_journeys
            .get_idx(&stop_time.trip_id)
            .unwrap();
        collections
            .vehicle_journeys
            .index_mut(vj_idx)
            .stop_times
            .push(::objects::StopTime {
                stop_point_idx: stop_point_idx,
                sequence: stop_time.stop_sequence,
                arrival_time: stop_time.arrival_time,
                departure_time: stop_time.departure_time,
                boarding_duration: stop_time.boarding_duration,
                alighting_duration: stop_time.alighting_duration,
                pickup_type: stop_time.pickup_type,
                dropoff_type: stop_time.dropoff_type,
                datetime_estimated: stop_time.datetime_estimated,
                local_zone_id: stop_time.local_zone_id,
            });
    }
    let mut vehicle_journeys = collections.vehicle_journeys.take();
    for vj in &mut vehicle_journeys {
        vj.stop_times.sort_unstable_by_key(|st| st.sequence);
    }
    collections.vehicle_journeys = CollectionWithId::new(vehicle_journeys);
}

fn insert_code_with_idx<T>(collection: &mut CollectionWithId<T>, idx: Idx<T>, code: Code)
where
    T: Codes + Id<T>,
{
    collection
        .index_mut(idx)
        .codes_mut()
        .push((code.object_system, code.object_code));
}
fn insert_code<T>(collection: &mut CollectionWithId<T>, code: Code)
where
    T: Codes + Id<T>,
{
    let idx = match collection.get_idx(&code.object_id) {
        Some(idx) => idx,
        None => {
            error!(
                "object_codes.txt: object_type={} object_id={} not found",
                code.object_type.as_str(),
                code.object_id
            );
            return;
        }
    };
    insert_code_with_idx(collection, idx, code);
}

pub fn manage_codes(collections: &mut Collections, path: &path::Path) {
    info!("Reading object_codes.txt");
    let mut rdr = csv::Reader::from_path(path.join("object_codes.txt")).unwrap();
    for code in rdr.deserialize().map(Result::unwrap) {
        let code: Code = code;
        match code.object_type {
            ObjectType::StopArea => insert_code(&mut collections.stop_areas, code),
            ObjectType::StopPoint => insert_code(&mut collections.stop_points, code),
            ObjectType::Network => insert_code(&mut collections.networks, code),
            ObjectType::Line => insert_code(&mut collections.lines, code),
            ObjectType::Route => insert_code(&mut collections.routes, code),
            ObjectType::VehicleJourney => insert_code(&mut collections.vehicle_journeys, code),
            _ => panic!("code does not support {}", code.object_type.as_str()),
        }
    }
}

fn insert_calendar_date(collection: &mut CollectionWithId<Calendar>, calendar_date: CalendarDate) {
    let idx = match collection.get_idx(&calendar_date.service_id) {
        Some(idx) => idx,
        None => {
            error!(
                "calendar_dates.txt: service_id={} not found",
                calendar_date.service_id
            );
            return;
        }
    };
    collection
        .index_mut(idx)
        .calendar_dates
        .push((calendar_date.date, calendar_date.exception_type))
}

pub fn manage_calendars(collections: &mut Collections, path: &path::Path) {
    collections.calendars = make_collection_with_id(path, "calendar.txt");

    info!("Reading calendar_dates.txt");
    if let Ok(mut rdr) = csv::Reader::from_path(path.join("calendar_dates.txt")) {
        for calendar_date in rdr.deserialize().map(Result::unwrap) {
            let calendar_date: CalendarDate = calendar_date;
            insert_calendar_date(&mut collections.calendars, calendar_date);
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct FeedInfo {
    #[serde(rename = "feed_info_param")]
    info_param: String,
    #[serde(rename = "feed_info_value")]
    info_value: String,
}

pub fn manage_feed_infos(collections: &mut Collections, path: &path::Path) {
    info!("Reading feed_infos.txt");
    let mut rdr = csv::Reader::from_path(path.join("feed_infos.txt")).unwrap();
    collections.feed_infos = rdr.deserialize::<FeedInfo>().map(Result::unwrap).fold(
        HashMap::default(),
        |mut acc, r| {
            assert!(
                acc.insert(r.info_param.to_string(), r.info_value.to_string())
                    .is_none(),
                "{} already found in file feed_infos.txt",
                r.info_param,
            );
            acc
        },
    )
}

fn insert_comment_link<T>(collection: &mut CollectionWithId<T>, comment_link: CommentLink)
where
    T: CommentLinks + Id<T>,
{
    let idx = match collection.get_idx(&comment_link.object_id) {
        Some(idx) => idx,
        None => {
            error!(
                "comment_links.txt: object_type={} object_id={} not found",
                comment_link.object_type.as_str(),
                comment_link.object_id
            );
            return;
        }
    };
    collection
        .index_mut(idx)
        .comment_links_mut()
        .push(comment_link.comment_id);
}

pub fn manage_comments(collections: &mut Collections, path: &path::Path) {
    if path.join("comments.txt").exists() {
        collections.comments = make_collection_with_id(path, "comments.txt");

        if let Ok(mut rdr) = csv::Reader::from_path(path.join("comment_links.txt")) {
            info!("Reading comment_links.txt");
            for comment_link in rdr.deserialize().map(Result::unwrap) {
                let comment_link: CommentLink = comment_link;
                match comment_link.object_type {
                    ObjectType::StopArea => {
                        insert_comment_link(&mut collections.stop_areas, comment_link)
                    }
                    ObjectType::StopPoint => {
                        insert_comment_link(&mut collections.stop_points, comment_link)
                    }
                    ObjectType::Line => insert_comment_link(&mut collections.lines, comment_link),
                    ObjectType::Route => insert_comment_link(&mut collections.routes, comment_link),
                    ObjectType::VehicleJourney => {
                        insert_comment_link(&mut collections.vehicle_journeys, comment_link)
                    }
                    ObjectType::StopTime => warn!("comments are not added to StopTime yet"),
                    ObjectType::LineGroup => warn!("line_groups.txt is not parsed yet"),
                    _ => panic!(
                        "comment does not support {}",
                        comment_link.object_type.as_str()
                    ),
                }
            }
        }
    }
}

fn insert_object_property<T>(collection: &mut CollectionWithId<T>, obj_prop: ObjectProperty)
where
    T: ObjectProperties + Id<T>,
{
    let idx = match collection.get_idx(&obj_prop.object_id) {
        Some(idx) => idx,
        None => {
            error!(
                "object_properties.txt: object_type={} object_id={} not found",
                obj_prop.object_type.as_str(),
                obj_prop.object_id
            );
            return;
        }
    };
    collection.index_mut(idx).object_properties_mut().push((
        obj_prop.object_property_name,
        obj_prop.object_property_value,
    ));
}

pub fn manage_object_properties(collections: &mut Collections, path: &path::Path) {
    info!("Reading object_properties.txt");
    let mut rdr = csv::Reader::from_path(path.join("object_properties.txt")).unwrap();
    for obj_prop in rdr.deserialize().map(Result::unwrap) {
        let obj_prop: ObjectProperty = obj_prop;
        match obj_prop.object_type {
            ObjectType::StopArea => insert_object_property(&mut collections.stop_areas, obj_prop),
            ObjectType::StopPoint => insert_object_property(&mut collections.stop_points, obj_prop),
            ObjectType::Line => insert_object_property(&mut collections.lines, obj_prop),
            ObjectType::Route => insert_object_property(&mut collections.routes, obj_prop),
            ObjectType::VehicleJourney => {
                insert_object_property(&mut collections.vehicle_journeys, obj_prop)
            }
            _ => panic!(
                "object_property does not support {}",
                obj_prop.object_type.as_str()
            ),
        }
    }
}
