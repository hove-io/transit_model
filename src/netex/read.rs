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

use collection::{CollectionWithId};
use model::Collections;
use objects::{self, CommentLinksT, Contributor, Coord, KeysValues};
use std::collections::HashMap;
use std::fs::File;
use std::path;
use Result;
extern crate serde_json;
extern crate quick_xml;
use self::quick_xml::Reader;
extern crate minidom;
use self::minidom::Element;
use std::str::FromStr;

// TODO : a déplacer et mutualiser avec ce qui est fait dans le GTFS
#[derive(Deserialize, Debug)]
struct Dataset {
    dataset_id: String,
}

#[derive(Deserialize, Debug)]
struct Config {
    contributor: objects::Contributor,
    dataset: Dataset,
}

pub fn read_config<P: AsRef<path::Path>>(
    config_path: Option<P>,
) -> Result<(
    CollectionWithId<objects::Contributor>,
    CollectionWithId<objects::Dataset>,
)> {
    let contributor;
    let dataset;
    if let Some(config_path) = config_path {
        let json_config_file = File::open(config_path)?;
        let config: Config = serde_json::from_reader(json_config_file)?;
        info!("Reading dataset and contributor from config: {:?}", config);

        contributor = config.contributor;

        use chrono::{Duration, Utc};
        let duration = Duration::days(15);
        let today = Utc::today();
        let start_date = today - duration;
        let end_date = today + duration;
        dataset = objects::Dataset {
            id: config.dataset.dataset_id,
            contributor_id: contributor.id.clone(),
            start_date: start_date.naive_utc(),
            end_date: end_date.naive_utc(),
            dataset_type: None,
            extrapolation: false,
            desc: None,
            system: None,
        };
    } else {
        contributor = Contributor::default();
        dataset = objects::Dataset::default();
    }

    let contributors = CollectionWithId::new(vec![contributor])?;
    let datasets = CollectionWithId::new(vec![dataset])?;
    Ok((contributors, datasets))
}
// fin TODO : a déplacer et mutualiser avec ce qui est fait dans le GTFS


type RoutePointId = String;
type StopPointId = String;
type RoutePointMapping = HashMap<RoutePointId, StopPointId>;
type RouteLineMap = HashMap<String, String>;

struct NetexContext{
    namespace: String,
    routepoint_mapping : RoutePointMapping,
    route_line_map : RouteLineMap,
}

pub fn read_netex_file<P: AsRef<path::Path>>(
    collections: &mut Collections,
    path: P,
) {
    let mut reader = Reader::from_file(path).unwrap();
    let root  = Element::from_reader(&mut reader).unwrap();

    let mut context = NetexContext{
        namespace: root.ns().unwrap(),
        routepoint_mapping : HashMap::new(),
        route_line_map : HashMap::new(),
    };

    for frame in root.get_child("dataObjects", context.namespace.as_str()).unwrap().children() {
        match frame.name() {
            "CompositeFrame" => read_composite_data_frame(collections, &mut context, frame),
            _ => (),
        }        
    }
}


fn read_composite_data_frame(
    collections: &mut Collections,
    context: &mut NetexContext,
    composite_frame: &Element,
) {
    for frame in composite_frame.get_child("frames", &mut context.namespace).unwrap().children() {
        match frame.name() {
            "SiteFrame" => read_site_frame(collections, context, &frame),
            "ServiceFrame" => read_service_frame(collections, context, &frame),
            "ServiceCalendarFrame" => read_service_calendar_frame(collections, context, &frame),
            "TimetableFrame" => read_time_table_frame(collections, context, &frame),
            "ResourceFrame" => read_resource_frame(collections, context, &frame),
            _ => (),
        }
    }
}

fn read_resource_frame(
    collections: &mut Collections,
    context: &mut NetexContext,
    resource_frame: &Element,
) {
    let organisations = resource_frame.get_child("organisations", &context.namespace).unwrap();
    read_organisations(
        collections,
        context,
        &organisations,
    );
}

fn read_service_frame(
    collections: &mut Collections,
    context: &mut NetexContext,
    service_frame: &Element,
) {
    let network_node = service_frame.get_child("Network", &context.namespace).unwrap();
    let network = read_network(
        collections,
        context,
        &network_node, 
    );
    let lines_node = service_frame.get_child("lines", &context.namespace).unwrap();
    read_lines_and_commercial_modes(
        collections,
        context,
        &lines_node,
        &network.id,
    );
    let stop_assignments_node = service_frame.get_child("stopAssignments", &context.namespace).unwrap();
    read_stop_assignements(
        collections,
        context,
        &stop_assignments_node, 
    );
    let routes_node = service_frame.get_child("routes", &context.namespace).unwrap();
    read_routes(
        collections,
        context,
        &routes_node,
    );
    let connections_node = service_frame.get_child("connections", &context.namespace).unwrap();
    read_connections(
        collections,
        context,
        &connections_node,
    );
    collections.networks.push(network).unwrap();
}

