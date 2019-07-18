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

use csv;
use std::path;

use super::{Code, CommentLink, ObjectProperty, Stop, StopTime};
use crate::collection::*;
use crate::model::Collections;
use crate::ntfs::has_fares_v2;
use crate::objects::*;
use crate::utils::make_collection_with_id;
use crate::Result;
use failure::{bail, ensure, format_err, ResultExt};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
impl StopPoint {
    fn from_with_type(stop: Stop, stop_type: StopType) -> StopPoint {
        let id = stop.id;
        StopPoint {
            id,
            name: stop.name,
            visible: stop.visible,
            coord: Coord {
                lon: stop.lon,
                lat: stop.lat,
            },
            stop_area_id: stop.parent_station.unwrap(),
            timezone: stop.timezone,
            geometry_id: stop.geometry_id,
            equipment_id: stop.equipment_id,
            fare_zone_id: stop.fare_zone_id,
            stop_type,
            ..Default::default()
        }
    }
}

pub fn manage_stops(collections: &mut Collections, path: &path::Path) -> Result<()> {
    info!("Reading stops.txt");
    let path = path.join("stops.txt");
    let mut rdr = csv::Reader::from_path(&path).with_context(ctx_from_path!(path))?;
    let mut stop_areas = vec![];
    let mut stop_points = vec![];
    for stop in rdr.deserialize() {
        let mut stop: Stop = stop.with_context(ctx_from_path!(path))?;
        match stop.location_type {
            0 | 2 => {
                let mut stop_type = StopType::Point;
                let mut area_visibility = true;
                if stop.location_type == 2 {
                    stop_type = StopType::Zone;
                    area_visibility = false;
                };
                if stop.parent_station.is_none() {
                    let mut new_stop_area = stop.clone();
                    new_stop_area.visible = area_visibility;
                    new_stop_area.id = format!("Navitia:{}", new_stop_area.id);
                    stop.parent_station = Some(new_stop_area.id.clone());
                    stop_areas.push(StopArea::from(new_stop_area));
                }
                stop_points.push(StopPoint::from_with_type(stop, stop_type));
            }
            1 => stop_areas.push(StopArea::from(stop)),
            i => warn!("stop.location_type = {} not yet supported, skipping.", i),
        }
    }
    collections.stop_areas = CollectionWithId::new(stop_areas)?;
    collections.stop_points = CollectionWithId::new(stop_points)?;
    Ok(())
}

pub fn manage_fares_v1(collections: &mut Collections, base_path: &path::Path) -> Result<()> {
    let file_prices = "prices.csv";
    let file_od_fares = "od_fares.csv";
    let file_fares = "fares.csv";

    if !base_path.join(file_prices).exists()
        || !base_path.join(file_od_fares).exists()
        || has_fares_v2(collections)
    {
        info!(
            "Skipping {}, {} and {}",
            file_prices, file_od_fares, file_fares
        );
        return Ok(());
    }

    let mut builder = csv::ReaderBuilder::new();
    builder.delimiter(b';');
    builder.has_headers(false);

    info!("Reading {}", file_prices);
    let path = base_path.join(file_prices);
    let mut rdr = builder
        .from_path(&path)
        .with_context(ctx_from_path!(path))?;
    let prices_v1 = rdr
        .deserialize()
        .collect::<std::result::Result<Vec<PriceV1>, _>>()
        .with_context(ctx_from_path!(path))?;
    collections.prices_v1 = Collection::new(prices_v1);

    builder.has_headers(true);

    info!("Reading {}", file_od_fares);
    let path = base_path.join(file_od_fares);
    let mut rdr = builder
        .from_path(&path)
        .with_context(ctx_from_path!(path))?;
    let od_fares_v1 = rdr
        .deserialize()
        .collect::<std::result::Result<Vec<ODFareV1>, _>>()
        .with_context(ctx_from_path!(path))?;
    collections.od_fares_v1 = Collection::new(od_fares_v1);

    if !base_path.join(file_fares).exists() {
        info!("Skipping {}", file_fares);
        return Ok(());
    }

    info!("Reading {}", file_fares);
    let path = base_path.join(file_fares);
    let mut rdr = builder
        .from_path(&path)
        .with_context(ctx_from_path!(path))?;
    let fares_v1 = rdr
        .deserialize()
        .collect::<std::result::Result<Vec<FareV1>, _>>()
        .with_context(ctx_from_path!(path))?;
    collections.fares_v1 = Collection::new(fares_v1);

    Ok(())
}

