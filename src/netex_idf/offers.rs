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

use super::{
    calendars::{self, DayTypes},
    lines::LineNetexIDF,
    modes::MODES,
};
use crate::{
    minidom_utils::{TryAttribute, TryOnlyChild},
    model::Collections,
    netex_utils::{self, FrameType},
    objects::{Calendar, Date, Route, VehicleJourney},
    Result,
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
use transit_model_collection::CollectionWithId;
use walkdir::WalkDir;

pub const CALENDARS_FILENAME: &str = "calendriers.xml";
pub const COMMON_FILENAME: &str = "commun.xml";
pub const NETEX_STRUCTURE: &str = "NETEX_STRUCTURE";
pub const NETEX_SCHEDULE: &str = "NETEX_HORAIRE";
pub const NETEX_CALENDAR: &str = "NETEX_CALENDRIER";

#[derive(Debug, Eq, Hash, PartialEq)]
pub enum GeneralFrameType {
    Structure,
    Schedule,
    Calendar,
}
type GeneralFrames<'a> = HashMap<GeneralFrameType, &'a Element>;

impl std::fmt::Display for GeneralFrameType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Structure => write!(f, "{}", NETEX_STRUCTURE),
            Self::Schedule => write!(f, "{}", NETEX_SCHEDULE),
            Self::Calendar => write!(f, "{}", NETEX_CALENDAR),
        }
    }
}

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
    let map_daytypes = if calendars_path.exists() {
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
    if common_path.exists() {
        let mut common_file = File::open(&common_path).with_context(ctx_from_path!(common_path))?;
        let mut common_file_content = String::new();
        common_file.read_to_string(&mut common_file_content)?;
        let common: Element = common_file_content
            .parse()
            .map_err(|_| format_err!("Failed to open {}", common_path.display()))?;
        info!("Reading {}", common_path.display());
        parse_common(&common)?;
    }

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
    }
    Ok(())
}

fn parse_common(_common: &Element) -> Result<()> {
    // TODO: To implement
    Ok(())
}

