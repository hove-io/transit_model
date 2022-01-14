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

use super::{Code, CommentLink, ObjectProperty, Stop, StopLocationType, StopTime};
use crate::model::Collections;
use crate::ntfs::has_fares_v2;
use crate::objects::*;
use crate::read_utils::{read_objects, read_objects_loose, FileHandler};
use crate::utils::make_opt_collection_with_id;
use crate::Result;
use anyhow::{anyhow, bail, ensure, Context};
use serde::{Deserialize, Serialize};
use skip_error::skip_error_and_warn;
use std::collections::HashMap;
use std::convert::TryFrom;
use tracing::{error, info, warn};
use typed_index_collection::{Collection, CollectionWithId, Id, Idx};

impl TryFrom<Stop> for StopArea {
    type Error = anyhow::Error;
    fn try_from(stop: Stop) -> Result<Self> {
        if stop.name.is_empty() {
            warn!("stop_id: {}: for platform stop_name is required", stop.id);
        }

        let coord = Coord::from((stop.lon, stop.lat));
        if coord == Coord::default() {
            warn!(
                "stop_id: {}: for platform coordinates are required",
                stop.id
            );
        }
        let stop_area = StopArea {
            id: stop.id,
            name: stop.name,
            codes: KeysValues::default(),
            object_properties: PropertiesMap::default(),
            comment_links: CommentLinksT::default(),
            visible: stop.visible,
            coord,
            timezone: stop.timezone,
            geometry_id: stop.geometry_id,
            equipment_id: stop.equipment_id,
            level_id: stop.level_id,
        };
        Ok(stop_area)
    }
}

impl TryFrom<Stop> for StopPoint {
    type Error = anyhow::Error;
    fn try_from(stop: Stop) -> Result<Self> {
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
        let stop_point = StopPoint {
            id: stop.id,
            name: stop.name,
            code: stop.code,
            visible: stop.visible,
            coord,
            stop_area_id: stop
                .parent_station
                .unwrap_or_else(|| String::from("default_id")),
            timezone: stop.timezone,
            geometry_id: stop.geometry_id,
            equipment_id: stop.equipment_id,
            fare_zone_id: stop.fare_zone_id,
            stop_type: stop.location_type.into(),
            platform_code: stop.platform_code,
            level_id: stop.level_id,
            address_id: stop.address_id,
            ..Default::default()
        };
        Ok(stop_point)
    }
}

impl TryFrom<Stop> for StopLocation {
    type Error = anyhow::Error;
    fn try_from(stop: Stop) -> Result<Self> {
        let coord = Coord::from((stop.lon, stop.lat));

        if stop.location_type == StopLocationType::EntranceExit {
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
        if stop.location_type == StopLocationType::PathwayInterconnectionNode
            && stop.parent_station.is_none()
        {
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
            comment_links: CommentLinksT::default(),
            visible: false,
            coord,
            parent_id: stop.parent_station,
            timezone: stop.timezone,
            geometry_id: stop.geometry_id,
            equipment_id: stop.equipment_id,
            stop_type: stop.location_type.clone().into(),
            level_id: stop.level_id,
        };
        Ok(stop_location)
    }
}

pub(crate) fn manage_stops<H>(collections: &mut Collections, file_handler: &mut H) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    let stops = read_objects::<_, Stop>(file_handler, "stops.txt", true)?;
    let mut stop_areas: CollectionWithId<StopArea> = CollectionWithId::default();
    let mut stop_points: CollectionWithId<StopPoint> = CollectionWithId::default();
    let mut stop_locations: CollectionWithId<StopLocation> = CollectionWithId::default();
    for stop in stops {
        match stop.location_type {
            StopLocationType::StopPoint | StopLocationType::GeographicArea => {
                let mut stop_point = skip_error_and_warn!(StopPoint::try_from(stop.clone()));
                if stop.parent_station.is_none() {
                    let mut stop_area = StopArea::from(stop_point.clone());
                    stop_point.stop_area_id = stop_area.id.clone();
                    stop_area.visible = stop.location_type == StopLocationType::StopPoint;
                    skip_error_and_warn!(stop_areas.push(stop_area));
                };
                skip_error_and_warn!(stop_points.push(stop_point));
            }
            StopLocationType::StopArea => {
                skip_error_and_warn!(stop_areas.push(StopArea::try_from(stop)?));
            }
            _ => {
                skip_error_and_warn!(stop_locations.push(StopLocation::try_from(stop)?));
            }
        }
    }
    collections.stop_areas = stop_areas;
    collections.stop_points = stop_points;
    collections.stop_locations = stop_locations;
    Ok(())
}

