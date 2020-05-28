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

use super::{
    attribute_with::AttributeWith,
    calendars::{self, DayTypes},
    common, lines,
    lines::LineNetexIDF,
    modes::MODES,
    stops,
};
use crate::{
    model::Collections,
    netex_utils::{self, FrameType},
    objects::{
        Calendar, Dataset, Date, KeysValues, Route, StopPoint, StopTime, Time, ValidityPeriod,
        VehicleJourney,
    },
    validity_period, Result,
};
use failure::{bail, format_err, ResultExt};
use log::{info, warn, Level as LogLevel};
use minidom::Element;
use minidom_ext::{AttributeElementExt, OnlyChildElementExt};
use skip_error::skip_error_and_log;
use std::{
    collections::{BTreeSet, HashMap},
    convert::TryFrom,
    fs::File,
    io::Read,
    path::Path,
};
use typed_index_collection::{CollectionWithId, Idx};
use walkdir::WalkDir;

pub const CALENDARS_FILENAME: &str = "calendriers.xml";
pub const COMMON_FILENAME: &str = "commun.xml";
pub const NETEX_STRUCTURE: &str = "NETEX_STRUCTURE";
pub const NETEX_SCHEDULE: &str = "NETEX_HORAIRE";
pub const NETEX_CALENDAR: &str = "NETEX_CALENDRIER";
pub const NETEX_COMMON: &str = "NETEX_COMMUN";

#[derive(Debug, Eq, Hash, PartialEq)]
pub enum GeneralFrameType {
    Structure,
    Schedule,
    Calendar,
    Common,
}
type GeneralFrames<'a> = HashMap<GeneralFrameType, &'a Element>;
type VehicleJourneyStopAssignment = HashMap<(String, String), Idx<StopPoint>>;
type VirtualStopPoint = StopPoint;

impl std::fmt::Display for GeneralFrameType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Structure => write!(f, "{}", NETEX_STRUCTURE),
            Self::Schedule => write!(f, "{}", NETEX_SCHEDULE),
            Self::Calendar => write!(f, "{}", NETEX_CALENDAR),
            Self::Common => write!(f, "{}", NETEX_COMMON),
        }
    }
}

struct DestinationDisplay {
    front_text: Option<String>,
    public_code: Option<String>,
}
type DestinationDisplays = HashMap<String, DestinationDisplay>;
#[derive(Debug, Clone)]
struct StopPointInJourneyPattern {
    stop_point_idx: Idx<StopPoint>,
    scheduled_stop_point_ref: String,
    pickup_type: u8,
    drop_off_type: u8,
    local_zone_id: Option<u16>,
}
struct JourneyPattern<'a, 'b> {
    route: &'a Route,
    destination_display: Option<&'b DestinationDisplay>,
    stop_points_in_journey_pattern: Vec<StopPointInJourneyPattern>,
}
type JourneyPatterns<'a, 'b> = HashMap<String, JourneyPattern<'a, 'b>>;

pub fn parse_general_frame_by_type<'a>(
    general_frames: &[&'a Element],
) -> Result<GeneralFrames<'a>> {
    fn extract_general_frame_type(general_frame: &Element) -> Result<GeneralFrameType> {
        let type_of_frame_ref: String = general_frame
            .try_only_child("TypeOfFrameRef")?
            .try_attribute("ref")?;
        if type_of_frame_ref.contains(NETEX_STRUCTURE) {
            return Ok(GeneralFrameType::Structure);
        }
        if type_of_frame_ref.contains(NETEX_SCHEDULE) {
            return Ok(GeneralFrameType::Schedule);
        }
        if type_of_frame_ref.contains(NETEX_CALENDAR) {
            return Ok(GeneralFrameType::Calendar);
        }
        if type_of_frame_ref.contains(NETEX_COMMON) {
            return Ok(GeneralFrameType::Common);
        }
        bail!("Failed to identify the type of this GeneralFrame")
    }
    general_frames
        .iter()
        .try_fold(HashMap::new(), |mut map, general_frame| {
            let general_frame_type = extract_general_frame_type(general_frame)?;
            if map.contains_key(&general_frame_type) {
                bail!("Multiple GeneralFrame of type {}", general_frame_type);
            }
            map.insert(general_frame_type, *general_frame);
            Ok(map)
        })
}

impl TryFrom<&Element> for Route {
    type Error = failure::Error;
    fn try_from(route_element: &Element) -> Result<Route> {
        if route_element.name() != "Route" {
            bail!(
                "Failed to convert a {} node into a Route",
                route_element.name()
            );
        }
        let raw_route_id = route_element.try_attribute("id")?;
        let id = route_element.try_attribute_with("id", extract_route_id)?;
        let line_id = route_element
            .try_only_child("LineRef")?
            .try_attribute_with("ref", lines::extract_line_id)?;
        let name = route_element
            .try_only_child("Name")?
            .text()
            .trim()
            .to_string();
        let direction_type = route_element
            .only_child("DirectionType")
            .map(|direction_type| direction_type.text().trim().to_string());
        let mut codes = KeysValues::default();
        codes.insert((String::from("source"), raw_route_id));
        let route = Route {
            id,
            line_id,
            name,
            direction_type,
            codes,
            ..Default::default()
        };
        Ok(route)
    }
}

fn extract_route_id(raw_id: &str) -> Result<String> {
    let error = || format_err!("Cannot extract Route identifier from '{}'", raw_id);
    let indices: Vec<_> = raw_id.match_indices(':').collect();
    let operator_right_bound = indices.get(0).ok_or_else(error)?.0;
    let id_left_bound = indices.get(1).ok_or_else(error)?.0 + 1;
    let id_right_bound = indices.get(2).ok_or_else(error)?.0;
    let operator = &raw_id[0..operator_right_bound];
    let id = &raw_id[id_left_bound..id_right_bound];
    Ok(format!("{}:{}", operator, id))
}

fn extract_vehicle_journey_id(raw_id: &str) -> Result<String> {
    let error = || {
        format_err!(
            "Cannot extract Vehicle Journey identifier from '{}'",
            raw_id
        )
    };
    let indices: Vec<_> = raw_id.match_indices(':').collect();
    let operator_right_bound = indices.get(0).ok_or_else(error)?.0;
    let id_left_bound = indices.get(1).ok_or_else(error)?.0 + 1;
    let id_right_bound = indices.get(2).ok_or_else(error)?.0;
    let operator = &raw_id[0..operator_right_bound];
    let id = &raw_id[id_left_bound..id_right_bound];
    Ok(format!("{}:{}", operator, id))
}

pub fn read_offer_folder(
    offer_folder: &Path,
    collections: &mut Collections,
    lines_netex_idf: &CollectionWithId<LineNetexIDF>,
    virtual_stop_points: &CollectionWithId<VirtualStopPoint>,
) -> Result<()> {
    let calendars_path = offer_folder.join(CALENDARS_FILENAME);
    let (map_daytypes, validity_period) = if calendars_path.exists() {
        let mut calendars_file = File::open(&calendars_path)
            .with_context(|_| format!("Error reading {:?}", calendars_path))?;
        let mut calendars_file_content = String::new();
        calendars_file.read_to_string(&mut calendars_file_content)?;
        let calendars: Element = calendars_file_content
            .parse()
            .map_err(|_| format_err!("Failed to open {}", calendars_path.display()))?;
        info!("Reading {}", calendars_path.display());
        calendars::parse_calendars(&calendars)?
    } else {
        warn!(
            "Offer {} ignored because it does not contain the '{}' file.",
            offer_folder.display(),
            CALENDARS_FILENAME
        );
        return Ok(());
    };

    let common_path = offer_folder.join(COMMON_FILENAME);
    let comments = if common_path.exists() {
        let mut common_file = File::open(&common_path)
            .with_context(|_| format!("Error reading {:?}", common_path))?;
        let mut common_file_content = String::new();
        common_file.read_to_string(&mut common_file_content)?;
        let common: Element = common_file_content
            .parse()
            .map_err(|_| format_err!("Failed to open {}", common_path.display()))?;
        info!("Reading {}", common_path.display());
        common::parse_common(&common)?
    } else {
        CollectionWithId::default()
    };
    collections.comments.try_merge(comments)?;

    for offer_entry in WalkDir::new(offer_folder)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|dir_entry| dir_entry.file_type().is_file())
        .filter(|dir_entry| {
            dir_entry
                .path()
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .map(|filename| filename.starts_with("offre_"))
                .unwrap_or_default()
        })
    {
        let offer_path = offer_entry.path();
        let mut offer_file =
            File::open(offer_path).with_context(|_| format!("Error reading {:?}", offer_path))?;
        let mut offer_file_content = String::new();
        offer_file.read_to_string(&mut offer_file_content)?;
        let offer: Element = offer_file_content
            .parse()
            .map_err(|_| format_err!("Failed to open {}", offer_path.display()))?;
        info!("Reading {}", offer_path.display());
        let (routes, vehicle_journeys, calendars) = skip_error_and_log!(
            parse_offer(
                &offer,
                collections,
                lines_netex_idf,
                &map_daytypes,
                virtual_stop_points
            )
            .map_err(|e| format_err!("Skip file {}: {}", offer_path.display(), e)),
            LogLevel::Warn
        );
        collections.routes.try_merge(routes)?;
        collections.vehicle_journeys.try_merge(vehicle_journeys)?;
        collections.calendars.try_merge(calendars)?;
        collections.datasets =
            update_validity_period_from_netex_idf(&mut collections.datasets, &validity_period)?;
    }
    Ok(())
}