fn parse_service_journey_patterns<'a, I>(sjp_elements: I) -> HashMap<String, &'a Element>
where
    I: Iterator<Item = &'a Element>,
{
    sjp_elements
        .filter_map(|sjp_element| {
            let id: String = sjp_element.attribute("id")?;
            Some((id, sjp_element))
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

fn enhance_with_object_code(
    routes: CollectionWithId<Route>,
    map_journeypatterns: &HashMap<String, &Element>,
) -> CollectionWithId<Route> {
    let mut enhanced_routes = CollectionWithId::default();
    let map_routes_journeypatterns: HashMap<String, String> = map_journeypatterns
        .iter()
        .filter_map(|(jp_id, jp_element)| {
            let route_ref: String = jp_element.only_child("RouteRef")?.attribute("ref")?;
            Some((route_ref, jp_id.clone()))
        })
        .collect();
    for route in routes {
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
        let route = Route { codes, ..route };
        // We are inserting only routes that were already in a 'CollectionWithId'
        enhanced_routes.push(route).unwrap();
    }
    enhanced_routes
}

fn parse_vehicle_journeys<'a, I>(
    service_journey_elements: I,
    collections: &Collections,
    lines_netex_idf: &CollectionWithId<LineNetexIDF>,
    routes: &CollectionWithId<Route>,
    map_journeypatterns: &HashMap<String, &Element>,
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
        map_journeypatterns: &HashMap<String, &Element>,
    ) -> Result<VehicleJourney> {
        let id = service_journey_element.try_attribute("id")?;
        let journey_pattern_ref: String = service_journey_element
            .try_only_child("JourneyPatternRef")?
            .try_attribute("ref")?;
        let route_id: String = map_journeypatterns
            .get(&journey_pattern_ref)
            .and_then(|sjp_element| sjp_element.only_child("RouteRef"))
            .and_then(|route_ref_element| route_ref_element.attribute("ref"))
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
        let vehicle_journey = VehicleJourney {
            id,
            route_id,
            dataset_id,
            company_id,
            physical_mode_id,
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
            map_journeypatterns
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
    let map_journeypatterns = structure_frame
        .only_child("members")
        .map(Element::children)
        .map(|childrens| childrens.filter(|e| e.name() == "ServiceJourneyPattern"))
        .map(parse_service_journey_patterns)
        .unwrap_or_else(HashMap::new);
    let routes = structure_frame
        .only_child("members")
        .map(Element::children)
        .map(|childrens| childrens.filter(|e| e.name() == "Route"))
        .map(|route_elements| parse_routes(route_elements, collections))
        .transpose()?
        .unwrap_or_else(CollectionWithId::default);
    let routes = enhance_with_object_code(routes, &map_journeypatterns);
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
                &map_journeypatterns,
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
            let mut map = HashMap::new();
            let xml = r#"<ServiceJourneyPattern id="service_journey_pattern_id">
                    <RouteRef ref="route_id" />
                </ServiceJourneyPattern>"#;
            let element: Element = xml.parse().unwrap();
            map.insert(String::from("service_journey_pattern_id"), &element);
            let routes = enhance_with_object_code(routes, &map);
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
            let routes = enhance_with_object_code(routes, &HashMap::new());
            assert_eq!(0, routes.len());
        }
    }

    mod parse_vehicle_journeys {
        use super::*;
        use crate::objects::Dataset;
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
        }

        fn service_journey() -> Element {
            let service_journey_xml = r#"<ServiceJourney id="service_journey_id">
                    <dayTypes>
                        <DayTypeRef ref="day_type_id_1" />
                        <DayTypeRef ref="day_type_id_2" />
                    </dayTypes>
                    <JourneyPatternRef ref="journey_pattern_id" />
                </ServiceJourney>"#;
            service_journey_xml.parse().unwrap()
        }

        fn journey_pattern() -> Element {
            let journey_pattern_xml = r#"<ServiceJourneyPattern id="journey_pattern_id">
                    <RouteRef ref="route_id" />
                </ServiceJourneyPattern>"#;
            journey_pattern_xml.parse().unwrap()
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
                wheelchair_accessible: false,
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
                    <dayTypes>
                        <DayTypeRef ref="day_type_id_1" />
                        <DayTypeRef ref="day_type_id_2" />
                    </dayTypes>
                    <JourneyPatternRef ref="journey_pattern_id" />
                </ServiceJourney>"#;
            let service_journey_element_1 = service_journey_xml.parse().unwrap();
            let journey_pattern_element = journey_pattern();
            let lines_netex_idf = lines_netex_idf();
            let day_types = day_types();
            let collections = collections();
            let mut map_journeypatterns = HashMap::new();
            map_journeypatterns
                .insert(String::from("journey_pattern_id"), &journey_pattern_element);
            let (vehicle_journeys, calendars) = parse_vehicle_journeys(
                vec![&service_journey_element, &service_journey_element_1].into_iter(),
                &collections,
                &lines_netex_idf,
                &CollectionWithId::default(),
                &map_journeypatterns,
                &day_types,
            )
            .unwrap();

            assert_eq!(2, vehicle_journeys.len());
            let vehicle_journey = vehicle_journeys.get("service_journey_id").unwrap();
            assert_eq!("route_id", vehicle_journey.route_id.as_str());
            assert_eq!("dataset_id", vehicle_journey.dataset_id.as_str());
            let vehicle_journey = vehicle_journeys.get("service_journey_id_1").unwrap();
            assert_eq!("route_id", vehicle_journey.route_id.as_str());
            assert_eq!("dataset_id", vehicle_journey.dataset_id.as_str());
            assert_eq!("company_id", vehicle_journey.company_id.as_str());
            assert_eq!("Bus", vehicle_journey.physical_mode_id.as_str());

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
            let map_journeypatterns = HashMap::new();
            let (vehicle_journeys, calendars) = parse_vehicle_journeys(
                vec![service_journey_element].iter(),
                &collections,
                &lines_netex_idf,
                &CollectionWithId::default(),
                &map_journeypatterns,
                &day_types,
            )
            .unwrap();
            assert_eq!(0, vehicle_journeys.len());
            assert_eq!(0, calendars.len());
        }

        #[test]
        fn ignore_vehicle_journey_without_route() {
            let service_journey_element = service_journey();
            let journey_pattern_element = journey_pattern();
            let lines_netex_idf = lines_netex_idf();
            let day_types = day_types();
            let collections = Collections::default();
            let mut map_journeypatterns = HashMap::new();
            map_journeypatterns
                .insert(String::from("journey_pattern_id"), &journey_pattern_element);
            let (vehicle_journeys, calendars) = parse_vehicle_journeys(
                vec![service_journey_element].iter(),
                &collections,
                &lines_netex_idf,
                &CollectionWithId::default(),
                &map_journeypatterns,
                &day_types,
            )
            .unwrap();
            assert_eq!(0, vehicle_journeys.len());
            assert_eq!(0, calendars.len());
        }

        #[test]
        fn ignore_vehicle_journey_without_line() {
            let service_journey_element = service_journey();
            let journey_pattern_element = journey_pattern();
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
            let mut map_journeypatterns = HashMap::new();
            map_journeypatterns
                .insert(String::from("journey_pattern_id"), &journey_pattern_element);
            let (vehicle_journeys, calendars) = parse_vehicle_journeys(
                vec![service_journey_element].iter(),
                &collections,
                &lines_netex_idf,
                &CollectionWithId::default(),
                &map_journeypatterns,
                &day_types,
            )
            .unwrap();
            assert_eq!(0, vehicle_journeys.len());
            assert_eq!(0, calendars.len());
        }

        #[test]
        fn ignore_vehicle_journey_without_physical_mode() {
            let service_journey_element = service_journey();
            let journey_pattern_element = journey_pattern();
            let mut lines_netex_idf = lines_netex_idf();
            use std::ops::DerefMut;
            lines_netex_idf.get_mut("line_id").unwrap().deref_mut().mode =
                String::from("unknown_mode_id");
            let day_types = day_types();
            let mut collections = Collections::default();
            collections
                .routes
                .push(Route {
                    id: String::from("route_id"),
                    line_id: String::from("line_id"),
                    ..Default::default()
                })
                .unwrap();
            let mut map_journeypatterns = HashMap::new();
            map_journeypatterns
                .insert(String::from("journey_pattern_id"), &journey_pattern_element);
            let (vehicle_journeys, calendars) = parse_vehicle_journeys(
                vec![service_journey_element].iter(),
                &collections,
                &lines_netex_idf,
                &CollectionWithId::default(),
                &map_journeypatterns,
                &day_types,
            )
            .unwrap();
            assert_eq!(0, vehicle_journeys.len());
            assert_eq!(0, calendars.len());
        }

        #[test]
        fn ignore_vehicle_journey_with_invalid_service_journey_no_id() {
            let service_journey_xml = r#"<ServiceJourney>
                    <dayTypes>
                        <DayTypeRef ref="day_type_id_1" />
                        <DayTypeRef ref="day_type_id_2" />
                    </dayTypes>
                    <JourneyPatternRef ref="journey_pattern_id" />
                </ServiceJourney>"#;
            let service_journey_element: Element = service_journey_xml.parse().unwrap();
            let journey_pattern_element = journey_pattern();
            let lines_netex_idf = lines_netex_idf();
            let day_types = day_types();
            let collections = Collections::default();
            let mut map_journeypatterns = HashMap::new();
            map_journeypatterns
                .insert(String::from("journey_pattern_id"), &journey_pattern_element);
            let (vehicle_journeys, calendars) = parse_vehicle_journeys(
                vec![service_journey_element].iter(),
                &collections,
                &lines_netex_idf,
                &CollectionWithId::default(),
                &map_journeypatterns,
                &day_types,
            )
            .unwrap();
            assert_eq!(0, vehicle_journeys.len());
            assert_eq!(0, calendars.len());
        }

        #[test]
        fn increment_service_id() {
            let service_journey_element = service_journey();
            let journey_pattern_element = journey_pattern();
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
            let mut map_journeypatterns = HashMap::new();
            map_journeypatterns
                .insert(String::from("journey_pattern_id"), &journey_pattern_element);
            let (_, calendars) = parse_vehicle_journeys(
                vec![&service_journey_element].into_iter(),
                &collections,
                &lines_netex_idf,
                &CollectionWithId::default(),
                &map_journeypatterns,
                &day_types,
            )
            .unwrap();

            assert_eq!(1, calendars.len());
            let calendar = calendars.get("2").unwrap();
            assert!(calendar.dates.contains(&Date::from_ymd(2019, 1, 1)));
            assert!(calendar.dates.contains(&Date::from_ymd(2019, 1, 2)));
        }
    }
}
