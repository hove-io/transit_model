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

use chrono;
use model::Collections;
use objects::{self, CommentLinksT, Coord, KeysValues};
use std::collections::HashMap;
use std::io::Read;
use Result;

extern crate quick_xml;
extern crate serde_json;
use self::quick_xml::Reader;
extern crate minidom;
use self::minidom::Element;
use std::str::FromStr;

pub type Date = chrono::NaiveDate;

type RoutePointId = String;
type StopPointId = String;
type RoutePointMapping = HashMap<RoutePointId, StopPointId>;
type RouteLineMap = HashMap<String, String>;

#[derive(Default)]
struct NetexContext {
    namespace: String,
    first_operator_id: String,
    routepoint_mapping: RoutePointMapping,
    route_line_map: RouteLineMap,
    route_mode_map: HashMap<String, String>,
    journeypattern_route_map: HashMap<String, String>,
}

pub fn read_netex_file<R: Read>(collections: &mut Collections, mut file: R) -> Result<()> {
    let mut file_content = "".to_string();
    file.read_to_string(&mut file_content)?;
    let mut reader = Reader::from_str(&file_content);
    let root = Element::from_reader(&mut reader)?;

    let mut context = NetexContext {
        namespace: root.ns().unwrap_or("".to_string()),
        ..Default::default()
    };

    root.get_child("dataObjects", context.namespace.as_str())
        .unwrap()
        .children()
        .filter(|frame| frame.name() == "CompositeFrame")
        .map(|frame| {
            read_composite_data_frame(collections, &mut context, frame).map_err(|_| {
                format_err!(
                    "Reading Frame id={:?}",
                    frame.attr("id").unwrap_or("undefined")
                )
            })
        })
        .collect()
}

fn read_composite_data_frame(
    collections: &mut Collections,
    context: &mut NetexContext,
    composite_frame: &Element,
) -> Result<()> {
    composite_frame
        .get_child("frames", &mut context.namespace)
        .unwrap()
        .children()
        .map(|frame| match frame.name() {
            "SiteFrame" => read_site_frame(collections, context, &frame),
            "ServiceFrame" => read_service_frame(collections, context, &frame),
            "ServiceCalendarFrame" => read_service_calendar_frame(collections, context, &frame),
            "TimetableFrame" => read_time_table_frame(collections, context, &frame),
            "ResourceFrame" => read_resource_frame(collections, context, &frame),
            _ => Ok(()),
        })
        .collect()
}

fn read_resource_frame(
    collections: &mut Collections,
    context: &mut NetexContext,
    resource_frame: &Element,
) -> Result<()> {
    // a ResourceFrame contains 0..1 organisations or 0..1 groupsOfOperators
    // (other objects don't seem to be relevant)
    // for the moment, only reading "organisations" until a groupsOfOperators use is encontered.

    let organisations = resource_frame.get_child("organisations", &context.namespace);
    match organisations {
        None => Ok(()),
        Some(orgs) => read_organisations(collections, context, &orgs),
    }
}

fn read_service_frame(
    collections: &mut Collections,
    context: &mut NetexContext,
    service_frame: &Element,
) -> Result<()> {
    let network_node = service_frame
        .get_child("Network", &context.namespace)
        .unwrap();
    let network = read_network(collections, context, &network_node);
    let lines_node = service_frame
        .get_child("lines", &context.namespace)
        .unwrap();
    read_lines_and_commercial_modes(collections, context, &lines_node, &network.id);
    let stop_assignments_node = service_frame
        .get_child("stopAssignments", &context.namespace)
        .unwrap();
    read_stop_assignements(collections, context, &stop_assignments_node);
    let routes_node = service_frame
        .get_child("routes", &context.namespace)
        .unwrap();
    read_routes(collections, context, &routes_node);
    let journey_patterns_node = service_frame.get_child("journeyPatterns", &context.namespace);
    if journey_patterns_node.is_some() {
        read_journey_patterns(context, &journey_patterns_node.unwrap());
    }

    let connections_node = service_frame.get_child("connections", &context.namespace);
    if connections_node.is_some() {
        read_connections(collections, context, &connections_node.unwrap());
    }
    collections.networks.push(network).unwrap();
    Ok(())
}

