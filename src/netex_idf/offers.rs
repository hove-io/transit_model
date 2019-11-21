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
    calendars::{self, DayTypes},
    common,
    lines::LineNetexIDF,
    modes::MODES,
};
use crate::{
    minidom_utils::{TryAttribute, TryOnlyChild},
    model::Collections,
    netex_utils::{self, FrameType},
    objects::{
        Calendar, Dataset, Date, Route, StopPoint, StopTime, Time, ValidityPeriod, VehicleJourney,
    },
    validity_period, Result,
};
use failure::{bail, format_err, ResultExt};
use log::{info, warn};
use minidom::Element;
use std::{
    collections::{BTreeSet, HashMap},
    convert::TryFrom,
    fs::File,
    io::Read,
    path::Path,
};
use transit_model_collection::{CollectionWithId, Idx};
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
struct StopPointInJourneyPattern {
    stop_point_idx: Idx<StopPoint>,
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
        let id = route_element.try_attribute("id")?;
        let line_id = route_element
            .try_only_child("LineRef")?
            .try_attribute("ref")?;
        let name = route_element
            .try_only_child("Name")?
            .text()
            .trim()
            .to_string();
        let direction_type = route_element
            .only_child("DirectionType")
            .map(|direction_type| direction_type.text().trim().to_string());
        let route = Route {
            id,
            line_id,
            name,
            direction_type,
            ..Default::default()
        };
        Ok(route)
    }
}