pub fn manage_stop_times(collections: &mut Collections, path: &path::Path) -> Result<()> {
    info!("Reading stop_times.txt");
    let path = path.join("stop_times.txt");
    let mut rdr = csv::Reader::from_path(&path).with_context(ctx_from_path!(path))?;
    let mut headsigns = HashMap::new();
    let mut stop_time_ids = HashMap::new();
    for stop_time in rdr.deserialize() {
        let stop_time: StopTime = stop_time.with_context(ctx_from_path!(path))?;
        let stop_point_idx = collections
            .stop_points
            .get_idx(&stop_time.stop_id)
            .ok_or_else(|| {
                format_err!(
                    "Problem reading {:?}: stop_id={:?} not found",
                    path,
                    stop_time.stop_id
                )
            })?;
        let vj_idx = collections
            .vehicle_journeys
            .get_idx(&stop_time.trip_id)
            .ok_or_else(|| {
                format_err!(
                    "Problem reading {:?}: trip_id={:?} not found",
                    path,
                    stop_time.trip_id
                )
            })?;

        if let Some(headsign) = stop_time.stop_headsign {
            headsigns.insert((vj_idx, stop_time.stop_sequence), headsign);
        }
        let datetime_estimated = stop_time.datetime_estimated.map_or_else(
            || match collections.stop_points[stop_point_idx].stop_type {
                StopType::Zone => true,
                _ => false,
            },
            |v| v != 0,
        );

        if let Some(stop_time_id) = stop_time.stop_time_id {
            stop_time_ids.insert((vj_idx, stop_time.stop_sequence), stop_time_id);
        }

        collections
            .vehicle_journeys
            .index_mut(vj_idx)
            .stop_times
            .push(crate::objects::StopTime {
                stop_point_idx,
                sequence: stop_time.stop_sequence,
                arrival_time: stop_time.arrival_time,
                departure_time: stop_time.departure_time,
                boarding_duration: stop_time.boarding_duration,
                alighting_duration: stop_time.alighting_duration,
                pickup_type: stop_time.pickup_type,
                drop_off_type: stop_time.drop_off_type,
                datetime_estimated,
                local_zone_id: stop_time.local_zone_id,
            });
    }
    collections.stop_time_headsigns = headsigns;
    collections.stop_time_ids = stop_time_ids;
    let mut vehicle_journeys = collections.vehicle_journeys.take();
    for vj in &mut vehicle_journeys {
        vj.stop_times.sort_unstable_by_key(|st| st.sequence);
    }
    collections.vehicle_journeys = CollectionWithId::new(vehicle_journeys)?;
    Ok(())
}

