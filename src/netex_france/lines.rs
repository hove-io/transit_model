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
    objects::Line,
    Model, Result,
};
use minidom::{Element, Node};

pub struct LineExporter<'a> {
    model: &'a Model,
}

// Publicly exposed methods
impl<'a> LineExporter<'a> {
    pub fn new(model: &'a Model) -> Self {
        LineExporter { model }
    }
    pub fn export(&self) -> Result<Vec<Element>> {
        self.model
            .lines
            .values()
            .map(|line| self.export_line(line))
            .collect()
    }
}

// Internal methods
impl<'a> LineExporter<'a> {
    fn export_line(&self, line: &'a Line) -> Result<Element> {
        let element_builder = Element::builder(ObjectType::Line.to_string())
            .attr("id", Exporter::generate_id(&line.id, ObjectType::Line))
            .attr("version", "any");
        let element_builder = element_builder.append(self.generate_name(line));
        let element_builder = if let Some(public_code) = self.generate_public_code(line) {
            element_builder.append(public_code)
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

    fn generate_public_code(&self, line: &'a Line) -> Option<Element> {
        line.code.as_ref().map(|code| {
            Element::builder("PublicCode")
                .append(Node::Text(code.to_owned()))
                .build()
        })
    }
}
