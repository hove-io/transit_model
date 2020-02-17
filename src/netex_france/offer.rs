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

use crate::{
    netex_france::{
        exporter::{Exporter, ObjectType},
        NetexMode, StopExporter,
    },
    objects::{Coord, Line, Route, StopTime, VehicleJourney},
    Model, Result,
};
use log::warn;
use minidom::{Element, Node};
use proj::Proj;
use transit_model_collection::Idx;
use transit_model_relations::IdxSet;

// A journey pattern is the sequence of stops of a particular trip.
// Modelization of JourneyPattern by a VehicleJourney is sufficient for now.
type JourneyPattern = VehicleJourney;

pub struct OfferExporter<'a> {
    model: &'a Model,
    converter: Proj,
}

// Publicly exposed methods
impl<'a> OfferExporter<'a> {
    pub fn new(model: &'a Model) -> Result<Self> {
        let converter = Exporter::get_coordinates_converter()?;
        let exporter = OfferExporter { model, converter };
        Ok(exporter)
    }
    pub fn export(&self, line_idx: Idx<Line>) -> Result<Vec<Element>> {
        let route_elements = self.export_routes(line_idx);
        let journey_patterns: Vec<(Idx<JourneyPattern>, Vec<Idx<VehicleJourney>>)> = self
            .model
            .get_corresponding_from_idx(line_idx)
            .into_iter()
            .flat_map(|route_idx| self.calculate_journey_patterns(route_idx))
            .collect();
        let journey_pattern_indexes: Vec<Idx<JourneyPattern>> = journey_patterns
            .iter()
            .map(|(journey_pattern_idx, _)| *journey_pattern_idx)
            .collect();
        let service_journey_pattern_elements =
            self.export_journey_patterns(&journey_pattern_indexes);
        let scheduled_stop_point_elements = journey_pattern_indexes
            .iter()
            .map(|journey_pattern_idx| self.export_scheduled_stop_points(*journey_pattern_idx))
            .try_fold::<_, _, Result<Vec<Element>>>(
                Vec::new(),
                |mut scheduled_stop_point_elements, elements| {
                    scheduled_stop_point_elements.extend(elements?);
                    Ok(scheduled_stop_point_elements)
                },
            )?;
        let passenger_stop_assignment_elements = journey_pattern_indexes
            .iter()
            .map(|journey_pattern_idx| self.export_passenger_stop_assignments(*journey_pattern_idx))
            .fold(
                Vec::new(),
                |mut passenger_stop_assignment_elements, elements| {
                    passenger_stop_assignment_elements.extend(elements);
                    passenger_stop_assignment_elements
                },
            );

        let mut elements = route_elements;
        elements.extend(service_journey_pattern_elements);
        elements.extend(scheduled_stop_point_elements);
        elements.extend(passenger_stop_assignment_elements);
        Ok(elements)
    }
}

// Internal methods
impl<'a> OfferExporter<'a> {
    fn export_routes(&self, line_idx: Idx<Line>) -> Vec<Element> {
        let route_indexes: IdxSet<Route> = self.model.get_corresponding_from_idx(line_idx);
        route_indexes
            .into_iter()
            .map(|route_idx| self.export_route(route_idx))
            .collect()
    }

    fn export_route(&self, route_idx: Idx<Route>) -> Element {
        let route = &self.model.routes[route_idx];
        let element_builder = Element::builder(ObjectType::Route.to_string())
            .attr("id", Exporter::generate_id(&route.id, ObjectType::Route))
            .attr("version", "any");
        let element_builder = element_builder.append(Self::generate_route_name(&route.name));
        let element_builder = element_builder.append(Self::generate_distance());
        let element_builder = element_builder.append(Self::generate_line_ref(&route.line_id));
        let element_builder = if let Some(direction_type_element) =
            Self::generate_direction_type(route.direction_type.as_ref().map(String::as_str))
        {
            element_builder.append(direction_type_element)
        } else {
            element_builder
        };
        element_builder.build()
    }

    fn export_journey_patterns(
        &self,
        journey_pattern_indexes: &[Idx<JourneyPattern>],
    ) -> Vec<Element> {
        journey_pattern_indexes
            .iter()
            .map(|journey_pattern_idx| self.export_journey_pattern(*journey_pattern_idx))
            .collect()
    }