pub fn read_offer_folder(
    offer_folder: &Path,
    collections: &mut Collections,
    lines_netex_idf: &CollectionWithId<LineNetexIDF>,
) -> Result<()> {
    let calendars_path = offer_folder.join(CALENDARS_FILENAME);
    let (map_daytypes, validity_period) = if calendars_path.exists() {
        let mut calendars_file =
            File::open(&calendars_path).with_context(ctx_from_path!(calendars_path))?;
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
        let mut common_file = File::open(&common_path).with_context(ctx_from_path!(common_path))?;
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
        let mut offer_file = File::open(offer_path).with_context(ctx_from_path!(offer_path))?;
        let mut offer_file_content = String::new();
        offer_file.read_to_string(&mut offer_file_content)?;
        let offer: Element = offer_file_content
            .parse()
            .map_err(|_| format_err!("Failed to open {}", offer_path.display()))?;
        info!("Reading {}", offer_path.display());
        let (routes, vehicle_journeys, calendars) =
            skip_fail!(
                parse_offer(&offer, collections, lines_netex_idf, &map_daytypes)
                    .map_err(|e| format_err!("Skip file {}: {}", offer_path.display(), e))
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
        let pickup_type = boarding_type(stop_point_in_journey_pattern_element, "ForBoarding");
        let drop_off_type = boarding_type(stop_point_in_journey_pattern_element, "ForAlighting");
        Ok(StopPointInJourneyPattern {
            stop_point_idx,
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
                .attribute::<String>("ref")
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
            Some(stop_point_idxs)
        })
        .collect()
}

fn parse_passenger_stop_assignment<'a, I>(psa_elements: I) -> HashMap<String, String>
where
    I: Iterator<Item = &'a Element>,
{
    psa_elements
        .filter_map(|psa_element| {
            let scheduled_stop_point_ref: String = psa_element
                .only_child("ScheduledStopPointRef")?
                .attribute("ref")?;
            let quay_ref: String = psa_element.only_child("QuayRef")?.attribute("ref")?;
            Some((scheduled_stop_point_ref, quay_ref))
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
        let route = skip_fail!(Route::try_from(route_element));
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
        validity_period::update_validity_period(dataset, &validity_period);
    }
    CollectionWithId::new(datasets)
}

fn enhance_with_object_code(
    routes: &CollectionWithId<Route>,
    journey_patterns: &JourneyPatterns,
) -> CollectionWithId<Route> {
    let mut enhanced_routes = CollectionWithId::default();
    let map_routes_journeypatterns: HashMap<&String, String> = journey_patterns
        .iter()
        .map(|(jp_id, journey_pattern)| (&journey_pattern.route.id, jp_id.clone()))
        .collect();
    for route in routes.values() {
        let journey_pattern_ref =
            skip_fail!(map_routes_journeypatterns.get(&route.id).ok_or_else(|| {
                format_err!(
                    "Route {} doesn't have any ServiceJourneyPattern associated",
                    route.id
                )
            }));
        let codes = vec![(
            String::from("Netex_ServiceJourneyPattern"),
            journey_pattern_ref.clone(),
        )]
        .into_iter()
        .collect();
        let mut route = route.clone();
        route.codes = codes;
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
) -> Result<Vec<StopTime>> {
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
                pickup_type,
                drop_off_type,
                local_zone_id,
            } = *stop_point_in_journey_pattern;
            let times = arrival_departure_times(tpt)?;
            let stop_time = StopTime {
                stop_point_idx,
                sequence: sequence as u32,
                arrival_time: times.0,
                departure_time: times.1,
                boarding_duration: 0,
                alighting_duration: 0,
                pickup_type,
                drop_off_type,
                datetime_estimated: false,
                local_zone_id,
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
        let id = service_journey_element.try_attribute("id")?;
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
            .get(line_netex_idf.mode.as_str())
            .ok_or_else(|| format_err!("Mode {} doesn't exist", line_netex_idf.mode))?
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
        let vehicle_journey = skip_fail!(parse_service_journey(
            service_journey_element,
            collections,
            lines_netex_idf,
            routes,
            journey_patterns,
        ));
        if !collections.routes.contains_id(&vehicle_journey.route_id)
            && !routes.contains_id(&vehicle_journey.route_id)
        {
            warn!(
                "Failed to create vehicle journey {} because route {} doesn't exist.",
                vehicle_journey.id, vehicle_journey.route_id
            );
            continue;
        }

        let stop_times = skip_fail!(stop_times(service_journey_element, &journey_patterns));
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
    collections: &Collections,
    lines_netex_idf: &CollectionWithId<LineNetexIDF>,
    map_daytypes: &DayTypes,
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
        .map(parse_passenger_stop_assignment)
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
                parse_general_frame_by_type(&vec![&general_frame_1, &general_frame_2]).unwrap();
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
            parse_general_frame_by_type(&vec![&general_frame_1, &general_frame_2]).unwrap();
        }

        #[test]
        #[should_panic(expected = "Failed to identify the type of this GeneralFrame")]
        fn incorrect_general_frame_type() {
            let xml_general_frame = r#"<GeneralFrame>
                    <TypeOfFrameRef ref="FR100:TypeOfFrame:NETEX_UNKNOWN_TYPE:"/>
               </GeneralFrame>"#;
            let general_frame: Element = xml_general_frame.parse().unwrap();
            parse_general_frame_by_type(&vec![&general_frame]).unwrap();
        }
    }

    mod parse_routes {
        use super::*;
        use crate::objects::Line;
        use pretty_assertions::assert_eq;

        #[test]
        fn routes() {
            let xml = r#"<Route id="route_id">
                    <Name>Route name</Name>
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
            let route = routes.get("route_id").unwrap();
            assert_eq!("route_id", route.id.as_str());
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
        fn add_object_code() {
            let route = Route {
                id: String::from("route_id"),
                name: String::from("Route Name"),
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
            assert_eq!("Route Name", route.name.as_str());
            assert_eq!(1, route.codes.len());
            let code = route.codes.iter().next().unwrap();
            assert_eq!("Netex_ServiceJourneyPattern", code.0.as_str());
            assert_eq!("service_journey_pattern_id", code.1.as_str());
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
        use crate::objects::{Comment, CommentType, Dataset};
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
                    id: String::from("route_id"),
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
            let service_journey_xml = r#"<ServiceJourney id="service_journey_id">
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
                pickup_type: 0,
                drop_off_type: 1,
                local_zone_id: None,
            });
            if let Some(route) = routes.get("route_id") {
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
                private_code: None,
                network_id: String::from("network_id"),
                company_id: String::from("company_id"),
                mode: String::from("bus"),
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
        fn parse_vehicle_journey() {
            let service_journey_element = service_journey();
            let service_journey_xml = r#"<ServiceJourney id="service_journey_id_1">
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
            )
            .unwrap();

            assert_eq!(2, vehicle_journeys.len());
            let vehicle_journey = vehicle_journeys.get("service_journey_id").unwrap();
            assert_eq!("route_id", vehicle_journey.route_id.as_str());
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
            let vehicle_journey = vehicle_journeys.get("service_journey_id_1").unwrap();
            assert_eq!("route_id", vehicle_journey.route_id.as_str());
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
                    id: String::from("route_id"),
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
            )
            .unwrap();
            assert_eq!(0, vehicle_journeys.len());
            assert_eq!(0, calendars.len());
        }

        #[test]
        fn ignore_vehicle_journey_without_physical_mode() {
            let service_journey_element = service_journey();
            let mut lines_netex_idf = lines_netex_idf();
            use std::ops::DerefMut;
            lines_netex_idf.get_mut("line_id").unwrap().deref_mut().mode =
                String::from("unknown_mode_id");
            let day_types = day_types();
            let collections = collections();
            let destination_displays = destination_displays();
            let journey_patterns = journey_patterns(&collections.routes, &destination_displays);
            let (vehicle_journeys, calendars) = parse_vehicle_journeys(
                vec![service_journey_element].iter(),
                &collections,
                &lines_netex_idf,
                &CollectionWithId::default(),
                &journey_patterns,
                &day_types,
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
}