fn read_service_calendar_frame(
    collections: &mut Collections,
    context: &mut NetexContext,
    service_calendar_frame: &Element,
) -> Result<()> {
    // each ServiceCalendarFrame seems to represent one Calendar
    for node in service_calendar_frame.children() {
        //let's hope calendars (DayType) are defined before used by dayTypeAssignments
        match node.name() {
            "dayTypes" => {
                for day_type in node.children() {
                    assert!(
                        day_type.name() == "DayType",
                        "dayTypes child is expected to be DayType, found {:?}",
                        day_type.name()
                    );
                    let calendar_id = day_type.attr("id").unwrap().to_string();
                    let calendar = objects::Calendar::new(calendar_id);
                    collections.calendars.push(calendar).unwrap();
                }
            }
            "dayTypeAssignments" => {
                for assignment_node in node.children() {
                    assert!(
                        assignment_node.name() == "DayTypeAssignment",
                        "dayTypeAssignments child is expected to be DayTypeAssignment, found {:?}",
                        assignment_node.name()
                    );
                    read_day_type_assignments(collections, context, assignment_node);
                }
            }
            _ => (),
        };
    }
    Ok(())
}

fn read_day_type_assignments(
    collections: &mut Collections,
    context: &mut NetexContext,
    day_type_assignment: &Element,
) {
    let calendar_id = day_type_assignment
        .get_child("DayTypeRef", &context.namespace)
        .unwrap()
        .attr("ref")
        .unwrap();
    let day: Date = day_type_assignment
        .get_child("Date", &context.namespace)
        .unwrap()
        .text()
        .parse()
        .unwrap();
    let mut c = collections.calendars.get_mut(calendar_id).unwrap();
    c.dates.insert(day);
}

fn read_stop_assignements(
    _collections: &mut Collections,
    context: &mut NetexContext,
    stop_assignments: &Element,
) {
    for node in stop_assignments.children() {
        // assuming all children are PassengerStopAssignment
        context.routepoint_mapping.insert(
            node.get_child("ScheduledStopPointRef", &context.namespace)
                .unwrap()
                .attr("ref")
                .unwrap()
                .to_string(),
            node.get_child("QuayRef", &context.namespace)
                .unwrap()
                .attr("ref")
                .unwrap()
                .to_string(),
        );
    }
}

fn read_time_table_frame(
    collections: &mut Collections,
    context: &mut NetexContext,
    time_table_frame: &Element,
) -> Result<()> {
    let vj_node = time_table_frame
        .get_child("vehicleJourneys", &context.namespace)
        .unwrap();
    read_vehicle_journeys(collections, context, &vj_node);
    Ok(())
}

fn read_organisations(
    collections: &mut Collections,
    context: &mut NetexContext,
    organisations: &Element,
) -> Result<()> {
    let companies: Vec<_> = organisations
        .children()
        .map(|node| objects::Company {
            id: node.attr("id").unwrap().to_string(),
            name: node
                .get_child("Name", &context.namespace)
                .unwrap()
                .text()
                .to_string(),
            address: None,
            url: None,
            mail: None,
            phone: None,
        })
        .collect();
    if companies.len() > 0 {
        context.first_operator_id = companies[0].id.to_string();
        let mut companies: Vec<objects::Company> = companies
            .into_iter()
            .filter(|c| collections.companies.get_idx(&c.id).is_none())
            .collect();
        collections.companies.append(&mut companies)?;
    } else {
        context.first_operator_id = "default_company".to_string();
        if collections
            .companies
            .get_idx(&context.first_operator_id)
            .is_none()
        {
            collections
                .companies
                .push(objects::Company {
                    id: "default_company".to_string(),
                    name: "Default Company".to_string(),
                    address: None,
                    url: None,
                    mail: None,
                    phone: None,
                })
                .unwrap();
        };
    }
    Ok(())
}

fn read_vehicle_journeys(
    collections: &mut Collections,
    context: &mut NetexContext,
    vehicle_journeys: &Element,
) {
    for node in vehicle_journeys.children() {
        match node.name() {
            "ServiceJourney" => read_service_journey(collections, context, node),
            _ => println!("Reading {} not implemented", node.name()),
        }
    }
}