fn apply_routing_constraint_zones(
    stop_points: &CollectionWithId<StopPoint>,
    stop_points_in_journey_pattern: &mut Vec<StopPointInJourneyPattern>,
    routing_constraint_zones: &[StopPointsSet],
) {
    fn flatten_stop_points_ids(
        stop_points: &CollectionWithId<StopPoint>,
        stop_points_idx: &[Idx<StopPoint>],
    ) -> String {
        stop_points_idx
            .iter()
            .map(|sp_idx| format!("/{}/", stop_points[*sp_idx].id.clone()))
            .collect()
    }

    let stop_points_idx_in_jp: StopPointsSet = stop_points_in_journey_pattern
        .iter()
        .map(|e| e.stop_point_idx)
        .collect();
    let stop_points_in_jp_flattened = flatten_stop_points_ids(stop_points, &stop_points_idx_in_jp);
    let mut id_incr = 1;
    for routing_constraint_zone in routing_constraint_zones {
        let stop_points_in_constraint_flattened =
            flatten_stop_points_ids(stop_points, &routing_constraint_zone);
        if stop_points_in_jp_flattened.contains(&stop_points_in_constraint_flattened) {
            for stop_point_in_journey_pattern in stop_points_in_journey_pattern
                .iter_mut()
                .filter(|sp| routing_constraint_zone.contains(&sp.stop_point_idx))
            {
                stop_point_in_journey_pattern.local_zone_id = Some(id_incr);
            }
            id_incr += 1;
        }
    }
}

fn parse_service_journey_patterns<'a, 'b, 'c, I>(
    sjp_elements: I,
    routes: &'b CollectionWithId<Route>,
    stop_points: &CollectionWithId<StopPoint>,
    destination_displays: &'c DestinationDisplays,
    map_schedule_stop_point_quay: &HashMap<String, String>,
    routing_constraint_zones: &[StopPointsSet],
) -> JourneyPatterns<'b, 'c>
where
    I: Iterator<Item = &'a Element>,
{
    fn parse_stop_point_in_journey_pattern(
        stop_point_in_journey_pattern_element: &Element,
        stop_points: &CollectionWithId<StopPoint>,
        map_schedule_stop_point_quay: &HashMap<String, String>,
    ) -> Result<StopPointInJourneyPattern> {
        let stop_point_idx = stop_point_idx(
            stop_point_in_journey_pattern_element,
            stop_points,
            map_schedule_stop_point_quay,
        )
        .map_err(|err| format_err!("impossible to get the stop point: {}", err))?;
        let scheduled_stop_point_ref: String = stop_point_in_journey_pattern_element
            .try_only_child("ScheduledStopPointRef")
            .and_then(|ssp_ref_el| ssp_ref_el.try_attribute::<String>("ref"))?;
        let pickup_type = boarding_type(stop_point_in_journey_pattern_element, "ForBoarding");
        let drop_off_type = boarding_type(stop_point_in_journey_pattern_element, "ForAlighting");
        Ok(StopPointInJourneyPattern {
            stop_point_idx,
            scheduled_stop_point_ref,
            pickup_type,
            drop_off_type,
            local_zone_id: None,
        })
    }
    sjp_elements
        .filter_map(|sjp_element| {
            let id: String = sjp_element.attribute("id")?;
            let route = sjp_element
                .only_child("RouteRef")?
                .attribute_with::<_, _, String>("ref", extract_route_id)
                .and_then(|route_ref| routes.get(&route_ref))?;
            let destination_display = sjp_element
                .only_child("DestinationDisplayRef")
                .and_then(|dd_ref| dd_ref.attribute::<String>("ref"))
                .and_then(|dd_ref| destination_displays.get(&dd_ref));
            let mut stop_points_in_journey_pattern = match sjp_element
                .only_child("pointsInSequence")?
                .children()
                .map(|sp_in_jp| {
                    parse_stop_point_in_journey_pattern(
                        sp_in_jp,
                        stop_points,
                        map_schedule_stop_point_quay,
                    )
                })
                .collect::<Result<_>>()
            {
                Ok(stop_points_in_journey_pattern) => stop_points_in_journey_pattern,
                Err(e) => {
                    warn!("{}", e);
                    return None;
                }
            };
            apply_routing_constraint_zones(
                stop_points,
                &mut stop_points_in_journey_pattern,
                routing_constraint_zones,
            );
            Some((
                id,
                JourneyPattern {
                    route,
                    destination_display,
                    stop_points_in_journey_pattern,
                },
            ))
        })
        .collect()
}

fn parse_destination_display<'a, I>(dd_elements: I) -> DestinationDisplays
where
    I: Iterator<Item = &'a Element>,
{
    dd_elements
        .filter_map(|dd_element| {
            let id: String = dd_element.attribute("id")?;
            let front_text = dd_element.only_child("FrontText").map(Element::text);
            let public_code = dd_element.only_child("PublicCode").map(Element::text);
            Some((
                id,
                DestinationDisplay {
                    front_text,
                    public_code,
                },
            ))
        })
        .collect()
}

type StopPointsSet = Vec<Idx<StopPoint>>;

fn parse_routing_constraint_zones<'a, I>(
    rcz_elements: I,
    map_schedule_stop_point_quay: &HashMap<String, String>,
    collections: &Collections,
) -> Vec<StopPointsSet>
where
    I: Iterator<Item = &'a Element>,
{
    rcz_elements
        .filter_map(|rcz_element| {
            let mut stop_point_idxs = StopPointsSet::new();
            for scheduled_stop_point_ref_element in rcz_element
                .only_child("members")
                .iter()
                .flat_map(|members| members.children())
            {
                if let Some(stop_point_idx) = scheduled_stop_point_ref_element
                    .attribute::<String>("ref")
                    .and_then(|ssp_ref| map_schedule_stop_point_quay.get(&ssp_ref))
                    .and_then(|quay_ref| collections.stop_points.get_idx(&quay_ref))
                {
                    stop_point_idxs.push(stop_point_idx);
                } else {
                    return None;
                }
            }
            if stop_point_idxs.is_empty() {
                None
            } else {
                Some(stop_point_idxs)
            }
        })
        .collect()
}

pub fn get_stop_point(
    stop_point_id: &str,
    stop_points: &mut CollectionWithId<StopPoint>,
    virtual_stop_points: &CollectionWithId<VirtualStopPoint>,
) -> Result<Idx<StopPoint>> {
    let stop_point_idx = if let Some(sp_idx) = stop_points.get_idx(&stop_point_id) {
        sp_idx
    } else {
        let virtual_stop_point = virtual_stop_points
            .get(&stop_point_id)
            .ok_or_else(|| format_err!("Failed to find StopPoint {}", stop_point_id))?;
        stop_points.push(virtual_stop_point.to_owned())?
    };
    Ok(stop_point_idx)
}

fn parse_passenger_stop_assignment<'a, I>(
    psa_elements: I,
    stop_points: &mut CollectionWithId<StopPoint>,
    virtual_stop_points: &CollectionWithId<VirtualStopPoint>,
) -> HashMap<String, String>
where
    I: Iterator<Item = &'a Element>,
{
    psa_elements
        .filter_map(|psa_element| {
            let psa_id: String = psa_element.attribute("id")?;
            let scheduled_stop_point_ref: String = psa_element
                .only_child("ScheduledStopPointRef")?
                .attribute("ref")?;
            let get_quay_ref = |element: &Element| -> Option<String> {
                element.only_child("QuayRef").and_then(|quay_ref_el| {
                    quay_ref_el.attribute_with("ref", stops::extract_quay_id)
                })
            };
            let mut get_stop_place_ref = |element: &Element| -> Option<String> {
                element
                    .only_child("StopPlaceRef")
                    .and_then(|stop_place_ref_el| {
                        stop_place_ref_el.attribute_with::<_, _, String>(
                            "ref",
                            stops::extract_monomodal_stop_place_id,
                        )
                    })
                    .map(|spr| get_stop_point(&spr, stop_points, virtual_stop_points))?
                    .map(|sp_idx| stop_points[sp_idx].id.clone())
                    // We only want to WARN about the error so
                    // the `.map_err` doesn't have to return the error
                    .map_err(|e| warn!("{}", e))
                    // Error is ignored here
                    .ok()
            };
            get_quay_ref(psa_element)
                .or_else(|| get_stop_place_ref(psa_element))
                .map(|stop_id| (scheduled_stop_point_ref, stop_id))
                .or_else(|| {
                    warn!(
                        "Missing QuayRef or StopPlaceRef node in PassengerStopAssignment {}",
                        psa_id
                    );
                    None
                })
        })
        .collect()
}

fn parse_vehicle_journey_stop_assignment<'a, I>(
    vjsa_elements: I,
    stop_points: &CollectionWithId<StopPoint>,
) -> VehicleJourneyStopAssignment
where
    I: Iterator<Item = &'a Element>,
{
    vjsa_elements
        .filter_map(|vjsa_element| {
            let vjsa_id: String = vjsa_element.attribute("id")?;
            let vehicle_journey_ref: String = vjsa_element
                .only_child("VehicleJourneyRef")?
                .attribute("ref")?;
            let scheduled_stop_point_ref: String = vjsa_element
                .only_child("ScheduledStopPointRef")?
                .attribute("ref")?;
            let quay_ref: String = vjsa_element
                .only_child("QuayRef")?
                .attribute_with("ref", stops::extract_quay_id)?;
            if let Some(stop_point_idx) = stop_points.get_idx(&quay_ref) {
                Some((
                    (vehicle_journey_ref, scheduled_stop_point_ref),
                    stop_point_idx,
                ))
            } else {
                warn!(
                    "Failed to find Quay {} in VehicleJourneyStopAssignment {}",
                    quay_ref, vjsa_id
                );
                None
            }
        })
        .collect()
}

fn parse_routes<'a, I>(
    route_elements: I,
    collections: &Collections,
) -> Result<CollectionWithId<Route>>
where
    I: Iterator<Item = &'a Element>,
{
    let mut routes = CollectionWithId::default();
    for route_element in route_elements {
        let route = skip_error_and_log!(Route::try_from(route_element), LogLevel::Warn);
        if !collections.lines.contains_id(&route.line_id) {
            warn!(
                "Failed to create route {} because line {} doesn't exist.",
                route.id, route.line_id
            );
            continue;
        }
        routes.push(route)?;
    }
    Ok(routes)
}