fn insert_code_with_idx<T>(collection: &mut CollectionWithId<T>, idx: Idx<T>, code: Code)
where
    T: Codes + Id<T>,
{
    collection
        .index_mut(idx)
        .codes_mut()
        .insert((code.object_system, code.object_code));
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

pub fn manage_codes(collections: &mut Collections, path: &path::Path) -> Result<()> {
    let file = "object_codes.txt";
    if !path.join(file).exists() {
        info!("Skipping {}", file);
        return Ok(());
    }
    info!("Reading {}", file);
    let path = path.join(file);
    let mut rdr = csv::Reader::from_path(&path).with_context(ctx_from_path!(path))?;
    for code in rdr.deserialize() {
        let code: Code = code.with_context(ctx_from_path!(path))?;
        match code.object_type {
            ObjectType::StopArea => insert_code(&mut collections.stop_areas, code),
            ObjectType::StopPoint => insert_code(&mut collections.stop_points, code),
            ObjectType::Network => insert_code(&mut collections.networks, code),
            ObjectType::Line => insert_code(&mut collections.lines, code),
            ObjectType::Route => insert_code(&mut collections.routes, code),
            ObjectType::VehicleJourney => insert_code(&mut collections.vehicle_journeys, code),
            _ => bail!(
                "Problem reading {:?}: code does not support {}",
                path,
                code.object_type.as_str()
            ),
        }
    }
    Ok(())
}

#[derive(Serialize, Deserialize, Debug)]
struct FeedInfo {
    #[serde(rename = "feed_info_param")]
    info_param: String,
    #[serde(rename = "feed_info_value")]
    info_value: String,
}

pub fn manage_feed_infos(collections: &mut Collections, path: &path::Path) -> Result<()> {
    info!("Reading feed_infos.txt");
    let path = path.join("feed_infos.txt");
    let mut rdr = csv::Reader::from_path(&path).with_context(ctx_from_path!(path))?;
    collections.feed_infos.clear();
    for feed_info in rdr.deserialize() {
        let feed_info: FeedInfo = feed_info.with_context(ctx_from_path!(path))?;
        ensure!(
            collections
                .feed_infos
                .insert(feed_info.info_param.clone(), feed_info.info_value)
                .is_none(),
            "Problem reading {:?}: {} already found in file feed_infos.txt",
            path,
            feed_info.info_param,
        );
    }
    Ok(())
}

fn insert_comment_link<T>(
    collection: &mut CollectionWithId<T>,
    comments: &CollectionWithId<Comment>,
    comment_link: &CommentLink,
) -> Result<()>
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
            return Ok(());
        }
    };
    let comment_idx = match comments.get_idx(&comment_link.comment_id) {
        Some(comment_idx) => comment_idx,
        None => bail!(
            "comment.txt: comment_id={} not found",
            comment_link.comment_id
        ),
    };
    collection
        .index_mut(idx)
        .comment_links_mut()
        .insert(comment_idx);
    Ok(())
}

fn insert_stop_time_comment_link(
    stop_time_comments: &mut HashMap<(Idx<VehicleJourney>, u32), Idx<Comment>>,
    stop_time_ids: &HashMap<&String, (Idx<VehicleJourney>, u32)>,
    comments: &CollectionWithId<Comment>,
    comment_link: &CommentLink,
) -> Result<()> {
    let idx_sequence = match stop_time_ids.get(&comment_link.object_id) {
        Some(idx_sequence) => idx_sequence,
        None => {
            error!(
                "comment_links.txt: object_type={} object_id={} not found",
                comment_link.object_type.as_str(),
                comment_link.object_id
            );
            return Ok(());
        }
    };
    let comment_idx = match comments.get_idx(&comment_link.comment_id) {
        Some(comment_idx) => comment_idx,
        None => bail!(
            "comment.txt: comment_id={} not found",
            comment_link.comment_id
        ),
    };
    stop_time_comments.insert(*idx_sequence, comment_idx);
    Ok(())
}

pub fn manage_comments(collections: &mut Collections, path: &path::Path) -> Result<()> {
    if path.join("comments.txt").exists() {
        collections.comments = make_collection_with_id(path, "comments.txt")?;

        let path = path.join("comment_links.txt");
        if let Ok(mut rdr) = csv::Reader::from_path(&path) {
            // invert the stop_time_ids map to search a stop_time by it's id
            let stop_time_ids = collections
                .stop_time_ids
                .iter()
                .map(|(k, v)| (v, *k))
                .collect();
            info!("Reading comment_links.txt");
            for comment_link in rdr.deserialize() {
                let comment_link: CommentLink = comment_link.with_context(ctx_from_path!(path))?;
                match comment_link.object_type {
                    ObjectType::StopArea => insert_comment_link(
                        &mut collections.stop_areas,
                        &collections.comments,
                        &comment_link,
                    )?,
                    ObjectType::StopPoint => insert_comment_link(
                        &mut collections.stop_points,
                        &collections.comments,
                        &comment_link,
                    )?,
                    ObjectType::Line => insert_comment_link(
                        &mut collections.lines,
                        &collections.comments,
                        &comment_link,
                    )?,
                    ObjectType::Route => insert_comment_link(
                        &mut collections.routes,
                        &collections.comments,
                        &comment_link,
                    )?,
                    ObjectType::VehicleJourney => insert_comment_link(
                        &mut collections.vehicle_journeys,
                        &collections.comments,
                        &comment_link,
                    )?,
                    ObjectType::StopTime => insert_stop_time_comment_link(
                        &mut collections.stop_time_comments,
                        &stop_time_ids,
                        &collections.comments,
                        &comment_link,
                    )?,
                    ObjectType::LineGroup => warn!("line_groups.txt is not parsed yet"),
                    _ => bail!(
                        "comment does not support {}",
                        comment_link.object_type.as_str()
                    ),
                }
            }
        }
    }
    Ok(())
}