fn read_service_journey(
    collections: &mut Collections,
    context: &mut NetexContext,
    service_journey: &Element,
) {
    let vj_id = service_journey.attr("id").unwrap().to_string();
    let journey_pattern_ref = service_journey
        .get_child("JourneyPatternRef", &context.namespace)
        .map(|n| n.attr("ref").unwrap().to_string());
    let route_ref = service_journey
        .get_child("RouteRef", &context.namespace)
        .map(|n| n.attr("ref").unwrap().to_string());
    let route_id: Option<String> = route_ref.or(context
        .journeypattern_route_map
        .get(&journey_pattern_ref.unwrap_or("".to_string()))
        .map(|s| s.to_string()));
    if route_id.is_none() {
        panic!(
            "read_vehicle_journeys : impossible to find Route for id {}",
            vj_id
        );
    }
    let calendar_id = service_journey
        .get_child("dayTypes", &context.namespace)
        .unwrap()
        .get_child("DayTypeRef", &context.namespace)
        .map(|n| n.attr("ref").unwrap().to_string())
        .unwrap();
    let route_id = route_id.unwrap();
    let mode_name = context.route_mode_map.get(&route_id).unwrap().to_string();
    let mut vj = objects::VehicleJourney {
        id: vj_id,
        codes: KeysValues::default(),
        object_properties: KeysValues::default(),
        comment_links: CommentLinksT::default(),
        route_id: route_id,
        physical_mode_id: netex_mode_to_physical_mode_id(&mode_name).to_string(),
        dataset_id: "default_dataset".to_string(),
        service_id: calendar_id,
        headsign: None,
        block_id: None,
        company_id: service_journey
            .get_child("OperatorRef", &context.namespace)
            .map(|c| c.attr("ref").unwrap().to_string())
            .unwrap_or(context.first_operator_id.to_string()),
        trip_property_id: None,
        geometry_id: None,
        stop_times: vec![],
    };
    let calls_node = service_journey.get_child("calls", &context.namespace);
    if calls_node.is_some() {
        read_calls_stop_times(collections, context, &mut vj, &calls_node.unwrap());
    }
    collections.vehicle_journeys.push(vj).unwrap();
}

fn read_calls_stop_times(
    collections: &mut Collections,
    context: &mut NetexContext,
    vj: &mut objects::VehicleJourney,
    calls: &Element,
) {
    let mut stop_sequence = 0;
    for call in calls.children() {
        // assuming all children are Call
        stop_sequence = stop_sequence + 1;
        let routepoint_id = call
            .get_child("ScheduledStopPointRef", &context.namespace)
            .unwrap()
            .attr("ref")
            .unwrap();
        let stoppoint_id = context.routepoint_mapping.get(routepoint_id).unwrap();
        vj.stop_times.push(objects::StopTime {
            stop_point_idx: collections.stop_points.get_idx(&stoppoint_id).unwrap(),
            sequence: stop_sequence,
            arrival_time: objects::Time::from_str(
                call.get_child("Arrival", &context.namespace)
                    .unwrap()
                    .get_child("Time", &context.namespace)
                    .unwrap()
                    .text()
                    .as_ref(),
            ).unwrap(),
            departure_time: objects::Time::from_str(
                call.get_child("Departure", &context.namespace)
                    .unwrap()
                    .get_child("Time", &context.namespace)
                    .unwrap()
                    .text()
                    .as_ref(),
            ).unwrap(),
            boarding_duration: 0,
            alighting_duration: 0,
            pickup_type: 0,
            drop_off_type: 0,
            datetime_estimated: false,
            local_zone_id: None,
        });
    }
}

fn read_network(
    _collections: &mut Collections,
    context: &mut NetexContext,
    network: &Element,
) -> objects::Network {
    let network_id: String = match network.get_child("PrivateCode", &context.namespace) {
        None => network.attr("id").unwrap().to_string(),
        Some(n) => n.text().to_string(),
    };
    objects::Network {
        id: network_id,
        name: network
            .get_child("Name", &context.namespace)
            .unwrap()
            .text()
            .to_string(),
        url: None,
        codes: KeysValues::default(),
        timezone: None,
        lang: None,
        phone: None,
        address: None,
        sort_order: None,
    }
}