    fn export_journey_pattern(&self, journey_pattern_idx: Idx<JourneyPattern>) -> Element {
        let journey_pattern = &self.model.vehicle_journeys[journey_pattern_idx];
        let points_in_sequence = Element::builder("pointsInSequence")
            .append_all(self.export_stop_points_in_journey_pattern(journey_pattern_idx))
            .build();
        Element::builder(ObjectType::ServiceJourneyPattern.to_string())
            .attr(
                "id",
                Exporter::generate_id(&journey_pattern.id, ObjectType::ServiceJourneyPattern),
            )
            .attr("version", "any")
            .append(Self::generate_distance())
            .append(Self::generate_route_ref(&journey_pattern.route_id))
            .append(points_in_sequence)
            .build()
    }

    fn export_stop_points_in_journey_pattern(
        &self,
        journey_pattern_idx: Idx<JourneyPattern>,
    ) -> Vec<Element> {
        let vehicle_journey = &self.model.vehicle_journeys[journey_pattern_idx];
        vehicle_journey
            .stop_times
            .iter()
            .map(|stop_time| {
                self.export_stop_point_in_journey_pattern(&vehicle_journey.id, stop_time)
            })
            .collect()
    }

    fn export_stop_point_in_journey_pattern(
        &self,
        vehicle_journey_id: &'a str,
        stop_time: &'a StopTime,
    ) -> Element {
        Element::builder(ObjectType::StopPointInJourneyPattern.to_string())
            .attr(
                "id",
                Self::generate_stop_sequence_id(
                    &vehicle_journey_id,
                    stop_time.sequence,
                    ObjectType::StopPointInJourneyPattern,
                ),
            )
            .attr("version", "any")
            .attr("order", stop_time.sequence + 1)
            .append(Self::generate_scheduled_stop_point_ref(
                &vehicle_journey_id,
                stop_time.sequence,
            ))
            .append(Self::generate_for_alighting(stop_time.drop_off_type))
            .append(Self::generate_for_boarding(stop_time.pickup_type))
            .build()
    }

    fn export_scheduled_stop_points(
        &self,
        journey_pattern_idx: Idx<JourneyPattern>,
    ) -> Result<Vec<Element>> {
        let vehicle_journey = &self.model.vehicle_journeys[journey_pattern_idx];
        vehicle_journey
            .stop_times
            .iter()
            .map(|stop_time| self.export_scheduled_stop_point(&vehicle_journey.id, stop_time))
            .collect()
    }

    fn export_scheduled_stop_point(
        &self,
        vehicle_journey_id: &'a str,
        stop_time: &'a StopTime,
    ) -> Result<Element> {
        let element_builder = Element::builder(ObjectType::ScheduledStopPoint.to_string())
            .attr(
                "id",
                Self::generate_stop_sequence_id(
                    &vehicle_journey_id,
                    stop_time.sequence,
                    ObjectType::ScheduledStopPoint,
                ),
            )
            .attr("version", "any");
        let location_element =
            self.generate_location(&self.model.stop_points[stop_time.stop_point_idx].coord)?;
        let element_builder = element_builder.append(location_element);
        Ok(element_builder.build())
    }

    fn export_passenger_stop_assignments(
        &self,
        journey_pattern_idx: Idx<JourneyPattern>,
    ) -> Vec<Element> {
        let vehicle_journey = &self.model.vehicle_journeys[journey_pattern_idx];
        vehicle_journey
            .stop_times
            .iter()
            .map(|stop_time| self.export_passenger_stop_assignment(vehicle_journey, stop_time))
            .collect()
    }

    fn export_passenger_stop_assignment(
        &self,
        vehicle_journey: &'a VehicleJourney,
        stop_time: &'a StopTime,
    ) -> Element {
        let element_builder = Element::builder(ObjectType::PassengerStopAssignment.to_string())
            .attr(
                "id",
                Self::generate_stop_sequence_id(
                    &vehicle_journey.id,
                    stop_time.sequence,
                    ObjectType::PassengerStopAssignment,
                ),
            )
            .attr("version", "any")
            .attr("order", stop_time.sequence + 1);
        let element_builder = element_builder.append(Self::generate_scheduled_stop_point_ref(
            &vehicle_journey.id,
            stop_time.sequence,
        ));
        let element_builder = if let Some(stop_place_ref_element) = self.generate_stop_place_ref(
            &self.model.stop_points[stop_time.stop_point_idx].stop_area_id,
            &vehicle_journey.physical_mode_id,
        ) {
            element_builder.append(stop_place_ref_element)
        } else {
            element_builder
        };
        let element_builder = element_builder.append(Self::generate_quay_ref(
            &self.model.stop_points[stop_time.stop_point_idx].id,
        ));
        element_builder.build()
    }