fn insert_object_property<T>(collection: &mut CollectionWithId<T>, obj_prop: ObjectProperty)
where
    T: Properties + Id<T>,
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
    collection.index_mut(idx).properties_mut().insert((
        obj_prop.object_property_name,
        obj_prop.object_property_value,
    ));
}

pub fn manage_object_properties(collections: &mut Collections, path: &path::Path) -> Result<()> {
    let file = "object_properties.txt";
    let path = path.join(file);
    if !path.exists() {
        info!("Skipping {}", file);
        return Ok(());
    }
    info!("Reading {}", file);
    let mut rdr = csv::Reader::from_path(&path).with_context(ctx_from_path!(path))?;
    for obj_prop in rdr.deserialize() {
        let obj_prop: ObjectProperty = obj_prop.with_context(ctx_from_path!(path))?;
        match obj_prop.object_type {
            ObjectType::StopArea => insert_object_property(&mut collections.stop_areas, obj_prop),
            ObjectType::StopPoint => insert_object_property(&mut collections.stop_points, obj_prop),
            ObjectType::Line => insert_object_property(&mut collections.lines, obj_prop),
            ObjectType::Route => insert_object_property(&mut collections.routes, obj_prop),
            ObjectType::VehicleJourney => {
                insert_object_property(&mut collections.vehicle_journeys, obj_prop)
            }
            _ => bail!(
                "Problem with {:?}: object_property does not support {}",
                path,
                obj_prop.object_type.as_str()
            ),
        }
    }
    Ok(())
}

pub fn manage_geometries(collections: &mut Collections, path: &path::Path) -> Result<()> {
    let file = "geometries.txt";
    let path = path.join(file);
    if !path.exists() {
        info!("Skipping {}", file);
        return Ok(());
    }

    info!("Reading {}", file);

    let mut geometries: Vec<Geometry> = vec![];
    let mut rdr = csv::Reader::from_path(&path).with_context(ctx_from_path!(path))?;
    for geometry in rdr.deserialize() {
        let geometry: Geometry = skip_fail!(geometry);
        geometries.push(geometry)
    }

    collections.geometries = CollectionWithId::new(geometries)?;

    Ok(())
}

pub fn manage_companies_on_vj(collections: &mut Collections) -> Result<()> {
    let vjs_without_company: Vec<Idx<VehicleJourney>> = collections
        .vehicle_journeys
        .iter()
        .filter_map(|(idx, _)| {
            if collections.vehicle_journeys[idx].company_id.is_empty() {
                Some(idx)
            } else {
                None
            }
        })
        .collect();

    if !vjs_without_company.is_empty() {
        let default_company = collections.companies.get_or_create("default_company");
        for vj_idx in vjs_without_company {
            collections.vehicle_journeys.index_mut(vj_idx).company_id =
                default_company.id.to_string();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::model::Collections;
    use crate::ntfs::read;
    use crate::test_utils::*;
    #[test]
    fn read_stop_points_with_no_parent() {
        let stops_content = "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\n\
                             sp:01,my stop name 1,0.1,1.2,0,";

        test_in_tmp_dir(|path| {
            create_file_with_content(path, "stops.txt", stops_content);
            let mut collections = Collections::default();
            read::manage_stops(&mut collections, path).unwrap();
            assert_eq!(collections.stop_points.len(), 1);
            let stop_point = collections.stop_points.values().next().unwrap();
            assert_eq!(stop_point.id, "sp:01");
            assert_eq!(stop_point.stop_area_id, "Navitia:sp:01");
            assert_eq!(collections.stop_areas.len(), 1);
            let stop_area = collections.stop_areas.values().next().unwrap();
            assert_eq!(stop_area.id, "Navitia:sp:01");
        });
    }
}