fn update_validity_period_from_netex_idf(
    datasets: &mut CollectionWithId<Dataset>,
    validity_period: &ValidityPeriod,
) -> Result<CollectionWithId<Dataset>> {
    let mut datasets = datasets.take();
    for dataset in &mut datasets {
        validity_period::set_dataset_validity_period(dataset, &validity_period);
    }
    CollectionWithId::new(datasets).map_err(|e| format_err!("{}", e))
}

fn enhance_with_object_code(
    routes: &CollectionWithId<Route>,
    journey_patterns: &JourneyPatterns,
) -> CollectionWithId<Route> {
    let mut enhanced_routes = CollectionWithId::default();
    let map_routes_journeypatterns: HashMap<&String, Vec<String>> =
        journey_patterns
            .iter()
            .fold(HashMap::new(), |mut mrjp, (jp_id, journey_pattern)| {
                mrjp.entry(&journey_pattern.route.id)
                    .or_insert_with(Vec::new)
                    .push(jp_id.clone());
                mrjp
            });

    for route in routes.values() {
        let journey_patterns_ref = skip_error_and_log!(
            map_routes_journeypatterns.get(&route.id).ok_or_else(|| {
                format_err!(
                    "Route {} doesn't have any ServiceJourneyPattern associated",
                    route.id
                )
            }),
            LogLevel::Warn
        );
        let mut codes = KeysValues::default();
        for journey_pattern_ref in journey_patterns_ref {
            codes.insert((
                "Netex_ServiceJourneyPattern".into(),
                journey_pattern_ref.clone(),
            ));
        }
        let mut route = route.clone();
        route.codes.extend(codes);
        // We are inserting only routes that were already in a 'CollectionWithId'
        enhanced_routes.push(route).unwrap();
    }
    enhanced_routes
}
// Representing N days
struct Days(u32);
impl From<Days> for Time {
    fn from(day: Days) -> Self {
        Time::new(24 * day.0, 0, 0)
    }
}

impl std::ops::Sub<Days> for Time {
    type Output = Self;
    fn sub(self, rhs: Days) -> Self::Output {
        self - Time::from(rhs)
    }
}

fn arrival_departure_times(el: &Element) -> Result<(Time, Time)> {
    fn time(el: &Element, node_name: &str) -> Result<Time> {
        Ok(el.try_only_child(node_name)?.text().parse()?)
    }
    let offset: u32 = el
        .try_only_child("DepartureDayOffset")?
        .text()
        .parse()
        .unwrap_or(0);
    let departure_offset_time: Time = Days(offset).into();

    let arrival_time = time(el, "ArrivalTime")?;
    let departure_time = time(el, "DepartureTime")?;

    let arrival_offset_time = if arrival_time.total_seconds() > departure_time.total_seconds() {
        departure_offset_time - Days(1)
    } else {
        departure_offset_time
    };

    Ok((
        arrival_time + arrival_offset_time,
        departure_time + departure_offset_time,
    ))
}

fn boarding_type(el: &Element, node_name: &str) -> u8 {
    el.only_child(node_name)
        .and_then(|node| node.text().parse::<bool>().ok())
        .map(|val| !val)
        .map(u8::from)
        .unwrap_or(0)
}

fn stop_point_idx(
    sp_in_jp: &Element,
    stop_points: &CollectionWithId<StopPoint>,
    map_schedule_stop_point_quay: &HashMap<String, String>,
) -> Result<Idx<StopPoint>> {
    sp_in_jp
        .try_only_child("ScheduledStopPointRef")
        .and_then(|ssp_ref_el| ssp_ref_el.try_attribute::<String>("ref"))
        .map_err(|e| format_err!("{}", e))
        .and_then(|ssp_ref| {
            map_schedule_stop_point_quay.get(&ssp_ref).ok_or_else(|| {
                format_err!(
                    "QuayRef corresponding to ScheduledStopPointRef {} not found",
                    ssp_ref
                )
            })
        })
        .and_then(|quay_ref| {
            stop_points
                .get_idx(quay_ref)
                .ok_or_else(|| format_err!("stop point {} not found", quay_ref))
        })
}

fn stop_times(
    service_journey_element: &Element,
    journey_patterns: &JourneyPatterns,
    map_vj_schedule_stop_point_quay: &VehicleJourneyStopAssignment,
) -> Result<Vec<StopTime>> {
    let service_journey_id: String = service_journey_element.try_attribute("id")?;
    let timetable_passing_times = service_journey_element
        .only_child("passingTimes")
        .into_iter()
        .flat_map(|el| el.children());
    let journey_pattern_ref: String = service_journey_element
        .try_only_child("JourneyPatternRef")?
        .try_attribute("ref")?;
    let stop_points_in_journey_pattern = journey_patterns
        .get(&journey_pattern_ref)
        .into_iter()
        .flat_map(|journey_pattern| &journey_pattern.stop_points_in_journey_pattern);

    let stop_times: Vec<_> = timetable_passing_times
        .zip(stop_points_in_journey_pattern)
        .enumerate()
        .map(|(sequence, (tpt, stop_point_in_journey_pattern))| {
            let StopPointInJourneyPattern {
                stop_point_idx,
                scheduled_stop_point_ref,
                pickup_type,
                drop_off_type,
                local_zone_id,
            } = stop_point_in_journey_pattern.to_owned();
            let stop_point_idx = if let Some(new_stop_point_idx) = map_vj_schedule_stop_point_quay
                .get(&(service_journey_id.to_string(), scheduled_stop_point_ref))
            {
                // Change StopPoint (idx) from virtual StopPoint
                // to a true StopPoint/Quay specified for this vehicle in the section VehicleJourneyStopAssignment
                *new_stop_point_idx
            } else {
                stop_point_idx
            };
            let times = arrival_departure_times(tpt)?;
            let stop_time = StopTime {
                id: None,
                stop_point_idx,
                sequence: sequence as u32,
                headsign: None,
                arrival_time: times.0,
                departure_time: times.1,
                boarding_duration: 0,
                alighting_duration: 0,
                pickup_type,
                drop_off_type,
                datetime_estimated: false,
                local_zone_id,
                precision: None,
                comment_links: None,
            };

            Ok(stop_time)
        })
        .collect::<Result<_>>()?;
    Ok(stop_times)
}

fn parse_vehicle_journeys<'a, I>(
    service_journey_elements: I,
    collections: &Collections,
    lines_netex_idf: &CollectionWithId<LineNetexIDF>,
    routes: &CollectionWithId<Route>,
    journey_patterns: &JourneyPatterns,
    map_daytypes: &DayTypes,
    map_vj_schedule_stop_point_quay: &VehicleJourneyStopAssignment,
) -> Result<(CollectionWithId<VehicleJourney>, CollectionWithId<Calendar>)>
where
    I: Iterator<Item = &'a Element>,
{
    fn parse_service_journey(
        service_journey_element: &Element,
        collections: &Collections,
        lines_netex_idf: &CollectionWithId<LineNetexIDF>,
        routes: &CollectionWithId<Route>,
        journey_patterns: &JourneyPatterns,
    ) -> Result<VehicleJourney> {
        let raw_vehicle_journey_id = service_journey_element.try_attribute("id")?;
        let id = service_journey_element.try_attribute_with("id", extract_vehicle_journey_id)?;
        let journey_pattern_ref: String = service_journey_element
            .try_only_child("JourneyPatternRef")?
            .try_attribute("ref")?;
        let journey_pattern_opt = journey_patterns.get(&journey_pattern_ref);
        let route_id = journey_pattern_opt
            .map(|journey_pattern| journey_pattern.route.id.clone())
            .ok_or_else(|| {
                format_err!("VehicleJourney {} doesn't have any Route associated", id)
            })?;
        let dataset_id = collections
            .datasets
            .values()
            .next()
            .map(|dataset| dataset.id.clone())
            .ok_or_else(|| format_err!("Failed to find a dataset"))?;
        let line_netex_idf = collections
            .routes
            .get(&route_id)
            .or_else(|| routes.get(&route_id))
            .and_then(|route| lines_netex_idf.get(&route.line_id))
            .ok_or_else(|| {
                format_err!("VehicleJourney {} doesn't have a corresponding line", id)
            })?;
        let company_id = service_journey_element
            .only_child("OperatorRef")
            .map(Element::text)
            .unwrap_or_else(|| line_netex_idf.company_id.clone());
        let physical_mode_id = MODES
            .get(&line_netex_idf.mode)
            .ok_or_else(|| format_err!("Mode {:?} doesn't exist", line_netex_idf.mode))?
            .physical_mode
            .0
            .to_string();
        let headsign = journey_pattern_opt
            .and_then(|journey_pattern| journey_pattern.destination_display)
            .and_then(|destination_display| destination_display.front_text.as_ref())
            .cloned();
        let short_name = journey_pattern_opt
            .and_then(|journey_pattern| journey_pattern.destination_display)
            .and_then(|destination_display| destination_display.public_code.as_ref())
            .cloned();
        let comment_links = service_journey_element
            .only_child("noticeAssignments")
            .iter()
            .flat_map(|notice_assignments_element| notice_assignments_element.children())
            .filter_map(|notice_assignment_element| {
                notice_assignment_element.only_child("NoticeRef")
            })
            .filter_map(|notice_ref_element| notice_ref_element.attribute::<String>("ref"))
            .filter_map(
                |notice_ref| match collections.comments.get_idx(&notice_ref) {
                    Some(comment_idx) => Some(comment_idx),
                    None => {
                        warn!("The comment with ID {} doesn't exist", notice_ref);
                        None
                    }
                },
            )
            .collect();
        let trip_property_id = line_netex_idf.trip_property_id.clone();
        let mut codes = KeysValues::default();
        codes.insert((String::from("source"), raw_vehicle_journey_id));
        let vehicle_journey = VehicleJourney {
            id,
            route_id,
            dataset_id,
            company_id,
            physical_mode_id,
            headsign,
            short_name,
            comment_links,
            trip_property_id,
            codes,
            ..Default::default()
        };
        Ok(vehicle_journey)
    }
    let mut vehicle_journeys = CollectionWithId::default();
    let mut calendars = CollectionWithId::default();
    let mut service_id = collections
        .calendars
        .values()
        .flat_map(|calendar| calendar.id.parse::<usize>().ok())
        .max()
        .unwrap_or(0);
    for service_journey_element in service_journey_elements {
        let vehicle_journey = skip_error_and_log!(
            parse_service_journey(
                service_journey_element,
                collections,
                lines_netex_idf,
                routes,
                journey_patterns,
            ),
            LogLevel::Warn
        );
        if !collections.routes.contains_id(&vehicle_journey.route_id)
            && !routes.contains_id(&vehicle_journey.route_id)
        {
            warn!(
                "Failed to create vehicle journey {} because route {} doesn't exist.",
                vehicle_journey.id, vehicle_journey.route_id
            );
            continue;
        }

        let stop_times = skip_error_and_log!(
            stop_times(
                service_journey_element,
                &journey_patterns,
                &map_vj_schedule_stop_point_quay
            ),
            LogLevel::Warn
        );
        if stop_times.is_empty() {
            warn!(
                "no stop times for vehicle journey {} found",
                vehicle_journey.id
            );
            continue;
        }

        let dates: BTreeSet<Date> = service_journey_element
            .only_child("dayTypes")
            .iter()
            .flat_map(|day_types| day_types.children())
            .filter_map(|day_type_ref| day_type_ref.attribute::<String>("ref"))
            .filter_map(|day_type_ref| map_daytypes.get(&day_type_ref))
            .flatten()
            .cloned()
            .collect();
        if dates.is_empty() {
            warn!(
                "Vehicle Journey {} doesn't have any date for the service",
                vehicle_journey.id
            );
            continue;
        }
        service_id += 1;
        calendars.push(Calendar {
            id: service_id.to_string(),
            dates,
        })?;

        let vehicle_journey = VehicleJourney {
            stop_times,
            service_id: service_id.to_string(),
            ..vehicle_journey
        };
        vehicle_journeys.push(vehicle_journey)?;
    }
    Ok((vehicle_journeys, calendars))
}