    fn generate_route_name(route_name: &'a str) -> Element {
        Element::builder("Name")
            .append(Node::Text(route_name.to_owned()))
            .build()
    }

    fn generate_distance() -> Element {
        Element::builder("Distance")
            .append(Node::Text(String::from("0")))
            .build()
    }

    fn generate_line_ref(line_id: &str) -> Element {
        Element::builder("LineRef")
            .attr("ref", Exporter::generate_id(line_id, ObjectType::Line))
            .build()
    }

    fn generate_route_ref(route_id: &str) -> Element {
        Element::builder("RouteRef")
            .attr("ref", Exporter::generate_id(route_id, ObjectType::Route))
            .build()
    }

    fn generate_scheduled_stop_point_ref(vehicle_journey_id: &'a str, sequence: u32) -> Element {
        Element::builder("ScheduledStopPointRef")
            .attr(
                "ref",
                Self::generate_stop_sequence_id(
                    vehicle_journey_id,
                    sequence,
                    ObjectType::ScheduledStopPoint,
                ),
            )
            .build()
    }

    fn generate_stop_place_ref(
        &self,
        stop_area_id: &'a str,
        physical_mode_id: &'a str,
    ) -> Option<Element> {
        let netex_mode = NetexMode::from_physical_mode_id(physical_mode_id)?;
        let stop_place_id = StopExporter::generate_stop_place_id(stop_area_id, netex_mode);
        let element = Element::builder("StopPlaceRef")
            .attr("ref", stop_place_id)
            .build();
        Some(element)
    }

    fn generate_quay_ref(stop_id: &'a str) -> Element {
        Element::builder("QuayRef")
            .attr("ref", Exporter::generate_id(stop_id, ObjectType::Quay))
            .build()
    }

    fn generate_direction_type(direction_type: Option<&str>) -> Option<Element> {
        direction_type
            .and_then(|direction_type| match direction_type {
                "forward" => Some(String::from("inbound")),
                "backward" => Some(String::from("outbound")),
                "inbound" | "outbound" | "clockwise" | "anticlockwise" => {
                    Some(String::from(direction_type))
                }
                dt => {
                    warn!("DirectionType '{}' not supported", dt);
                    None
                }
            })
            .map(|direction_type| {
                Element::builder("DirectionType")
                    .append(Node::Text(direction_type))
                    .build()
            })
    }

    fn generate_stop_sequence_id(id: &str, sequence: u32, object_type: ObjectType) -> String {
        let order_id = format!("{}_{}", id, sequence);
        Exporter::generate_id(&order_id, object_type)
    }

    fn generate_location(&self, coord: &'a Coord) -> Result<Element> {
        let coord_epsg2154 = self.converter.convert(*coord)?;
        let coord_text = Node::Text(format!("{} {}", coord_epsg2154.x(), coord_epsg2154.y()));
        let pos = Element::builder("gml:pos")
            .attr("srsName", "EPSG:2154")
            .append(coord_text)
            .build();
        let location = Element::builder("Location").append(pos).build();
        Ok(location)
    }

    fn generate_for_alighting(drop_off_type: u8) -> Element {
        let is_alighting = if drop_off_type == 0 { "true" } else { "false" };
        Element::builder("ForAlighting")
            .append(Node::Text(is_alighting.to_owned()))
            .build()
    }

    fn generate_for_boarding(pickup_type: u8) -> Element {
        let is_boarding = if pickup_type == 0 { "true" } else { "false" };
        Element::builder("ForBoarding")
            .append(Node::Text(is_boarding.to_owned()))
            .build()
    }