fn read_service_calendar_frame(
    _collections: &mut Collections,
    _context: &mut NetexContext,
    _service_frame: &Element,
) {


}


fn read_stop_assignements(
    _collections: &mut Collections,
    context: &mut NetexContext,
    stop_assignments: &Element,
) {
    for node in stop_assignments.children() {
        // assuming all children are PassengerStopAssignment
        context.routepoint_mapping.insert(
            node
                .get_child("ScheduledStopPointRef", &context.namespace)
                .unwrap().attr("ref").unwrap().to_string(),
            node
                .get_child("QuayRef", &context.namespace)
                .unwrap().attr("ref").unwrap().to_string(),
        );
    }
}

fn read_time_table_frame(
    collections: &mut Collections,
    context: &mut NetexContext,
    time_table_frame: &Element,
) {
    let vj_node = time_table_frame.get_child("vehicleJourneys", &context.namespace).unwrap();
    read_vehicle_journeys(
        collections,
        context,
        &vj_node,
    );
}

fn read_organisations(
    collections: &mut Collections,
    context: &mut NetexContext,
    organisations: &Element,
) {
    for node in organisations.children() {
        // for the moment, assuming all children are Operator
        collections.companies.push(
            objects::Company {
                id: node.attr("id").unwrap().to_string(),
                name: node.get_child("Name", &context.namespace).unwrap().text().to_string(),
                address: None,
                url: None,
                mail: None,
                phone: None,
            }
        ).unwrap();
    }
}

fn read_vehicle_journeys(
    collections: &mut Collections,
    context: &mut NetexContext,
    vehicle_journeys: &Element,
) {
    for node in vehicle_journeys.children() {
        // assuming all children are ServiceJourney
        if node.name() != "ServiceJourney" {
            panic!("read_vehicle_journeys : node is expected to be ServiceJourney");
        };
        let route_id = node.get_child("RouteRef", &context.namespace).unwrap().attr("ref").unwrap();
        let mut vj = objects::VehicleJourney {
            id : node.attr("id").unwrap().to_string(),
            codes: KeysValues::default(),
            object_properties: KeysValues::default(),
            comment_links: CommentLinksT::default(),
            route_id: route_id.to_string(),
            physical_mode_id: netex_mode_to_physical_mode_id("tram").to_string(),
            dataset_id: "default_dataset".to_string(),
            service_id: "".to_string(), 
            headsign: None,
            block_id: None,
            company_id: node.get_child("OperatorRef", &context.namespace).unwrap().attr("ref").unwrap().to_string(),
            trip_property_id: None,
            geometry_id: None,
            stop_times: vec![],
        };
        let calls_node = node.get_child("calls", &context.namespace).unwrap();
        read_stop_times(
            collections,
            context,
            &mut vj,
            &calls_node,
        );
        collections.vehicle_journeys.push(vj).unwrap();
    }
}

fn read_stop_times(
    collections: &mut Collections,
    context: &mut NetexContext,
    vj: &mut objects::VehicleJourney,
    calls: &Element,
) {
    let mut stop_sequence = 0;
    for call in calls.children() {
        // assuming all children are Call
        stop_sequence = stop_sequence + 1;
        let routepoint_id = call.get_child("ScheduledStopPointRef", &context.namespace).unwrap().attr("ref").unwrap();
        let stoppoint_id = context.routepoint_mapping.get(routepoint_id).unwrap();
        vj.stop_times.push(
            objects::StopTime {
                stop_point_idx: collections
                    .stop_points
                    .get_idx(&stoppoint_id)
                    .unwrap(),
                sequence: stop_sequence,
                arrival_time: objects::Time::from_str(
                    call
                        .get_child("Arrival", &context.namespace).unwrap()
                        .get_child("Time", &context.namespace).unwrap()
                        .text().as_ref()
                ).unwrap(),
                departure_time: objects::Time::from_str(
                    call
                        .get_child("Departure", &context.namespace).unwrap()
                        .get_child("Time", &context.namespace).unwrap()
                        .text().as_ref()
                ).unwrap(),
                boarding_duration: 0,
                alighting_duration: 0,
                pickup_type: 0,
                drop_off_type: 0,
                datetime_estimated: false,
                local_zone_id: None,
            }
        );
    }
}

fn read_network(
    _collections: &mut Collections,
    context: &mut NetexContext,
    network: &Element
) -> objects::Network {
    objects::Network {
        id: network.get_child("PrivateCode", &context.namespace).unwrap().text().to_string(),
        name: network.get_child("Name", &context.namespace).unwrap().text().to_string(),
        url: None,
        codes: KeysValues::default(),
        timezone: None,
        lang: None,
        phone: None,
        address: None,
        sort_order: None,
    }
}