fn parse_offer(
    offer: &Element,
    collections: &mut Collections,
    lines_netex_idf: &CollectionWithId<LineNetexIDF>,
    map_daytypes: &DayTypes,
    virtual_stop_points: &CollectionWithId<VirtualStopPoint>,
) -> Result<(
    CollectionWithId<Route>,
    CollectionWithId<VehicleJourney>,
    CollectionWithId<Calendar>,
)> {
    let frames = netex_utils::parse_frames_by_type(
        offer
            .try_only_child("dataObjects")?
            .try_only_child("CompositeFrame")?
            .try_only_child("frames")?,
    )?;
    let general_frames = parse_general_frame_by_type(frames.get(&FrameType::General).unwrap())?;
    let structure_frame = general_frames
        .get(&GeneralFrameType::Structure)
        .ok_or_else(|| {
            format_err!(
                "Failed to find the GeneralFrame of type {}",
                NETEX_STRUCTURE
            )
        })?;
    let schedule_frame = general_frames
        .get(&GeneralFrameType::Schedule)
        .ok_or_else(|| format_err!("Failed to find the GeneralFrame of type {}", NETEX_SCHEDULE))?;

    let map_schedule_stop_point_quay = structure_frame
        .only_child("members")
        .map(Element::children)
        .map(|childrens| childrens.filter(|e| e.name() == "PassengerStopAssignment"))
        .map(|psa_elements| {
            parse_passenger_stop_assignment(
                psa_elements,
                &mut collections.stop_points,
                virtual_stop_points,
            )
        })
        .unwrap_or_else(HashMap::new);

    let map_vj_schedule_stop_point_quay = schedule_frame
        .only_child("members")
        .map(Element::children)
        .map(|childrens| childrens.filter(|e| e.name() == "VehicleJourneyStopAssignment"))
        .map(|vjsa_elements| {
            parse_vehicle_journey_stop_assignment(vjsa_elements, &collections.stop_points)
        })
        .unwrap_or_else(HashMap::new);

    let routes = structure_frame
        .only_child("members")
        .map(Element::children)
        .map(|childrens| childrens.filter(|e| e.name() == "Route"))
        .map(|route_elements| parse_routes(route_elements, collections))
        .transpose()?
        .unwrap_or_else(CollectionWithId::default);
    let destination_displays = structure_frame
        .only_child("members")
        .map(Element::children)
        .map(|childrens| childrens.filter(|e| e.name() == "DestinationDisplay"))
        .map(parse_destination_display)
        .unwrap_or_else(HashMap::new);
    let routing_constraint_zones = structure_frame
        .only_child("members")
        .map(Element::children)
        .map(|childrens| childrens.filter(|e| e.name() == "RoutingConstraintZone"))
        .map(|rcz_elements| {
            parse_routing_constraint_zones(
                rcz_elements,
                &map_schedule_stop_point_quay,
                &collections,
            )
        })
        .unwrap_or_else(Vec::new);
    let journey_patterns = structure_frame
        .only_child("members")
        .map(Element::children)
        .map(|childrens| childrens.filter(|e| e.name() == "ServiceJourneyPattern"))
        .map(|sjp_elements| {
            parse_service_journey_patterns(
                sjp_elements,
                &routes,
                &collections.stop_points,
                &destination_displays,
                &map_schedule_stop_point_quay,
                &routing_constraint_zones,
            )
        })
        .unwrap_or_else(HashMap::new);
    let routes = enhance_with_object_code(&routes, &journey_patterns);
    let (vehicle_journeys, calendars) = schedule_frame
        .only_child("members")
        .map(Element::children)
        .map(|childrens| childrens.filter(|e| e.name() == "ServiceJourney"))
        .map(|service_journey_elements| {
            parse_vehicle_journeys(
                service_journey_elements,
                collections,
                lines_netex_idf,
                &routes,
                &journey_patterns,
                map_daytypes,
                &map_vj_schedule_stop_point_quay,
            )
        })
        .transpose()?
        .unwrap_or_else(|| (CollectionWithId::default(), CollectionWithId::default()));
    Ok((routes, vehicle_journeys, calendars))
}

#[cfg(test)]
mod tests {
    use super::*;

    mod parse_general_frame_by_type {
        use super::*;

        #[test]
        fn general_frames() {
            let xml_general_frame_1 = r#"<GeneralFrame>
                    <TypeOfFrameRef ref="FR100:TypeOfFrame:NETEX_STRUCTURE:"/>
               </GeneralFrame>"#;
            let xml_general_frame_2 = r#"<GeneralFrame>
                    <TypeOfFrameRef ref="FR100:TypeOfFrame:NETEX_HORAIRE:"/>
               </GeneralFrame>"#;
            let general_frame_1: Element = xml_general_frame_1.parse().unwrap();
            let general_frame_2: Element = xml_general_frame_2.parse().unwrap();
            let general_frames =
                parse_general_frame_by_type(&[&general_frame_1, &general_frame_2]).unwrap();
            assert!(general_frames.contains_key(&GeneralFrameType::Schedule));
            assert!(general_frames.contains_key(&GeneralFrameType::Structure));
        }

        #[test]
        #[should_panic(expected = "Multiple GeneralFrame of type NETEX_STRUCTURE")]
        fn multiple_general_frames_of_same_type() {
            let xml_general_frame_1 = r#"<GeneralFrame>
                    <TypeOfFrameRef ref="FR100:TypeOfFrame:NETEX_STRUCTURE:"/>
               </GeneralFrame>"#;
            let xml_general_frame_2 = r#"<GeneralFrame>
                    <TypeOfFrameRef ref="FR100:TypeOfFrame:NETEX_STRUCTURE:"/>
               </GeneralFrame>"#;
            let general_frame_1: Element = xml_general_frame_1.parse().unwrap();
            let general_frame_2: Element = xml_general_frame_2.parse().unwrap();
            parse_general_frame_by_type(&[&general_frame_1, &general_frame_2]).unwrap();
        }

        #[test]
        #[should_panic(expected = "Failed to identify the type of this GeneralFrame")]
        fn incorrect_general_frame_type() {
            let xml_general_frame = r#"<GeneralFrame>
                    <TypeOfFrameRef ref="FR100:TypeOfFrame:NETEX_UNKNOWN_TYPE:"/>
               </GeneralFrame>"#;
            let general_frame: Element = xml_general_frame.parse().unwrap();
            parse_general_frame_by_type(&[&general_frame]).unwrap();
        }
    }

    mod parse_routes {
        use super::*;
        use crate::objects::Line;
        use pretty_assertions::assert_eq;

        #[test]
        fn routes() {
            let xml = r#"<Route id="stif:Route:route_id:">
                    <Name>Route name</Name>
                    <LineRef ref="FR:Line:line_id:" />
                    <DirectionType>inbound</DirectionType>
                </Route>"#;
            let root: Element = xml.parse().unwrap();
            let mut collections = Collections::default();
            collections
                .lines
                .push(Line {
                    id: String::from("line_id"),
                    ..Default::default()
                })
                .unwrap();
            let routes = parse_routes(vec![root].iter(), &collections).unwrap();
            let route = routes.get("stif:route_id").unwrap();
            assert_eq!("stif:route_id", route.id.as_str());
            assert_eq!("Route name", route.name.as_str());
            assert_eq!("line_id", route.line_id.as_str());
            assert_eq!("inbound", route.direction_type.as_ref().unwrap().as_str());
        }