    fn calculate_journey_patterns(
        &self,
        route_idx: Idx<Route>,
    ) -> Vec<(Idx<JourneyPattern>, Vec<Idx<VehicleJourney>>)> {
        let same_stop_time = |a: &StopTime, b: &StopTime| {
            a.stop_point_idx == b.stop_point_idx
                && a.pickup_type == b.pickup_type
                && a.drop_off_type == b.drop_off_type
                && a.local_zone_id == b.local_zone_id
        };
        let mut vehicle_journey_indexes: Vec<Idx<VehicleJourney>> = self
            .model
            .get_corresponding_from_idx(route_idx)
            .into_iter()
            .collect();
        vehicle_journey_indexes.sort_unstable_by_key(|vehicle_journey_idx| {
            &self.model.vehicle_journeys[*vehicle_journey_idx].id
        });
        let mut journey_patterns: Vec<(Idx<JourneyPattern>, Vec<Idx<VehicleJourney>>)> = Vec::new();
        for vehicle_journey_idx in vehicle_journey_indexes {
            let vehicle_journey = &self.model.vehicle_journeys[vehicle_journey_idx];
            let is_same_journey_pattern = |journey_pattern_idx: Idx<VehicleJourney>| {
                let journey_pattern_vj = &self.model.vehicle_journeys[journey_pattern_idx];
                vehicle_journey.stop_times.len() == journey_pattern_vj.stop_times.len()
                    && vehicle_journey
                        .stop_times
                        .iter()
                        .zip(&journey_pattern_vj.stop_times)
                        .all(|(stop_time_a, stop_time_b)| same_stop_time(stop_time_a, stop_time_b))
            };
            let mut is_new = true;
            for journey_pattern in &mut journey_patterns {
                let journey_pattern_idx = journey_pattern.0;
                let vehicle_journeys = &mut journey_pattern.1;
                if is_same_journey_pattern(journey_pattern_idx) {
                    is_new = false;
                    vehicle_journeys.push(vehicle_journey_idx);
                }
            }
            if is_new {
                // If no existing Journey Pattern could be found,
                // then the current Vehicle Journey become a new Journey Pattern
                journey_patterns.push((vehicle_journey_idx, vec![vehicle_journey_idx]));
            }
        }
        journey_patterns
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::Collections,
        objects::{
            Calendar, CommercialMode, Company, Contributor, Dataset, Date, Network, PhysicalMode,
            StopArea, StopPoint, StopTimePrecision, Time,
        },
    };
    use pretty_assertions::assert_eq;
    use transit_model_collection::CollectionWithId;

    fn default_collections() -> Collections {
        let mut collections = Collections::default();
        collections.contributors = CollectionWithId::from(Contributor {
            id: String::from("contributor_id"),
            ..Default::default()
        });
        collections.datasets = CollectionWithId::from(Dataset {
            id: String::from("dataset_id"),
            contributor_id: String::from("contributor_id"),
            ..Default::default()
        });
        collections.companies = CollectionWithId::from(Company {
            id: String::from("company_id"),
            ..Default::default()
        });
        collections.physical_modes = CollectionWithId::from(PhysicalMode {
            id: String::from("Bus"),
            ..Default::default()
        });
        collections.commercial_modes = CollectionWithId::from(CommercialMode {
            id: String::from("MagicBus"),
            ..Default::default()
        });
        collections.stop_areas = CollectionWithId::new(vec![
            StopArea {
                id: String::from("sa_id_1"),
                ..Default::default()
            },
            StopArea {
                id: String::from("sa_id_2"),
                ..Default::default()
            },
        ])
        .unwrap();
        collections.stop_points = CollectionWithId::new(vec![
            StopPoint {
                id: String::from("sp_id_1"),
                stop_area_id: String::from("sa_id_1"),
                ..Default::default()
            },
            StopPoint {
                id: String::from("sp_id_2"),
                stop_area_id: String::from("sa_id_2"),
                ..Default::default()
            },
        ])
        .unwrap();
        collections.networks = CollectionWithId::from(Network {
            id: String::from("network_id"),
            ..Default::default()
        });
        collections.lines = CollectionWithId::from(Line {
            id: String::from("line_id"),
            network_id: String::from("network_id"),
            commercial_mode_id: String::from("MagicBus"),
            ..Default::default()
        });
        collections.routes = CollectionWithId::from(Route {
            id: String::from("route_id"),
            line_id: String::from("line_id"),
            ..Default::default()
        });
        collections.calendars = CollectionWithId::from(Calendar {
            id: String::from("service_id"),
            dates: vec![Date::from_ymd(2020, 1, 1)].into_iter().collect(),
        });
        collections.vehicle_journeys = CollectionWithId::from(VehicleJourney {
            id: String::from("vj_id_1"),
            route_id: String::from("route_id"),
            service_id: String::from("service_id"),
            company_id: String::from("company_id"),
            dataset_id: String::from("dataset_id"),
            physical_mode_id: String::from("Bus"),
            stop_times: vec![
                StopTime {
                    stop_point_idx: collections.stop_points.get_idx("sp_id_1").unwrap(),
                    sequence: 0,
                    arrival_time: Time::new(0, 0, 0),
                    departure_time: Time::new(0, 0, 0),
                    boarding_duration: 0,
                    alighting_duration: 0,
                    pickup_type: 0,
                    drop_off_type: 0,
                    datetime_estimated: false,
                    local_zone_id: Some(1),
                    precision: Some(StopTimePrecision::Exact),
                },
                StopTime {
                    stop_point_idx: collections.stop_points.get_idx("sp_id_2").unwrap(),
                    sequence: 1,
                    arrival_time: Time::new(0, 0, 0),
                    departure_time: Time::new(0, 0, 0),
                    boarding_duration: 0,
                    alighting_duration: 0,
                    pickup_type: 1,
                    drop_off_type: 1,
                    datetime_estimated: false,
                    local_zone_id: Some(1),
                    precision: Some(StopTimePrecision::Exact),
                },
            ],
            ..Default::default()
        });
        collections
    }