fn netex_mode_to_physical_mode_id(netex_mode : &str) -> String {
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
)  {
    for l in lines.children() {
        let mode_name = l.get_child("TransportMode", &context.namespace).unwrap().text();
        if collections.commercial_modes.get(&mode_name).is_none() {
            collections.commercial_modes.push(
                objects::CommercialMode{
                    id: mode_name.to_string(),
                    name: mode_name.to_string(),
                }
            ).unwrap();
        };
        if collections.physical_modes.get(&netex_mode_to_physical_mode_id(&mode_name)).is_none() {
            collections.physical_modes.push(
                objects::PhysicalMode{
                    id: netex_mode_to_physical_mode_id(&mode_name).to_string(),
                    name: netex_mode_to_physical_mode_id(&mode_name).to_string(),
                    co2_emission: None,
                }
            ).unwrap();
        };
        let line =  objects::Line {
            id: l.attr("id").unwrap().to_string(),
            code: Some(l.get_child("PrivateCode", &context.namespace).unwrap().text().to_string()),
            name: l.get_child("Name", &context.namespace).unwrap().text().to_string(),
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
            commercial_mode_id : mode_name.to_string(),
            geometry_id: None,
            opening_time: None,
            closing_time: None,
        };
        for r in l.get_child("routes", &context.namespace).unwrap().children(){
            context.route_line_map.insert(
                    r.attr("ref").unwrap().to_string(),
                    line.id.to_string(),
            );
        };
        collections.lines.push(line).unwrap();
    };
}

fn read_routes(
    collections: &mut Collections,
    context: &mut NetexContext,
    routes: &Element,
) {
    for r in routes.children() {
        let route =  objects::Route {
            id: r.attr("id").unwrap().to_string(),
            name: r.get_child("Name", &context.namespace).unwrap().text().to_string(),
            codes: KeysValues::default(),
            object_properties: KeysValues::default(),
            direction_type: None,
            comment_links: CommentLinksT::default(),
            line_id: context.route_line_map.get(r.attr("id").unwrap()).unwrap().to_string(),
            geometry_id: None,
            destination_id: None,
        };
        collections.routes.push(route).unwrap();
    }
}

fn read_connections(
    collections: &mut Collections,
    context: &mut NetexContext,
    connections: &Element,
) {
    for cnx in connections.children() {
        let transfer =  objects::Transfer {
            from_stop_id: cnx
                            .get_child("From", &context.namespace).unwrap()
                            .get_child("StopPlaceRef", &context.namespace).unwrap()
                            .attr("ref").unwrap().to_string(),
            to_stop_id: cnx
                            .get_child("To", &context.namespace).unwrap()
                            .get_child("StopPlaceRef", &context.namespace).unwrap()
                            .attr("ref").unwrap().to_string(),
            min_transfer_time: None,
            real_min_transfer_time: None,
            equipment_id: None,
        };
        collections.transfers.push(transfer).unwrap();
    }
}

fn read_site_frame(
    collections: &mut Collections,
    context: &mut NetexContext,
    site_frame: &Element,
) {
    for stop_place in site_frame.get_child("stopPlaces", &context.namespace).unwrap().children() {
        read_stop_place(
            collections,
            context,
            &stop_place,
        );
    }
}

fn read_stop_place(
    collections: &mut Collections,
    context: &mut NetexContext,
    stop_place: &Element,
) {
    let stop_place_id = stop_place.attr("id").unwrap();
    let stop_area = objects::StopArea {
        id: stop_place_id.to_string(),
        name: stop_place.get_child("Name", &context.namespace).unwrap().text().to_string(),
        codes: KeysValues::default(),
        object_properties: KeysValues::default(),
        comment_links: objects::CommentLinksT::default(),
        coord: Coord {
            lon: stop_place
                    .get_child("Centroid", &context.namespace).unwrap()
                    .get_child("Location", &context.namespace).unwrap()
                    .get_child("Longitude", &context.namespace).unwrap()
                    .text()
                    .parse().unwrap(),
            lat: stop_place
                    .get_child("Centroid", &context.namespace).unwrap()
                    .get_child("Location", &context.namespace).unwrap()
                    .get_child("Latitude", &context.namespace).unwrap()
                    .text()
                    .parse().unwrap(),
        },
        timezone: None,
        visible: true,
        geometry_id: None,
        equipment_id: None,
    };
    collections.stop_areas.push(stop_area).unwrap();
    for quai in stop_place.get_child("quays", &context.namespace).unwrap().children() {
        let stop_point = objects::StopPoint {
            id: quai.attr("id").unwrap().to_string(),
            name: quai.get_child("Name", &context.namespace).unwrap().text().to_string(),
            codes: KeysValues::default(),
            object_properties: KeysValues::default(),
            comment_links: objects::CommentLinksT::default(),
            coord: Coord {
                lon: quai
                        .get_child("Centroid", &context.namespace).unwrap()
                        .get_child("Location", &context.namespace).unwrap()
                        .get_child("Longitude", &context.namespace).unwrap()
                        .text()
                        .parse().unwrap(),
                lat: quai
                        .get_child("Centroid", &context.namespace).unwrap()
                        .get_child("Location", &context.namespace).unwrap()
                        .get_child("Latitude", &context.namespace).unwrap()
                        .text()
                        .parse().unwrap(),
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
}