        #[test]
        fn ignore_no_line_id() {
            let xml = r#"<Route id="route_id">
                    <Name>Route name</Name>
                    <LineRef ref="line_id" />
                    <DirectionType>inbound</DirectionType>
                </Route>"#;
            let root: Element = xml.parse().unwrap();
            let collections = Collections::default();
            let routes = parse_routes(vec![root].iter(), &collections).unwrap();
            assert_eq!(0, routes.len());
        }

        #[test]
        fn ignore_no_name() {
            let xml = r#"<Route id="route_id">
                    <LineRef ref="line_id" />
                    <DirectionType>inbound</DirectionType>
                </Route>"#;
            let root: Element = xml.parse().unwrap();
            let mut collections = Collections::default();
            collections
                .lines
                .push(Line {
                    id: String::from("line_id"),
                    ..Default::default()
                })
                .unwrap();
            let routes = parse_routes(vec![root].iter(), &collections).unwrap();
            assert_eq!(0, routes.len());
        }
    }

    mod enhance_with_object_code {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn add_object_code_1_route_2_journey_patterns() {
            let route = Route {
                id: String::from("route_id"),
                name: String::from("Route Name"),
                ..Default::default()
            };
            let routes = CollectionWithId::from(route);
            let journey_pattern1 = JourneyPattern {
                route: &routes.get("route_id").as_ref().unwrap(),
                destination_display: None,
                stop_points_in_journey_pattern: Vec::new(),
            };
            let journey_pattern2 = JourneyPattern {
                route: &routes.get("route_id").as_ref().unwrap(),
                destination_display: None,
                stop_points_in_journey_pattern: Vec::new(),
            };
            let mut journey_patterns = JourneyPatterns::default();
            journey_patterns.insert(
                String::from("service_journey_pattern_id1"),
                journey_pattern1,
            );
            journey_patterns.insert(
                String::from("service_journey_pattern_id2"),
                journey_pattern2,
            );
            let routes = enhance_with_object_code(&routes, &journey_patterns);
            let route = routes.get("route_id").unwrap();
            assert_eq!("Route Name", route.name.as_str());
            assert_eq!(2, route.codes.len());
            let mut codes = route.codes.iter();
            let code1 = codes.next().unwrap();
            assert_eq!("Netex_ServiceJourneyPattern", code1.0.as_str());
            assert_eq!("service_journey_pattern_id1", code1.1.as_str());
            let code2 = codes.next().unwrap();
            assert_eq!("Netex_ServiceJourneyPattern", code2.0.as_str());
            assert_eq!("service_journey_pattern_id2", code2.1.as_str());
        }

        #[test]
        fn add_object_code_keep_existing_route_code() {
            let mut source = KeysValues::default();
            source.insert((String::from("source"), String::from("route_source_id")));
            let route = Route {
                id: String::from("route_id"),
                name: String::from("Route Name"),
                codes: source,
                ..Default::default()
            };
            let routes = CollectionWithId::from(route);
            let journey_pattern = JourneyPattern {
                route: &routes.get("route_id").as_ref().unwrap(),
                destination_display: None,
                stop_points_in_journey_pattern: Vec::new(),
            };
            let mut journey_patterns = JourneyPatterns::default();
            journey_patterns.insert(String::from("service_journey_pattern_id"), journey_pattern);
            let routes = enhance_with_object_code(&routes, &journey_patterns);
            let route = routes.get("route_id").unwrap();
            assert_eq!(2, route.codes.len());
            let mut codes = route.codes.iter();
            let code1 = codes.next().unwrap();
            assert_eq!("Netex_ServiceJourneyPattern", code1.0.as_str());
            assert_eq!("service_journey_pattern_id", code1.1.as_str());
            let code2 = codes.next().unwrap();
            assert_eq!("source", code2.0.as_str());
            assert_eq!("route_source_id", code2.1.as_str());
        }

        #[test]
        fn no_associated_service_journey_pattern() {
            let route = Route {
                id: String::from("route_id"),
                name: String::from("Route Name"),
                ..Default::default()
            };
            let routes = CollectionWithId::from(route);
            let routes = enhance_with_object_code(&routes, &JourneyPatterns::default());
            assert_eq!(0, routes.len());
        }
    }

    mod parse_vehicle_journeys {
        use super::*;
        use crate::{
            netex_idf::modes::IDFMMode,
            objects::{Comment, CommentType, Dataset},
        };
        use pretty_assertions::assert_eq;

        fn collections() -> Collections {
            let mut collections = Collections::default();
            collections
                .datasets
                .push(Dataset {
                    id: String::from("dataset_id"),
                    ..Default::default()
                })
                .unwrap();
            collections
                .routes
                .push(Route {
                    id: String::from("stif:route_id"),
                    line_id: String::from("line_id"),
                    ..Default::default()
                })
                .unwrap();
            collections
                .comments
                .push(Comment {
                    id: String::from("comment_id"),
                    comment_type: CommentType::Information,
                    label: None,
                    name: String::from("Comment"),
                    url: None,
                })
                .unwrap();
            collections
        }

        fn service_journey() -> Element {
            let service_journey_xml = r#"<ServiceJourney id="stif:ServiceJourney:service_journey_id:">
                    <JourneyPatternRef ref="journey_pattern_id" />
                    <dayTypes>
                        <DayTypeRef ref="day_type_id_1" />
                        <DayTypeRef ref="day_type_id_2" />
                    </dayTypes>
                    <passingTimes>
                       <TimetabledPassingTime version="any">
                          <ArrivalTime>06:00:00</ArrivalTime>
                          <DepartureTime>06:00:00</DepartureTime>
                          <DepartureDayOffset>0</DepartureDayOffset>
                       </TimetabledPassingTime>
                    </passingTimes>
                    <noticeAssignments>
                       <NoticeAssignment>
                          <NoticeRef ref="comment_id" />
                       </NoticeAssignment>
                    </noticeAssignments>
                </ServiceJourney>"#;
            service_journey_xml.parse().unwrap()
        }

        fn destination_displays() -> DestinationDisplays {
            let mut destination_displays = DestinationDisplays::new();
            let destination_display = DestinationDisplay {
                front_text: Some(String::from("Trip Name")),
                public_code: Some(String::from("Trip Short Name")),
            };
            destination_displays
                .insert(String::from("destination_display_id"), destination_display);
            destination_displays
        }

        fn journey_patterns<'a, 'b>(
            routes: &'a CollectionWithId<Route>,
            destination_displays: &'b DestinationDisplays,
        ) -> JourneyPatterns<'a, 'b> {
            let mut journey_patterns = JourneyPatterns::new();
            let mut stop_points_in_journey_pattern = Vec::new();
            let stop_point_idx = CollectionWithId::from(StopPoint {
                id: String::from("stop_id"),
                ..Default::default()
            })
            .get_idx("stop_id")
            .unwrap();
            stop_points_in_journey_pattern.push(StopPointInJourneyPattern {
                stop_point_idx,
                scheduled_stop_point_ref: String::new(),
                pickup_type: 0,
                drop_off_type: 1,
                local_zone_id: None,
            });
            if let Some(route) = routes.get("stif:route_id") {
                let journey_pattern = JourneyPattern {
                    route,
                    destination_display: destination_displays.get("destination_display_id"),
                    stop_points_in_journey_pattern,
                };
                journey_patterns.insert(String::from("journey_pattern_id"), journey_pattern);
            }
            journey_patterns
        }

        fn lines_netex_idf() -> CollectionWithId<LineNetexIDF> {
            CollectionWithId::from(LineNetexIDF {
                id: String::from("line_id"),
                name: String::from("The Line"),
                code: None,
                source_code: String::from("FR:Line:line_id:"),
                private_code: None,
                network_id: String::from("network_id"),
                company_id: String::from("company_id"),
                mode: IDFMMode::Bus,
                color: None,
                text_color: None,
                comment_ids: BTreeSet::new(),
                trip_property_id: Some("tp_id".into()),
            })
        }

        fn day_types() -> DayTypes {
            let mut day_type_1 = BTreeSet::new();
            day_type_1.insert(Date::from_ymd(2019, 1, 1));
            let mut day_type_2 = BTreeSet::new();
            day_type_2.insert(Date::from_ymd(2019, 1, 2));
            let mut day_types = HashMap::new();
            day_types.insert(String::from("day_type_id_1"), day_type_1);
            day_types.insert(String::from("day_type_id_2"), day_type_2);
            day_types
        }