fn netex_mode_to_physical_mode_id(netex_mode: &str) -> String {
    match netex_mode {
        "air" => "Air".to_string(),
        "bus" => "Bus".to_string(),
        "coach" => "Coach".to_string(),
        "funicular" => "Fuicular".to_string(),
        "metro" => "Metro".to_string(),
        "rail" => "Train".to_string(),
        "trolleyBus" => "Tramway".to_string(),
        "tram" => "Tramway".to_string(),
        "water" => "Boat".to_string(),
        "cableWay" => "BusRapidTransit".to_string(),
        _ => "Bus".to_string(),
    }
}

fn read_lines_and_commercial_modes(
    collections: &mut Collections,
    context: &mut NetexContext,
    lines: &Element,
    network_id: &str,
) {
    for l in lines.children() {
        let mode_name = l
            .get_child("TransportMode", &context.namespace)
            .unwrap()
            .text();
        if collections.commercial_modes.get(&mode_name).is_none() {
            collections
                .commercial_modes
                .push(objects::CommercialMode {
                    id: mode_name.to_string(),
                    name: mode_name.to_string(),
                })
                .unwrap();
        };
        if collections
            .physical_modes
            .get(&netex_mode_to_physical_mode_id(&mode_name))
            .is_none()
        {
            collections
                .physical_modes
                .push(objects::PhysicalMode {
                    id: netex_mode_to_physical_mode_id(&mode_name).to_string(),
                    name: netex_mode_to_physical_mode_id(&mode_name).to_string(),
                    co2_emission: None,
                })
                .unwrap();
        };
        let private_code = l
            .get_child("PrivateCode", &context.namespace)
            .map(|s| s.text().to_string());
        let public_code = l
            .get_child("PublicCode", &context.namespace)
            .map(|s| s.text().to_string());
        let line_code = public_code.or(private_code);
        let line = objects::Line {
            id: l.attr("id").unwrap().to_string(),
            code: line_code,
            name: l
                .get_child("Name", &context.namespace)
                .unwrap()
                .text()
                .to_string(),
            codes: KeysValues::default(),
            object_properties: KeysValues::default(),
            comment_links: CommentLinksT::default(),
            forward_name: None,
            forward_direction: None,
            backward_name: None,
            backward_direction: None,
            color: None,
            text_color: None,
            sort_order: None,
            network_id: network_id.to_string(),
            commercial_mode_id: mode_name.to_string(),
            geometry_id: None,
            opening_time: None,
            closing_time: None,
        };
        for r in l
            .get_child("routes", &context.namespace)
            .unwrap()
            .children()
        {
            context
                .route_line_map
                .insert(r.attr("ref").unwrap().to_string(), line.id.to_string());
            context
                .route_mode_map
                .insert(r.attr("ref").unwrap().to_string(), mode_name.to_string());
        }
        collections.lines.push(line).unwrap();
    }
}

fn read_routes(collections: &mut Collections, context: &mut NetexContext, routes: &Element) {
    for r in routes.children() {
        let route = objects::Route {
            id: r.attr("id").unwrap().to_string(),
            name: r
                .get_child("Name", &context.namespace)
                .unwrap()
                .text()
                .to_string(),
            codes: KeysValues::default(),
            object_properties: KeysValues::default(),
            direction_type: None,
            comment_links: CommentLinksT::default(),
            line_id: context
                .route_line_map
                .get(r.attr("id").unwrap())
                .unwrap()
                .to_string(),
            geometry_id: None,
            destination_id: None,
        };
        collections.routes.push(route).unwrap();
    }
}

fn read_journey_patterns(context: &mut NetexContext, journey_patterns: &Element) {
    for jp in journey_patterns.children() {
        let jp_id = jp.attr("id").unwrap().to_string();
        let r = jp.get_child("RouteRef", &context.namespace);
        if let Some(ref_node) = r {
            let route_id = ref_node.attr("ref").unwrap().to_string();
            context.journeypattern_route_map.insert(jp_id, route_id);
        }
    }
}

fn read_connections(
    collections: &mut Collections,
    context: &mut NetexContext,
    connections: &Element,
) {
    for cnx in connections.children() {
        let transfer = objects::Transfer {
            from_stop_id: cnx
                .get_child("From", &context.namespace)
                .unwrap()
                .get_child("StopPlaceRef", &context.namespace)
                .unwrap()
                .attr("ref")
                .unwrap()
                .to_string(),
            to_stop_id: cnx
                .get_child("To", &context.namespace)
                .unwrap()
                .get_child("StopPlaceRef", &context.namespace)
                .unwrap()
                .attr("ref")
                .unwrap()
                .to_string(),
            min_transfer_time: None,
            real_min_transfer_time: None,
            equipment_id: None,
        };
        collections.transfers.push(transfer);
    }
}

