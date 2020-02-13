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
    netex_france::exporter::{Exporter, ObjectType},
    objects::{Line, Route, StopTime, VehicleJourney},
    Model, Result,
};
use log::warn;
use minidom::{Element, Node};
use transit_model_collection::Idx;
use transit_model_relations::IdxSet;

// A journey pattern is the sequence of stops of a particular trip.
// Modelization of JourneyPattern by a VehicleJourney is sufficient for now.
type JourneyPattern = VehicleJourney;

pub struct OfferExporter<'a> {
    model: &'a Model,
}

// Publicly exposed methods
impl<'a> OfferExporter<'a> {
    pub fn new(model: &'a Model) -> Self {
        OfferExporter { model }
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

        let mut elements = route_elements;
        elements.extend(service_journey_pattern_elements);
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
        Element::builder(ObjectType::ServiceJourneyPattern.to_string())
            .attr(
                "id",
                Exporter::generate_id(&journey_pattern.id, ObjectType::ServiceJourneyPattern),
            )
            .attr("version", "any")
            .append(Self::generate_distance())
            .append(Self::generate_route_ref(&journey_pattern.route_id))
            .build()
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
        let offer_exporter = OfferExporter::new(&model);
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
        let offer_exporter = OfferExporter::new(&model);
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
        let offer_exporter = OfferExporter::new(&model);
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