        #[test]
        #[allow(clippy::cognitive_complexity)]
        fn parse_vehicle_journey() {
            let service_journey_element = service_journey();
            let service_journey_xml = r#"<ServiceJourney id="stif:ServiceJourney:service_journey_id_1:">
                    <JourneyPatternRef ref="journey_pattern_id" />
                    <dayTypes>
                        <DayTypeRef ref="day_type_id_1" />
                        <DayTypeRef ref="day_type_id_2" />
                    </dayTypes>
                    <passingTimes>
                       <TimetabledPassingTime version="any">
                          <ArrivalTime>23:55:00</ArrivalTime>
                          <DepartureTime>00:05:00</DepartureTime>
                          <DepartureDayOffset>1</DepartureDayOffset>
                       </TimetabledPassingTime>
                    </passingTimes>
                </ServiceJourney>"#;
            let service_journey_element_1 = service_journey_xml.parse().unwrap();
            let lines_netex_idf = lines_netex_idf();
            let day_types = day_types();
            let collections = collections();
            let destination_displays = destination_displays();
            let journey_patterns = journey_patterns(&collections.routes, &destination_displays);
            let (vehicle_journeys, calendars) = parse_vehicle_journeys(
                vec![&service_journey_element, &service_journey_element_1].into_iter(),
                &collections,
                &lines_netex_idf,
                &CollectionWithId::default(),
                &journey_patterns,
                &day_types,
                &HashMap::default(),
            )
            .unwrap();

            assert_eq!(2, vehicle_journeys.len());
            let vehicle_journey = vehicle_journeys.get("stif:service_journey_id").unwrap();
            assert_eq!("stif:route_id", vehicle_journey.route_id.as_str());
            assert_eq!("dataset_id", vehicle_journey.dataset_id.as_str());
            assert_eq!("company_id", vehicle_journey.company_id.as_str());
            assert_eq!("Bus", vehicle_journey.physical_mode_id.as_str());
            assert_eq!("Trip Name", vehicle_journey.headsign.as_ref().unwrap());
            assert_eq!(
                "Trip Short Name",
                vehicle_journey.short_name.as_ref().unwrap()
            );
            assert!(vehicle_journey
                .comment_links
                .contains(&collections.comments.get_idx("comment_id").unwrap()));
            let stop_time = &vehicle_journey.stop_times[0];
            assert_eq!(0, stop_time.sequence);
            assert_eq!(Time::new(6, 0, 0), stop_time.arrival_time);
            assert_eq!(Time::new(6, 0, 0), stop_time.departure_time);
            assert_eq!(0, stop_time.boarding_duration);
            assert_eq!(0, stop_time.alighting_duration);
            assert_eq!(0, stop_time.pickup_type);
            assert_eq!(1, stop_time.drop_off_type);
            let vehicle_journey = vehicle_journeys.get("stif:service_journey_id_1").unwrap();
            assert_eq!("stif:route_id", vehicle_journey.route_id.as_str());
            assert_eq!("dataset_id", vehicle_journey.dataset_id.as_str());
            assert_eq!("company_id", vehicle_journey.company_id.as_str());
            assert_eq!("Bus", vehicle_journey.physical_mode_id.as_str());
            assert_eq!("Trip Name", vehicle_journey.headsign.as_ref().unwrap());
            assert_eq!(
                "Trip Short Name",
                vehicle_journey.short_name.as_ref().unwrap()
            );
            assert_eq!("tp_id", vehicle_journey.trip_property_id.as_ref().unwrap());
            let stop_time = &vehicle_journey.stop_times[0];
            assert_eq!(0, stop_time.sequence);
            assert_eq!(Time::new(23, 55, 0), stop_time.arrival_time);
            assert_eq!(Time::new(24, 5, 0), stop_time.departure_time);
            assert_eq!(0, stop_time.boarding_duration);
            assert_eq!(0, stop_time.alighting_duration);
            assert_eq!(0, stop_time.pickup_type);
            assert_eq!(1, stop_time.drop_off_type);

            assert_eq!(2, calendars.len());
            let calendar = calendars.get("1").unwrap();
            assert!(calendar.dates.contains(&Date::from_ymd(2019, 1, 1)));
            assert!(calendar.dates.contains(&Date::from_ymd(2019, 1, 2)));
            let calendar = calendars.get("2").unwrap();
            assert!(calendar.dates.contains(&Date::from_ymd(2019, 1, 1)));
            assert!(calendar.dates.contains(&Date::from_ymd(2019, 1, 2)));
        }

        #[test]
        fn ignore_vehicle_journey_without_journey_pattern() {
            let service_journey_element = service_journey();
            let lines_netex_idf = lines_netex_idf();
            let day_types = day_types();
            let collections = collections();
            let journey_patterns = JourneyPatterns::default();
            let (vehicle_journeys, calendars) = parse_vehicle_journeys(
                vec![service_journey_element].iter(),
                &collections,
                &lines_netex_idf,
                &CollectionWithId::default(),
                &journey_patterns,
                &day_types,
                &HashMap::default(),
            )
            .unwrap();
            assert_eq!(0, vehicle_journeys.len());
            assert_eq!(0, calendars.len());
        }

        #[test]
        fn ignore_vehicle_journey_without_route() {
            let service_journey_element = service_journey();
            let lines_netex_idf = lines_netex_idf();
            let day_types = day_types();
            let collections = Collections::default();
            let destination_displays = destination_displays();
            let journey_patterns = journey_patterns(&collections.routes, &destination_displays);
            let (vehicle_journeys, calendars) = parse_vehicle_journeys(
                vec![service_journey_element].iter(),
                &collections,
                &lines_netex_idf,
                &CollectionWithId::default(),
                &journey_patterns,
                &day_types,
                &HashMap::default(),
            )
            .unwrap();
            assert_eq!(0, vehicle_journeys.len());
            assert_eq!(0, calendars.len());
        }

        #[test]
        fn ignore_vehicle_journey_without_line() {
            let service_journey_element = service_journey();
            let lines_netex_idf = lines_netex_idf();
            let day_types = day_types();
            let mut collections = Collections::default();
            collections
                .routes
                .push(Route {
                    id: String::from("stif:route_id"),
                    line_id: String::from("unknown_line_id"),
                    ..Default::default()
                })
                .unwrap();
            let destination_displays = destination_displays();
            let journey_patterns = journey_patterns(&collections.routes, &destination_displays);
            let (vehicle_journeys, calendars) = parse_vehicle_journeys(
                vec![service_journey_element].iter(),
                &collections,
                &lines_netex_idf,
                &CollectionWithId::default(),
                &journey_patterns,
                &day_types,
                &HashMap::default(),
            )
            .unwrap();
            assert_eq!(0, vehicle_journeys.len());
            assert_eq!(0, calendars.len());
        }

        #[test]
        fn ignore_vehicle_journey_with_invalid_service_journey_no_id() {
            let service_journey_xml = r#"<ServiceJourney>
                    <JourneyPatternRef ref="journey_pattern_id" />
                    <dayTypes>
                        <DayTypeRef ref="day_type_id_1" />
                        <DayTypeRef ref="day_type_id_2" />
                    </dayTypes>
                    <passingTimes>
                       <TimetabledPassingTime version="any">
                          <ArrivalTime>06:00:00</ArrivalTime>
                          <DepartureTime>06:00:00</DepartureTime>
                          <DepartureDayOffset>0</DepartureDayOffset>
                       </TimetabledPassingTime>
                    </passingTimes>
                </ServiceJourney>"#;
            let service_journey_element: Element = service_journey_xml.parse().unwrap();
            let lines_netex_idf = lines_netex_idf();
            let day_types = day_types();
            let collections = Collections::default();
            let destination_displays = destination_displays();
            let journey_patterns = journey_patterns(&collections.routes, &destination_displays);
            let (vehicle_journeys, calendars) = parse_vehicle_journeys(
                vec![service_journey_element].iter(),
                &collections,
                &lines_netex_idf,
                &CollectionWithId::default(),
                &journey_patterns,
                &day_types,
                &HashMap::default(),
            )
            .unwrap();
            assert_eq!(0, vehicle_journeys.len());
            assert_eq!(0, calendars.len());
        }

        #[test]
        fn increment_service_id() {
            let service_journey_element = service_journey();
            let lines_netex_idf = lines_netex_idf();
            let day_types = day_types();
            let mut collections = collections();
            // There is already an existing service
            // (for example, from a previous call to 'parse_vehicle_journeys')
            collections
                .calendars
                .push(Calendar {
                    id: String::from("1"),
                    ..Default::default()
                })
                .unwrap();
            let destination_displays = destination_displays();
            let journey_patterns = journey_patterns(&collections.routes, &destination_displays);
            let (_, calendars) = parse_vehicle_journeys(
                vec![&service_journey_element].into_iter(),
                &collections,
                &lines_netex_idf,
                &CollectionWithId::default(),
                &journey_patterns,
                &day_types,
                &HashMap::default(),
            )
            .unwrap();

            assert_eq!(1, calendars.len());
            let calendar = calendars.get("2").unwrap();
            assert!(calendar.dates.contains(&Date::from_ymd(2019, 1, 1)));
            assert!(calendar.dates.contains(&Date::from_ymd(2019, 1, 2)));
        }
    }

    mod stop_times {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        #[should_panic]
        fn test_arrival_departure_times_invalid_xml() {
            let tpt_xml = r#"<TimetabledPassingTime version="any"></TimetabledPassingTime>"#;
            let tpt_el: Element = tpt_xml.parse().unwrap();
            arrival_departure_times(&tpt_el).unwrap();
        }

        #[test]
        fn test_arrival_departure_times_with_offset_0() {
            let tpt_xml = r#"<TimetabledPassingTime version="any">
                                        <ArrivalTime>01:30:00</ArrivalTime>
                                        <DepartureTime>01:32:00</DepartureTime>
                                        <DepartureDayOffset>0</DepartureDayOffset>
                                    </TimetabledPassingTime>"#;
            let tpt_el: Element = tpt_xml.parse().unwrap();
            let times = arrival_departure_times(&tpt_el).unwrap();

            let expected = (Time::new(1, 30, 0), Time::new(1, 32, 0));
            assert_eq!(expected, times);
        }

        #[test]
        fn test_arrival_departure_times_with_positive_offset() {
            let tpt_xml = r#"<TimetabledPassingTime version="any">
                                        <ArrivalTime>01:30:00</ArrivalTime>
                                        <DepartureTime>01:32:00</DepartureTime>
                                        <DepartureDayOffset>1</DepartureDayOffset>
                                    </TimetabledPassingTime>"#;
            let tpt_el: Element = tpt_xml.parse().unwrap();
            let times = arrival_departure_times(&tpt_el).unwrap();

            let expected = (Time::new(25, 30, 0), Time::new(25, 32, 00));
            assert_eq!(expected, times);
        }

        #[test]
        fn test_arrival_departure_times_with_negative_offset() {
            let tpt_xml = r#"<TimetabledPassingTime version="any">
                                        <ArrivalTime>01:30:00</ArrivalTime>
                                        <DepartureTime>01:32:00</DepartureTime>
                                        <DepartureDayOffset>-1</DepartureDayOffset>
                                    </TimetabledPassingTime>"#;
            let tpt_el: Element = tpt_xml.parse().unwrap();
            let times = arrival_departure_times(&tpt_el).unwrap();

            let expected = (Time::new(1, 30, 0), Time::new(1, 32, 0));
            assert_eq!(expected, times);
        }