fn read_site_frame(
    collections: &mut Collections,
    context: &mut NetexContext,
    site_frame: &Element,
) -> Result<()> {
    site_frame
        .get_child("stopPlaces", &context.namespace)
        .unwrap()
        .children()
        .map(|node| read_stop_place(collections, context, &node))
        .collect()
}

fn read_stop_place_name(context: &mut NetexContext, stop_place: &Element) -> Option<String> {
    // This function aims to be usable from a StopPlace node or a Quay node.
    let direct_name = stop_place
        .get_child("Name", &context.namespace)
        .map(|s| s.text().to_string());
    let label = stop_place
        .get_child("Label", &context.namespace)
        .map(|s| s.text().to_string());
    let topographic_place_view_name = stop_place
        .get_child("TopographicPlaceView", &context.namespace)
        .map(|c| {
            c.get_child("Name", &context.namespace)
                .map(|s| s.text().to_string())
                .unwrap_or("".to_string())
        });
    direct_name.or(label).or(topographic_place_view_name)
}

fn read_stop_place_coord(context: &mut NetexContext, stop_place: &Element) -> Coord {
    // In some examples, Coords are only specified in Quay.
    // For the moment, only reading direct coord or return {0, 0}
    let location_node = stop_place.get_child("Centroid", &context.namespace);
    let location_node = match location_node {
        None => None,
        Some(n) => n.get_child("Location", &context.namespace),
    };
    Coord {
        lon: location_node
            .map(|c| {
                c.get_child("Longitude", &context.namespace)
                    .map_or(0.0, |s| s.text().parse().unwrap_or(0.0))
            })
            .unwrap_or(0.0),
        lat: location_node
            .map(|c| {
                c.get_child("Latitude", &context.namespace)
                    .map_or(0.0, |s| s.text().parse().unwrap_or(0.0))
            })
            .unwrap_or(0.0),
    }
}

fn read_stop_place(
    collections: &mut Collections,
    context: &mut NetexContext,
    stop_place: &Element,
) -> Result<()> {
    let stop_place_id = stop_place.attr("id").unwrap_or("");
    let stop_area = objects::StopArea {
        id: stop_place_id.to_string(),
        name: read_stop_place_name(context, stop_place).unwrap_or("".to_string()),
        codes: KeysValues::default(),
        object_properties: KeysValues::default(),
        comment_links: objects::CommentLinksT::default(),
        coord: read_stop_place_coord(context, stop_place),
        timezone: None,
        visible: true,
        geometry_id: None,
        equipment_id: None,
    };
    collections.stop_areas.push(stop_area).unwrap();
    for quai in stop_place
        .get_child("quays", &context.namespace)
        .unwrap()
        .children()
    {
        let stop_point = objects::StopPoint {
            id: quai.attr("id").unwrap().to_string(),
            name: read_stop_place_name(context, quai).unwrap_or("".to_string()),
            codes: KeysValues::default(),
            object_properties: KeysValues::default(),
            comment_links: objects::CommentLinksT::default(),
            coord: Coord {
                lon: quai
                    .get_child("Centroid", &context.namespace)
                    .unwrap()
                    .get_child("Location", &context.namespace)
                    .unwrap()
                    .get_child("Longitude", &context.namespace)
                    .unwrap()
                    .text()
                    .parse()
                    .unwrap(),
                lat: quai
                    .get_child("Centroid", &context.namespace)
                    .unwrap()
                    .get_child("Location", &context.namespace)
                    .unwrap()
                    .get_child("Latitude", &context.namespace)
                    .unwrap()
                    .text()
                    .parse()
                    .unwrap(),
            },
            stop_area_id: stop_place_id.to_string(),
            timezone: None,
            visible: true,
            geometry_id: None,
            equipment_id: None,
            fare_zone_id: None,
        };
        collections.stop_points.push(stop_point).unwrap();
    }
    Ok(())
}