// for legacy reason fares files are csv, and some don't have any headers, so we use a custom function
fn read_fares_v1<T, H>(
    file_handler: &mut H,
    file_name: &str,
    has_headers: bool,
) -> Result<Collection<T>>
where
    for<'a> &'a mut H: FileHandler,
    for<'de> T: serde::Deserialize<'de>,
{
    let (reader, path) = file_handler.get_file_if_exists(file_name)?;
    let file_name = path.file_name();
    let basename = file_name.map_or(path.to_string_lossy(), |b| b.to_string_lossy());

    match reader {
        None => {
            info!("Skipping {}", basename);
            Ok(Collection::default())
        }
        Some(reader) => {
            info!("Reading {}", basename);
            let mut rdr = csv::ReaderBuilder::new()
                .flexible(true)
                .has_headers(has_headers)
                .trim(csv::Trim::All)
                .delimiter(b';')
                .from_reader(reader);
            let res = rdr
                .deserialize()
                .collect::<Result<_, _>>()
                .with_context(|| format!("Error reading {:?}", path))?;
            Ok(Collection::new(res))
        }
    }
}

pub(crate) fn manage_fares_v1<H>(collections: &mut Collections, file_handler: &mut H) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    let file_prices = "prices.csv";
    let file_od_fares = "od_fares.csv";
    let file_fares = "fares.csv";

    if has_fares_v2(collections) {
        info!(
            "data has fares v2, skipping fares v1 files ({}, {} and {})",
            file_prices, file_od_fares, file_fares
        );
        return Ok(());
    }
    collections.prices_v1 = read_fares_v1(file_handler, "prices.csv", false)?;
    if collections.prices_v1.is_empty() {
        info!(
            "no prices found, skipping {} and {}",
            file_od_fares, file_fares
        );
        return Ok(());
    }
    collections.od_fares_v1 = read_fares_v1(file_handler, file_od_fares, true)?;
    collections.fares_v1 = read_fares_v1(file_handler, file_fares, true)?;

    Ok(())
}

pub(crate) fn manage_stop_times<H>(
    collections: &mut Collections,
    file_handler: &mut H,
) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    let stop_times = read_objects::<_, StopTime>(file_handler, "stop_times.txt", true)?;
    let mut headsigns = HashMap::new();
    let mut stop_time_ids = HashMap::new();
    for stop_time in stop_times {
        let stop_point_idx = collections
            .stop_points
            .get_idx(&stop_time.stop_id)
            .ok_or_else(|| {
                anyhow!(
                    "Problem reading {:?}: stop_id={:?} not found",
                    file_handler.source_name(),
                    stop_time.stop_id
                )
            })?;
        let vj_idx = collections
            .vehicle_journeys
            .get_idx(&stop_time.trip_id)
            .ok_or_else(|| {
                anyhow!(
                    "Problem reading {:?}: trip_id={:?} not found",
                    file_handler.source_name(),
                    stop_time.trip_id
                )
            })?;

        if let Some(headsign) = stop_time.stop_headsign {
            headsigns.insert(
                (stop_time.trip_id.clone(), stop_time.stop_sequence),
                headsign,
            );
        }
        let datetime_estimated = stop_time.datetime_estimated.map_or_else(
            || collections.stop_points[stop_point_idx].stop_type == StopType::Zone,
            |v| v != 0,
        );

        let precision = stop_time.precision.or_else(|| {
            if datetime_estimated {
                Some(StopTimePrecision::Estimated)
            } else {
                Some(StopTimePrecision::Exact)
            }
        });

        if let Some(stop_time_id) = stop_time.stop_time_id {
            stop_time_ids.insert(
                (stop_time.trip_id.clone(), stop_time.stop_sequence),
                stop_time_id,
            );
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
                local_zone_id: stop_time.local_zone_id,
                precision,
            });
    }
    collections.stop_time_headsigns = headsigns;
    collections.stop_time_ids = stop_time_ids;
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

