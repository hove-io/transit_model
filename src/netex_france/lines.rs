// Copyright (C) 2017 Hove and/or its affiliates.
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

use crate::xml_builder::{Element, Node};
use crate::{
    netex_france::{
        exporter::{Exporter, ObjectType},
        NetexMode,
    },
    objects::Line,
    Model, Result,
};
use anyhow::anyhow;
use std::collections::{BTreeSet, HashMap};

// `line_modes` is storing all the Netex modes for a Line.
// A line can have multiple associated modes in NTM model (through trips).
pub type LineModes<'a> = HashMap<&'a str, BTreeSet<NetexMode>>;

pub struct LineExporter<'a> {
    model: &'a Model,
    line_modes: LineModes<'a>,
}

// Publicly exposed methods
impl<'a> LineExporter<'a> {
    pub fn new(model: &'a Model) -> Self {
        let line_modes = Self::build_line_modes(model);
        LineExporter { model, line_modes }
    }
    pub fn export(&self) -> Result<Vec<Element>> {
        self.model
            .lines
            .values()
            .map(|line| self.export_line(line))
            .collect()
    }
    pub fn build_line_modes(model: &'a Model) -> LineModes<'a> {
        model
            .vehicle_journeys
            .values()
            .filter_map(|vehicle_journey| {
                NetexMode::from_physical_mode_id(&vehicle_journey.physical_mode_id)
                    .map(move |netex_mode| (vehicle_journey, netex_mode))
                    .and_then(|(vehicle_journey, netex_mode)| {
                        model
                            .routes
                            .get(&vehicle_journey.route_id)
                            .map(|route| &route.line_id)
                            .map(|line_id| (line_id, netex_mode))
                    })
            })
            .fold(HashMap::new(), |mut line_modes, (line_id, netex_mode)| {
                line_modes.entry(line_id).or_default().insert(netex_mode);
                line_modes
            })
    }
}

// Internal methods
impl<'a> LineExporter<'a> {
    fn export_line(&self, line: &'a Line) -> Result<Element> {
        let element_builder = Element::builder(ObjectType::Line.to_string())
            .attr("id", Exporter::generate_id(&line.id, ObjectType::Line))
            .attr("version", "any");
        // Errors should never happen; a line always have one trip with associated mode
        let netex_modes = self
            .line_modes
            .get(line.id.as_str())
            .ok_or_else(|| anyhow!("Unable to find modes for Line '{}'", line.id))?;
        let highest_netex_mode = NetexMode::calculate_highest_mode(netex_modes)
            .ok_or_else(|| anyhow!("Unable to resolve main NeTEx mode for Line {}", line.id))?;
        let element_builder = element_builder
            .append(self.generate_name(line))
            .append(self.generate_transport_mode(highest_netex_mode));
        let element_builder = if let Some(public_code) = self.generate_public_code(line) {
            element_builder.append(public_code)
        } else {
            element_builder
        };
        let element_builder = if let Some(presentation) = self.generate_presentation(line) {
            element_builder.append(presentation)
        } else {
            element_builder
        };
        Ok(element_builder.build())
    }

    fn generate_name(&self, line: &'a Line) -> Element {
        Element::builder("Name")
            .append(Node::Text(line.name.to_owned()))
            .build()
    }

    fn generate_transport_mode(&self, netex_mode: NetexMode) -> Element {
        let transport_mode_text = Node::Text(netex_mode.to_string());
        Element::builder("TransportMode")
            .append(transport_mode_text)
            .build()
    }

    fn generate_public_code(&self, line: &'a Line) -> Option<Element> {
        line.code.as_ref().map(|code| {
            Element::builder("PublicCode")
                .append(Node::Text(code.to_owned()))
                .build()
        })
    }

    fn generate_presentation(&self, line: &'a Line) -> Option<Element> {
        if line.color.is_none() && line.text_color.is_none() {
            return None;
        }
        let mut builder = Element::builder("Presentation");
        if let Some(rgb) = line.color.as_ref() {
            builder = builder.append(
                Element::builder("Colour")
                    .append(Node::Text(rgb.to_string()))
                    .build(),
            );
        }
        if let Some(rgb) = line.text_color.as_ref() {
            builder = builder.append(
                Element::builder("TextColour")
                    .append(Node::Text(rgb.to_string()))
                    .build(),
            );
        }
        Some(builder.build())
    }
}