    #[test]
    fn same_journey_pattern() {
        let mut collections = default_collections();
        collections
            .vehicle_journeys
            .push(VehicleJourney {
                id: String::from("vj_id_2"),
                route_id: String::from("route_id"),
                service_id: String::from("service_id"),
                company_id: String::from("company_id"),
                dataset_id: String::from("dataset_id"),
                physical_mode_id: String::from("Bus"),
                stop_times: vec![
                    StopTime {
                        stop_point_idx: collections.stop_points.get_idx("sp_id_1").unwrap(),
                        sequence: 0,
                        arrival_time: Time::new(0, 0, 0),
                        departure_time: Time::new(0, 0, 0),
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 0,
                        drop_off_type: 0,
                        datetime_estimated: false,
                        local_zone_id: Some(1),
                        precision: Some(StopTimePrecision::Exact),
                    },
                    StopTime {
                        stop_point_idx: collections.stop_points.get_idx("sp_id_2").unwrap(),
                        sequence: 1,
                        arrival_time: Time::new(0, 0, 0),
                        departure_time: Time::new(0, 0, 0),
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 1,
                        drop_off_type: 1,
                        datetime_estimated: false,
                        local_zone_id: Some(1),
                        precision: Some(StopTimePrecision::Exact),
                    },
                ],
                ..Default::default()
            })
            .unwrap();
        let model = Model::new(collections).unwrap();
        let offer_exporter = OfferExporter::new(&model).unwrap();
        let route_idx = model.routes.get_idx("route_id").unwrap();
        let journey_pattern_indexes = offer_exporter.calculate_journey_patterns(route_idx);
        assert_eq!(1, journey_pattern_indexes.len());
        let journey_pattern_id = &model.vehicle_journeys[journey_pattern_indexes[0].0].id;
        assert_eq!("vj_id_1", journey_pattern_id);
        assert_eq!(2, journey_pattern_indexes[0].1.len());
        let vehicle_journey_id = &model.vehicle_journeys[journey_pattern_indexes[0].1[0]].id;
        assert_eq!("vj_id_1", vehicle_journey_id);
        let vehicle_journey_id = &model.vehicle_journeys[journey_pattern_indexes[0].1[1]].id;
        assert_eq!("vj_id_2", vehicle_journey_id);
    }