pub(crate) fn manage_codes<H>(collections: &mut Collections, file_handler: &mut H) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    let codes = read_objects::<_, Code>(file_handler, "object_codes.txt", false)?;
    for code in codes {
        match code.object_type {
            ObjectType::StopArea => insert_code(&mut collections.stop_areas, code),
            ObjectType::StopPoint => insert_code(&mut collections.stop_points, code),
            ObjectType::Network => insert_code(&mut collections.networks, code),
            ObjectType::Line => insert_code(&mut collections.lines, code),
            ObjectType::Route => insert_code(&mut collections.routes, code),
            ObjectType::VehicleJourney => insert_code(&mut collections.vehicle_journeys, code),
            ObjectType::Company => insert_code(&mut collections.companies, code),
            _ => bail!(
                "Problem reading {:?}: code does not support {}",
                file_handler.source_name(),
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

pub(crate) fn manage_feed_infos<H>(
    collections: &mut Collections,
    file_handler: &mut H,
) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    let feed_infos = read_objects::<_, FeedInfo>(file_handler, "feed_infos.txt", true)?;
    collections.feed_infos.clear();
    for feed_info in feed_infos {
        ensure!(
            collections
                .feed_infos
                .insert(feed_info.info_param.clone(), feed_info.info_value)
                .is_none(),
            "Problem reading {:?}: {} already found in file feed_infos.txt",
            file_handler.source_name(),
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
    if !comments.contains_id(&comment_link.comment_id) {
        bail!(
            "comment.txt: comment_id={} not found",
            comment_link.comment_id
        );
    } else {
    }
    collection
        .index_mut(idx)
        .comment_links_mut()
        .insert(comment_link.comment_id.clone());
    Ok(())
}

fn insert_stop_time_comment_link(
    stop_time_comments: &mut HashMap<(String, u32), String>,
    stop_time_ids: &HashMap<&String, (String, u32)>,
    comments: &CollectionWithId<Comment>,
    comment_link: &CommentLink,
) -> Result<()> {
    if let Some(vehicle_journey_id) = stop_time_ids.get(&comment_link.object_id) {
        if comments.contains_id(&comment_link.comment_id) {
            stop_time_comments.insert(vehicle_journey_id.clone(), comment_link.comment_id.clone());
        } else {
            bail!(
                "comment.txt: comment_id={} not found",
                comment_link.comment_id
            )
        }
    } else {
        error!(
            "comment_links.txt: object_type={} object_id={} not found",
            comment_link.object_type.as_str(),
            comment_link.object_id
        );
    }
    Ok(())
}

pub(crate) fn manage_comments<H>(collections: &mut Collections, file_handler: &mut H) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    collections.comments = make_opt_collection_with_id(file_handler, "comments.txt")?;

    if collections.comments.is_empty() {
        // no need to read the comment_links (and invert the huge stoptimes collection)
        return Ok(());
    }
    let comment_links = read_objects::<_, CommentLink>(file_handler, "comment_links.txt", false)?;

    // invert the stop_time_ids map to search a stop_time by it's id
    let stop_time_ids = collections
        .stop_time_ids
        .iter()
        .map(|(k, v)| (v, k.clone()))
        .collect();
    info!("Reading comment_links.txt");
    for comment_link in comment_links {
        match comment_link.object_type {
            ObjectType::StopArea => skip_error_and_warn!(insert_comment_link(
                &mut collections.stop_areas,
                &collections.comments,
                &comment_link,
            )),
            ObjectType::StopPoint => skip_error_and_warn!(insert_comment_link(
                &mut collections.stop_points,
                &collections.comments,
                &comment_link,
            )),
            ObjectType::Line => {
                skip_error_and_warn!(insert_comment_link(
                    &mut collections.lines,
                    &collections.comments,
                    &comment_link
                ))
            }
            ObjectType::Route => skip_error_and_warn!(insert_comment_link(
                &mut collections.routes,
                &collections.comments,
                &comment_link,
            )),
            ObjectType::VehicleJourney => skip_error_and_warn!(insert_comment_link(
                &mut collections.vehicle_journeys,
                &collections.comments,
                &comment_link,
            )),
            ObjectType::StopTime => skip_error_and_warn!(insert_stop_time_comment_link(
                &mut collections.stop_time_comments,
                &stop_time_ids,
                &collections.comments,
                &comment_link,
            )),
            ObjectType::LineGroup => warn!("line_groups.txt is not parsed yet"),
            _ => warn!(
                "comment does not support {}",
                comment_link.object_type.as_str()
            ),
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
    collection.index_mut(idx).properties_mut().insert(
        obj_prop.object_property_name,
        obj_prop.object_property_value,
    );
}

pub(crate) fn manage_object_properties<H>(
    collections: &mut Collections,
    file_handler: &mut H,
) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    let obj_props =
        read_objects::<_, ObjectProperty>(file_handler, "object_properties.txt", false)?;
    for obj_prop in obj_props {
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
                file_handler.source_name(),
                obj_prop.object_type.as_str()
            ),
        }
    }
    Ok(())
}

pub(crate) fn manage_geometries<H>(
    collections: &mut Collections,
    file_handler: &mut H,
) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    let mut geometries: CollectionWithId<Geometry> = CollectionWithId::default();
    for geo in read_objects_loose::<_, Geometry>(file_handler, "geometries.txt", false)? {
        skip_error_and_warn!(geometries.push(geo));
    }
    collections.geometries = geometries;
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

pub(crate) fn manage_pathways<H>(collections: &mut Collections, file_handler: &mut H) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    let file = "pathways.txt";
    let mut pathways: CollectionWithId<Pathway> = CollectionWithId::default();
    let ntfs_pathways = read_objects_loose::<_, Pathway>(file_handler, file, false)?;
    for mut pathway in ntfs_pathways {
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
        skip_error_and_warn!(pathways.push(pathway));
    }

    collections.pathways = pathways;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendars;
    use crate::objects;
    use crate::read_utils::{self, PathFileHandler};
    use crate::test_utils::*;
    use crate::utils::make_collection_with_id;
    use pretty_assertions::assert_eq;
    use std::path;

    fn generate_minimal_ntfs<P: AsRef<path::Path>>(path: P) {
        let commercial_modes_content = "commercial_mode_id,commercial_mode_name\n\
                                        commercial_mode_1,My Commercial Mode 1";

        let networks_content = "network_id,network_name\n\
                                network_1, My Network 1";

        let lines_content = "line_id,line_name,network_id,commercial_mode_id\n\
                             line_1,My Line 1,network_1, commercial_mode_1";

        let routes_content = "route_id,route_name,line_id\n\
                              route_1,My Route 1,line_1";

        let stops_content = "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\n\
                             sp:01,my stop point name 1,0.1,1.2,0,\n\
                             sp:02,my stop point name 2,0.2,1.5,0,\n\
                             sp:03,my stop point name 3,0.2,1.5,0,\n\
                             sp:04,my stop point name 4,0.2,1.5,0,\n\
                             sp:05,my stop point name 5,0.2,1.5,2,";

        let companies_content = "company_id,company_name\n\
                                 company_1, My Company 1";

        let physical_modes_content = "physical_mode_id,physical_mode_name\n\
                                      physical_mode_1,My Physical Mode 1";

        let contributors_content = "contributor_id,contributor_name\n\
                                    contributor_1,My Contributor 1";

        let datasets_content = "dataset_id,contributor_id,dataset_start_date,dataset_end_date\n\
                                dataset_1,contributor_1,20190101,20191231";

        let calendar_content = "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
                                service_1,1,1,1,1,1,0,0,20190101,20191231";

        let trips_content = "trip_id,route_id,service_id,company_id,physical_mode_id,dataset_id\n\
                             1,route_1,service_1,company_1,physical_mode_1,dataset_1";

        let stop_times_content = "stop_time_id,trip_id,arrival_time,departure_time,stop_id,stop_sequence,pickup_type,drop_off_type,shape_dist_traveled,stop_time_precision\n\
                                  1,1,06:00:00,06:00:00,sp:01,1,0,0,,0\n\
                                  2,1,06:06:27,06:06:27,sp:02,2,2,1,,1\n\
                                  3,1,06:07:27,06:07:27,sp:03,3,2,1,,2\n\
                                  4,1,06:08:27,06:08:27,sp:04,4,2,1,,\n\
                                  5,1,06:09:27,06:09:27,sp:05,5,2,1,,";

        let path = path.as_ref();
        create_file_with_content(path, "commercial_modes.txt", commercial_modes_content);
        create_file_with_content(path, "networks.txt", networks_content);
        create_file_with_content(path, "lines.txt", lines_content);
        create_file_with_content(path, "routes.txt", routes_content);
        create_file_with_content(path, "stops.txt", stops_content);
        create_file_with_content(path, "companies.txt", companies_content);
        create_file_with_content(path, "physical_modes.txt", physical_modes_content);
        create_file_with_content(path, "contributors.txt", contributors_content);
        create_file_with_content(path, "datasets.txt", datasets_content);
        create_file_with_content(path, "trips.txt", trips_content);
        create_file_with_content(path, "calendar.txt", calendar_content);
        create_file_with_content(path, "stop_times.txt", stop_times_content);
    }

    fn make_collection(path: &path::Path) -> Collections {
        let mut collections = Collections::default();
        let mut file_handler = read_utils::PathFileHandler::new(path.to_path_buf());
        collections.contributors =
            make_collection_with_id(&mut file_handler, "contributors.txt").unwrap();
        collections.datasets = make_collection_with_id(&mut file_handler, "datasets.txt").unwrap();
        collections.commercial_modes =
            make_collection_with_id(&mut file_handler, "commercial_modes.txt").unwrap();
        collections.networks = make_collection_with_id(&mut file_handler, "networks.txt").unwrap();
        collections.lines = make_collection_with_id(&mut file_handler, "lines.txt").unwrap();
        collections.routes = make_collection_with_id(&mut file_handler, "routes.txt").unwrap();
        collections.vehicle_journeys =
            make_collection_with_id(&mut file_handler, "trips.txt").unwrap();
        collections.physical_modes =
            make_collection_with_id(&mut file_handler, "physical_modes.txt").unwrap();
        collections.companies =
            make_collection_with_id(&mut file_handler, "companies.txt").unwrap();
        calendars::manage_calendars(&mut file_handler, &mut collections).unwrap();
        manage_stops(&mut collections, &mut file_handler).unwrap();
        manage_stop_times(&mut collections, &mut file_handler).unwrap();
        manage_codes(&mut collections, &mut file_handler).unwrap();
        collections
    }

    #[test]
    fn read_stop_points_with_no_parent() {
        let stops_content =
            "stop_id,stop_name,stop_code,stop_lat,stop_lon,location_type,parent_station\n\
             sp:01,my stop name 1,stopcode,0.1,1.2,0,";

        test_in_tmp_dir(|path| {
            create_file_with_content(path, "stops.txt", stops_content);
            let mut collections = Collections::default();
            let mut handler = PathFileHandler::new(path.to_path_buf());
            manage_stops(&mut collections, &mut handler).unwrap();
            assert_eq!(1, collections.stop_points.len());
            let stop_point = collections.stop_points.values().next().unwrap();
            assert_eq!("sp:01", stop_point.id);
            assert_eq!("Navitia:sp:01", stop_point.stop_area_id);
            assert_eq!("stopcode", stop_point.code.as_ref().unwrap());
            assert_eq!(1, collections.stop_areas.len());
            let stop_area = collections.stop_areas.values().next().unwrap();
            assert_eq!("Navitia:sp:01", stop_area.id);
        });
    }
    #[test]
    fn ntfs_stop_times_precision() {
        test_in_tmp_dir(|path| {
            let _ = generate_minimal_ntfs(path);
            let collections = make_collection(path);

            assert_eq!(
                vec![
                    objects::StopTime {
                        stop_point_idx: collections.stop_points.get_idx("sp:01").unwrap(),
                        sequence: 1,
                        arrival_time: Time::new(6, 0, 0),
                        departure_time: Time::new(6, 0, 0),
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 0,
                        drop_off_type: 0,
                        local_zone_id: None,
                        precision: Some(StopTimePrecision::Exact),
                    },
                    objects::StopTime {
                        stop_point_idx: collections.stop_points.get_idx("sp:02").unwrap(),
                        sequence: 2,
                        arrival_time: Time::new(6, 6, 27),
                        departure_time: Time::new(6, 6, 27),
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 2,
                        drop_off_type: 1,
                        local_zone_id: None,
                        precision: Some(StopTimePrecision::Approximate),
                    },
                    objects::StopTime {
                        stop_point_idx: collections.stop_points.get_idx("sp:03").unwrap(),
                        sequence: 3,
                        arrival_time: Time::new(6, 7, 27),
                        departure_time: Time::new(6, 7, 27),
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 2,
                        drop_off_type: 1,
                        local_zone_id: None,
                        precision: Some(StopTimePrecision::Estimated),
                    },
                    objects::StopTime {
                        stop_point_idx: collections.stop_points.get_idx("sp:04").unwrap(),
                        sequence: 4,
                        arrival_time: Time::new(6, 8, 27),
                        departure_time: Time::new(6, 8, 27),
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 2,
                        drop_off_type: 1,
                        local_zone_id: None,
                        precision: Some(StopTimePrecision::Exact),
                    },
                    objects::StopTime {
                        stop_point_idx: collections.stop_points.get_idx("sp:05").unwrap(),
                        sequence: 5,
                        arrival_time: Time::new(6, 9, 27),
                        departure_time: Time::new(6, 9, 27),
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 2,
                        drop_off_type: 1,
                        local_zone_id: None,
                        precision: Some(StopTimePrecision::Estimated),
                    },
                ],
                collections.vehicle_journeys.into_vec()[0].stop_times
            );
        });
    }
    #[test]
    fn company_object_codes() {
        test_in_tmp_dir(|path| {
            let _ = generate_minimal_ntfs(path);
            let object_codes_content = "object_type,object_id,object_system,object_code\n\
            company,company_1,source,source_code";
            create_file_with_content(path, "object_codes.txt", object_codes_content);

            let mut collections = make_collection(path);
            let mut handler = PathFileHandler::new(path.to_path_buf());
            manage_codes(&mut collections, &mut handler).unwrap();

            let company = collections.companies.values().next().unwrap();
            assert_eq!(company.codes.len(), 1);
            let code = company.codes.iter().next().unwrap();
            assert_eq!(code.0, "source");
            assert_eq!(code.1, "source_code");
        });
    }
}
