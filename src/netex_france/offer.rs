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
    objects::{Line, Route},
    Model, Result,
};
use log::warn;
use minidom::{Element, Node};
use transit_model_collection::Idx;
use transit_model_relations::IdxSet;

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
        let elements = route_elements;
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
}