    #[test]
    fn journey_patterns_with_different_number_stop_times() {
        let mut collections = default_collections();
        collections
            .vehicle_journeys
            .push(VehicleJourney {
                id: String::from("vj_id_2"),
                route_id: String::from("route_id"),
                service_id: String::from("service_id"),
                company_id: String::from("company_id"),
                dataset_id: String::from("dataset_id"),
                physical_mode_id: String::from("Bus"),
                stop_times: vec![StopTime {
                    stop_point_idx: collections.stop_points.get_idx("sp_id_1").unwrap(),
                    sequence: 0,
                    arrival_time: Time::new(0, 0, 0),
                    departure_time: Time::new(0, 0, 0),
                    boarding_duration: 0,
                    alighting_duration: 0,
                    pickup_type: 0,
                    drop_off_type: 0,
                    datetime_estimated: false,
                    local_zone_id: Some(1),
                    precision: Some(StopTimePrecision::Exact),
                }],
                ..Default::default()
            })
            .unwrap();
        let model = Model::new(collections).unwrap();
        let offer_exporter = OfferExporter::new(&model).unwrap();
        let route_idx = model.routes.get_idx("route_id").unwrap();
        let journey_pattern_indexes = offer_exporter.calculate_journey_patterns(route_idx);
        assert_eq!(2, journey_pattern_indexes.len());

        let journey_pattern_id = &model.vehicle_journeys[journey_pattern_indexes[0].0].id;
        assert_eq!("vj_id_1", journey_pattern_id);
        assert_eq!(1, journey_pattern_indexes[0].1.len());
        let vehicle_journey_id = &model.vehicle_journeys[journey_pattern_indexes[0].1[0]].id;
        assert_eq!("vj_id_1", vehicle_journey_id);

        let journey_pattern_id = &model.vehicle_journeys[journey_pattern_indexes[1].0].id;
        assert_eq!("vj_id_2", journey_pattern_id);
        assert_eq!(1, journey_pattern_indexes[1].1.len());
        let vehicle_journey_id = &model.vehicle_journeys[journey_pattern_indexes[1].1[0]].id;
        assert_eq!("vj_id_2", vehicle_journey_id);
    }

    #[test]
    fn journey_patterns_with_different_stop_time_properties() {
        let mut collections = default_collections();
        collections
            .vehicle_journeys
            .push(VehicleJourney {
                id: String::from("vj_id_2"),
                route_id: String::from("route_id"),
                service_id: String::from("service_id"),
                company_id: String::from("company_id"),
                dataset_id: String::from("dataset_id"),
                physical_mode_id: String::from("Bus"),
                stop_times: vec![
                    StopTime {
                        stop_point_idx: collections.stop_points.get_idx("sp_id_1").unwrap(),
                        sequence: 0,
                        arrival_time: Time::new(0, 0, 0),
                        departure_time: Time::new(0, 0, 0),
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 0,
                        drop_off_type: 0,
                        datetime_estimated: false,
                        local_zone_id: Some(1),
                        precision: Some(StopTimePrecision::Exact),
                    },
                    StopTime {
                        stop_point_idx: collections.stop_points.get_idx("sp_id_2").unwrap(),
                        sequence: 1,
                        arrival_time: Time::new(0, 0, 0),
                        departure_time: Time::new(0, 0, 0),
                        boarding_duration: 0,
                        alighting_duration: 0,
                        // This pickup type is different from 'vj_id_1'
                        pickup_type: 0,
                        drop_off_type: 1,
                        datetime_estimated: false,
                        local_zone_id: Some(1),
                        precision: Some(StopTimePrecision::Exact),
                    },
                ],
                ..Default::default()
            })
            .unwrap();
        let model = Model::new(collections).unwrap();
        let offer_exporter = OfferExporter::new(&model).unwrap();
        let route_idx = model.routes.get_idx("route_id").unwrap();
        let journey_pattern_indexes = offer_exporter.calculate_journey_patterns(route_idx);
        assert_eq!(2, journey_pattern_indexes.len());

        let journey_pattern_id = &model.vehicle_journeys[journey_pattern_indexes[0].0].id;
        assert_eq!("vj_id_1", journey_pattern_id);
        assert_eq!(1, journey_pattern_indexes[0].1.len());
        let vehicle_journey_id = &model.vehicle_journeys[journey_pattern_indexes[0].1[0]].id;
        assert_eq!("vj_id_1", vehicle_journey_id);

        let journey_pattern_id = &model.vehicle_journeys[journey_pattern_indexes[1].0].id;
        assert_eq!("vj_id_2", journey_pattern_id);
        assert_eq!(1, journey_pattern_indexes[1].1.len());
        let vehicle_journey_id = &model.vehicle_journeys[journey_pattern_indexes[1].1[0]].id;
        assert_eq!("vj_id_2", vehicle_journey_id);
    }
}