        #[test]
        fn test_arrival_departure_times_passing_midnight() {
            let tpt_xml = r#"<TimetabledPassingTime version="any">
                                        <ArrivalTime>23:50:00</ArrivalTime>
                                        <DepartureTime>00:10:00</DepartureTime>
                                        <DepartureDayOffset>1</DepartureDayOffset>
                                    </TimetabledPassingTime>"#;
            let tpt_el: Element = tpt_xml.parse().unwrap();
            let times = arrival_departure_times(&tpt_el).unwrap();

            let expected = (Time::new(23, 50, 0), Time::new(24, 10, 0));
            assert_eq!(expected, times);
        }

        #[test]
        fn test_boarding_type_no_node() {
            let sp_in_jp_xml = r#"<StopPointInJourneyPattern>
                                    </StopPointInJourneyPattern>"#;
            let sp_in_jp_el: Element = sp_in_jp_xml.parse().unwrap();
            let boarding_type = boarding_type(&sp_in_jp_el, "unknown_node");
            assert_eq!(0, boarding_type);
        }

        #[test]
        fn test_boarding_type_node_true() {
            let sp_in_jp_xml = r#"<StopPointInJourneyPattern>
                                        <ForAlighting>true</ForAlighting>
                                    </StopPointInJourneyPattern>"#;
            let sp_in_jp_el: Element = sp_in_jp_xml.parse().unwrap();
            let boarding_type = boarding_type(&sp_in_jp_el, "ForAlighting");
            assert_eq!(0, boarding_type);
        }

        #[test]
        fn test_boarding_type_node_whatever() {
            let sp_in_jp_xml = r#"<StopPointInJourneyPattern>
                                        <ForAlighting>whatever</ForAlighting>
                                    </StopPointInJourneyPattern>"#;
            let sp_in_jp_el: Element = sp_in_jp_xml.parse().unwrap();
            let boarding_type = boarding_type(&sp_in_jp_el, "ForAlighting");
            assert_eq!(0, boarding_type);
        }

        #[test]
        fn test_boarding_type_node_false() {
            let sp_in_jp_xml = r#"<StopPointInJourneyPattern>
                                        <ForAlighting>false</ForAlighting>
                                    </StopPointInJourneyPattern>"#;
            let sp_in_jp_el: Element = sp_in_jp_xml.parse().unwrap();
            let boarding_type = boarding_type(&sp_in_jp_el, "ForAlighting");
            assert_eq!(1, boarding_type);
        }
    }

    mod parse_routing_constraint_zones {
        use super::*;
        use pretty_assertions::assert_eq;

        fn map_schedule_stop_point_quay() -> HashMap<String, String> {
            vec![
                (
                    String::from("scheduled_stop_point_ref"),
                    String::from("stop_point_id"),
                ),
                (
                    String::from("schedule_stop_point_ref_with_incorrect_stop_point_id"),
                    String::from("unknown_id"),
                ),
            ]
            .into_iter()
            .collect()
        }

        fn collections() -> Collections {
            let mut collections = Collections::default();
            collections
                .stop_points
                .push(StopPoint {
                    id: String::from("stop_point_id"),
                    ..Default::default()
                })
                .unwrap();
            collections
        }

        #[test]
        fn valid_routing_constraint_zone() {
            let routing_constraint_zone_xml = r#"<RoutingConstraintZone>
                    <members>
                        <ScheduledStopPointRef ref="scheduled_stop_point_ref" />
                    </members>
                </RoutingConstraintZone>"#;
            let routing_constraint_zone: Element = routing_constraint_zone_xml.parse().unwrap();
            let map_schedule_stop_point_quay = map_schedule_stop_point_quay();
            let collections = collections();
            let routing_constraint_zones = parse_routing_constraint_zones(
                [routing_constraint_zone].iter(),
                &map_schedule_stop_point_quay,
                &collections,
            );
            assert_eq!(1, routing_constraint_zones.len());
            let routing_constraint_zone = &routing_constraint_zones[0];
            let expected_idx = collections.stop_points.get_idx("stop_point_id").unwrap();
            assert_eq!(1, routing_constraint_zone.len());
            assert_eq!(expected_idx, routing_constraint_zone[0]);
        }

        #[test]
        fn invalid_scheduled_stop_point_ref() {
            let routing_constraint_zone_xml = r#"<RoutingConstraintZone>
                    <members>
                        <ScheduledStopPointRef ref="scheduled_stop_point_ref" />
                        <ScheduledStopPointRef ref="unknown_ref" />
                    </members>
                </RoutingConstraintZone>"#;
            let routing_constraint_zone: Element = routing_constraint_zone_xml.parse().unwrap();
            let map_schedule_stop_point_quay = map_schedule_stop_point_quay();
            let collections = collections();
            let routing_constraint_zones = parse_routing_constraint_zones(
                [routing_constraint_zone].iter(),
                &map_schedule_stop_point_quay,
                &collections,
            );
            assert_eq!(0, routing_constraint_zones.len());
        }

        #[test]
        fn routing_constraint_zone_without_members() {
            let routing_constraint_zone_xml = r#"<RoutingConstraintZone />"#;
            let routing_constraint_zone: Element = routing_constraint_zone_xml.parse().unwrap();
            let map_schedule_stop_point_quay = map_schedule_stop_point_quay();
            let collections = collections();
            let routing_constraint_zones = parse_routing_constraint_zones(
                [routing_constraint_zone].iter(),
                &map_schedule_stop_point_quay,
                &collections,
            );
            assert_eq!(0, routing_constraint_zones.len());
        }

        #[test]
        fn invalid_stop_point_id() {
            let routing_constraint_zone_xml = r#"<RoutingConstraintZone>
                    <members>
                        <ScheduledStopPointRef ref="scheduled_stop_point_ref" />
                        <ScheduledStopPointRef ref="scheduled_stop_point_ref_with_incorrect_stop_point_id" />
                    </members>
                </RoutingConstraintZone>"#;
            let routing_constraint_zone: Element = routing_constraint_zone_xml.parse().unwrap();
            let map_schedule_stop_point_quay = map_schedule_stop_point_quay();
            let collections = collections();
            let routing_constraint_zones = parse_routing_constraint_zones(
                [routing_constraint_zone].iter(),
                &map_schedule_stop_point_quay,
                &collections,
            );
            assert_eq!(0, routing_constraint_zones.len());
        }
    }

    mod apply_routing_constraint_zones {
        use super::*;
        use pretty_assertions::assert_eq;

        fn stop_points() -> CollectionWithId<StopPoint> {
            let mut stop_points = CollectionWithId::default();
            stop_points
                .push(StopPoint {
                    id: String::from("sp1"),
                    ..Default::default()
                })
                .unwrap();
            stop_points
                .push(StopPoint {
                    id: String::from("sp2"),
                    ..Default::default()
                })
                .unwrap();
            stop_points
                .push(StopPoint {
                    id: String::from("sp3"),
                    ..Default::default()
                })
                .unwrap();
            stop_points
                .push(StopPoint {
                    id: String::from("sp4"),
                    ..Default::default()
                })
                .unwrap();
            stop_points
                .push(StopPoint {
                    id: String::from("sp5"),
                    ..Default::default()
                })
                .unwrap();
            stop_points
        }

        #[test]
        fn apply_routing_constraint_zones_at_start_of_journey_pattern() {
            let stop_points = stop_points();
            let sp1_idx = stop_points.get_idx("sp1").unwrap();
            let sp2_idx = stop_points.get_idx("sp2").unwrap();
            let sp3_idx = stop_points.get_idx("sp3").unwrap();
            let mut stop_points_in_journey_pattern = vec![sp1_idx, sp2_idx, sp3_idx]
                .into_iter()
                .map(|stop_point_idx| StopPointInJourneyPattern {
                    stop_point_idx,
                    scheduled_stop_point_ref: String::new(),
                    pickup_type: 0,
                    drop_off_type: 0,
                    local_zone_id: None,
                })
                .collect();
            let routing_constraint_zones = vec![vec![sp1_idx, sp2_idx]];
            apply_routing_constraint_zones(
                &stop_points,
                &mut stop_points_in_journey_pattern,
                &routing_constraint_zones,
            );

            assert_eq!(1, stop_points_in_journey_pattern[0].local_zone_id.unwrap());
            assert_eq!(1, stop_points_in_journey_pattern[1].local_zone_id.unwrap());
            assert_eq!(None, stop_points_in_journey_pattern[2].local_zone_id);
        }

        #[test]
        fn apply_routing_constraint_zones_at_end_of_journey_pattern() {
            let stop_points = stop_points();
            let sp1_idx = stop_points.get_idx("sp1").unwrap();
            let sp2_idx = stop_points.get_idx("sp2").unwrap();
            let sp3_idx = stop_points.get_idx("sp3").unwrap();
            let mut stop_points_in_journey_pattern = vec![sp1_idx, sp2_idx, sp3_idx]
                .into_iter()
                .map(|stop_point_idx| StopPointInJourneyPattern {
                    stop_point_idx,
                    scheduled_stop_point_ref: String::new(),
                    pickup_type: 0,
                    drop_off_type: 0,
                    local_zone_id: None,
                })
                .collect();
            let routing_constraint_zones = vec![vec![sp3_idx]];
            apply_routing_constraint_zones(
                &stop_points,
                &mut stop_points_in_journey_pattern,
                &routing_constraint_zones,
            );
            assert_eq!(None, stop_points_in_journey_pattern[0].local_zone_id);
            assert_eq!(None, stop_points_in_journey_pattern[1].local_zone_id);
            assert_eq!(1, stop_points_in_journey_pattern[2].local_zone_id.unwrap());
        }

