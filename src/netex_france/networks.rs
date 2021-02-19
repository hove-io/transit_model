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
        NETEX_NS,
    },
    objects::{Line, Network},
    Model,
};
use minidom::{Element, Node};

pub struct NetworkExporter<'a> {
    model: &'a Model,
}

// Publicly exposed methods
impl<'a> NetworkExporter<'a> {
    pub fn new(model: &'a Model) -> Self {
        NetworkExporter { model }
    }
    pub fn export(&self) -> Vec<Element> {
        self.model
            .networks
            .values()
            .map(|network| self.export_network(network))
            .collect()
    }
}

// Internal methods
impl<'a> NetworkExporter<'a> {
    fn export_network(&self, network: &'a Network) -> Element {
        let element_builder = Element::builder(ObjectType::Network.to_string(), NETEX_NS)
            .attr(
                "id",
                Exporter::generate_id(&network.id, ObjectType::Network),
            )
            .attr("version", "any");
        let element_builder = element_builder.append(self.generate_name(network));
        let line_ref_elements = self
            .model
            .lines
            .values()
            .filter(|line| line.network_id == network.id)
            .map(|line| self.generate_line_ref(line));
        let element_builder = element_builder.append(Exporter::create_members(line_ref_elements));
        element_builder.build()
    }

    fn generate_name(&self, network: &'a Network) -> Element {
        Element::builder("Name", NETEX_NS)
            .append(Node::Text(network.name.to_owned()))
            .build()
    }

    fn generate_line_ref(&self, line: &'a Line) -> Element {
        let line_id = Exporter::generate_id(&line.id, ObjectType::Line);
        Element::builder("LineRef", NETEX_NS)
            .attr("ref", line_id)
            .build()
    }
}