        #[test]
        fn apply_two_routing_constraint_zones() {
            let stop_points = stop_points();
            let sp1_idx = stop_points.get_idx("sp1").unwrap();
            let sp2_idx = stop_points.get_idx("sp2").unwrap();
            let sp3_idx = stop_points.get_idx("sp3").unwrap();
            let sp4_idx = stop_points.get_idx("sp4").unwrap();
            let sp5_idx = stop_points.get_idx("sp5").unwrap();
            let mut stop_points_in_journey_pattern =
                vec![sp1_idx, sp2_idx, sp3_idx, sp4_idx, sp5_idx]
                    .into_iter()
                    .map(|stop_point_idx| StopPointInJourneyPattern {
                        stop_point_idx,
                        scheduled_stop_point_ref: String::new(),
                        pickup_type: 0,
                        drop_off_type: 0,
                        local_zone_id: None,
                    })
                    .collect();
            let routing_constraint_zones = vec![vec![sp1_idx, sp2_idx], vec![sp4_idx]];
            apply_routing_constraint_zones(
                &stop_points,
                &mut stop_points_in_journey_pattern,
                &routing_constraint_zones,
            );
            assert_eq!(1, stop_points_in_journey_pattern[0].local_zone_id.unwrap());
            assert_eq!(1, stop_points_in_journey_pattern[1].local_zone_id.unwrap());
            assert_eq!(None, stop_points_in_journey_pattern[2].local_zone_id);
            assert_eq!(2, stop_points_in_journey_pattern[3].local_zone_id.unwrap());
            assert_eq!(None, stop_points_in_journey_pattern[4].local_zone_id);
        }

        #[test]
        fn apply_two_routing_constraint_zones_with_override() {
            let stop_points = stop_points();
            let sp1_idx = stop_points.get_idx("sp1").unwrap();
            let sp2_idx = stop_points.get_idx("sp2").unwrap();
            let sp3_idx = stop_points.get_idx("sp3").unwrap();
            let sp4_idx = stop_points.get_idx("sp4").unwrap();
            let mut stop_points_in_journey_pattern = vec![sp1_idx, sp2_idx, sp3_idx, sp4_idx]
                .into_iter()
                .map(|stop_point_idx| StopPointInJourneyPattern {
                    stop_point_idx,
                    scheduled_stop_point_ref: String::new(),
                    pickup_type: 0,
                    drop_off_type: 0,
                    local_zone_id: None,
                })
                .collect();
            let routing_constraint_zones = vec![vec![sp2_idx, sp3_idx], vec![sp3_idx, sp4_idx]];
            apply_routing_constraint_zones(
                &stop_points,
                &mut stop_points_in_journey_pattern,
                &routing_constraint_zones,
            );
            assert_eq!(None, stop_points_in_journey_pattern[0].local_zone_id);
            assert_eq!(1, stop_points_in_journey_pattern[1].local_zone_id.unwrap());
            assert_eq!(2, stop_points_in_journey_pattern[2].local_zone_id.unwrap());
            assert_eq!(2, stop_points_in_journey_pattern[3].local_zone_id.unwrap());
        }
    }

    mod get_stop_point {
        use super::*;
        use pretty_assertions::assert_eq;

        fn stop_points() -> (
            CollectionWithId<StopPoint>,
            CollectionWithId<VirtualStopPoint>,
        ) {
            let stop_points = CollectionWithId::new(vec![
                StopPoint {
                    id: String::from("sp1"),
                    ..Default::default()
                },
                StopPoint {
                    id: String::from("sp2"),
                    ..Default::default()
                },
            ])
            .unwrap();
            let virtual_stop_points = CollectionWithId::new(vec![StopPoint {
                id: String::from("vsp0"),
                ..Default::default()
            }])
            .unwrap();
            (stop_points, virtual_stop_points)
        }

        #[test]
        fn existing_stoppoint() {
            let (mut stop_points, virtual_stop_points) = stop_points();
            let idx = get_stop_point("sp2", &mut stop_points, &virtual_stop_points).unwrap();
            assert_eq!(2, stop_points.len());
            assert_eq!(String::from("sp2"), stop_points[idx].id);
        }

        #[test]
        fn insert_virtual_stoppoint_() {
            let (mut stop_points, virtual_stop_points) = stop_points();
            let idx = get_stop_point("vsp0", &mut stop_points, &virtual_stop_points).unwrap();
            assert_eq!(3, stop_points.len());
            assert_eq!(String::from("vsp0"), stop_points[idx].id);
        }

        #[test]
        #[should_panic(expected = "Failed to find StopPoint unknown")]
        fn unknown_stoppoint() {
            let (mut stop_points, virtual_stop_points) = stop_points();
            get_stop_point("unknown", &mut stop_points, &virtual_stop_points).unwrap();
        }
    }

    mod parse_passenger_stop_assignment {
        use super::*;
        use pretty_assertions::assert_eq;

        fn stop_points() -> (
            CollectionWithId<StopPoint>,
            CollectionWithId<VirtualStopPoint>,
        ) {
            let stop_points = CollectionWithId::new(vec![StopPoint {
                id: String::from("sp1"),
                ..Default::default()
            }])
            .unwrap();
            let virtual_stop_points = CollectionWithId::new(vec![StopPoint {
                id: String::from("monomodalStopPlace:vsp0"),
                ..Default::default()
            }])
            .unwrap();
            (stop_points, virtual_stop_points)
        }

        #[test]
        fn valid_passenger_stop_assignment_with_quayref() {
            let passenger_stop_assignment_xml = r#"<PassengerStopAssignment id="psa">
                    <ScheduledStopPointRef ref="sspr" />
                    <QuayRef ref=":::sp1:" />
                </PassengerStopAssignment>"#;
            let passenger_stop_assignment_el: Element =
                passenger_stop_assignment_xml.parse().unwrap();
            let (mut stop_points, virtual_stop_points) = stop_points();
            let passenger_stop_assignment = parse_passenger_stop_assignment(
                [passenger_stop_assignment_el].iter(),
                &mut stop_points,
                &virtual_stop_points,
            );
            assert_eq!(1, passenger_stop_assignment.len());
            assert_eq!("sp1", passenger_stop_assignment.get("sspr").unwrap());
        }

        #[test]
        fn valid_passenger_stop_assignment_with_stopplaceref() {
            let passenger_stop_assignment_xml = r#"<PassengerStopAssignment id="psa">
                    <ScheduledStopPointRef ref="sspr" />
                    <StopPlaceRef ref="::monomodalStopPlace:vsp0:" />
                </PassengerStopAssignment>"#;
            let passenger_stop_assignment_el: Element =
                passenger_stop_assignment_xml.parse().unwrap();
            let (mut stop_points, virtual_stop_points) = stop_points();
            let passenger_stop_assignment = parse_passenger_stop_assignment(
                [passenger_stop_assignment_el].iter(),
                &mut stop_points,
                &virtual_stop_points,
            );
            assert_eq!(1, passenger_stop_assignment.len());
            assert_eq!(
                "monomodalStopPlace:vsp0",
                passenger_stop_assignment.get("sspr").unwrap()
            );
        }

        #[test]
        fn valid_passenger_stop_assignment_without_quayref_or_stopplaceref() {
            let passenger_stop_assignment_xml = r#"<PassengerStopAssignment id="psa">
                    <ScheduledStopPointRef ref="sspr" />
                </PassengerStopAssignment>"#;
            let passenger_stop_assignment_el: Element =
                passenger_stop_assignment_xml.parse().unwrap();
            let (mut stop_points, virtual_stop_points) = stop_points();
            let passenger_stop_assignment = parse_passenger_stop_assignment(
                [passenger_stop_assignment_el].iter(),
                &mut stop_points,
                &virtual_stop_points,
            );
            assert_eq!(0, passenger_stop_assignment.len());
        }
    }

    mod parse_vehicle_journey_stop_assignment {
        use super::*;
        use pretty_assertions::assert_eq;

        fn stop_points() -> CollectionWithId<StopPoint> {
            CollectionWithId::new(vec![StopPoint {
                id: String::from("sp1"),
                ..Default::default()
            }])
            .unwrap()
        }

        #[test]
        fn valid_vehicle_journey_stop_assignment() {
            let vehicle_journey_stop_assignment_xml = r#"<VehicleJourneyStopAssignment id="vjsa">
                    <QuayRef ref=":::sp1:" />
                    <ScheduledStopPointRef ref="sspr" />
                    <VehicleJourneyRef ref="vjr" />
                </VehicleJourneyStopAssignment>"#;
            let vehicle_journey_stop_assignment_el: Element =
                vehicle_journey_stop_assignment_xml.parse().unwrap();
            let stop_points = stop_points();
            let sp_idx = stop_points.get_idx("sp1").unwrap();
            let vehicle_journey_stop_assignment = parse_vehicle_journey_stop_assignment(
                [vehicle_journey_stop_assignment_el].iter(),
                &stop_points,
            );
            assert_eq!(1, vehicle_journey_stop_assignment.len());
            assert_eq!(
                &sp_idx,
                vehicle_journey_stop_assignment
                    .get(&("vjr".to_string(), "sspr".to_string()))
                    .unwrap()
            );
        }

        #[test]
        fn unknown_stoppoint_in_vehicle_journey_stop_assignment() {
            let vehicle_journey_stop_assignment_xml = r#"<VehicleJourneyStopAssignment id="vjsa">
                    <QuayRef ref=":::sp2:" />
                    <ScheduledStopPointRef ref="sspr" />
                    <VehicleJourneyRef ref="vjr" />
                </VehicleJourneyStopAssignment>"#;
            let vehicle_journey_stop_assignment_el: Element =
                vehicle_journey_stop_assignment_xml.parse().unwrap();
            let stop_points = stop_points();
            let vehicle_journey_stop_assignment = parse_vehicle_journey_stop_assignment(
                [vehicle_journey_stop_assignment_el].iter(),
                &stop_points,
            );
            assert_eq!(0, vehicle_journey_stop_assignment.len());
        }
    }
}
