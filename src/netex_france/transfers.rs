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
    objects::Transfer,
    Model, Result,
};
use failure::format_err;
use minidom::{Element, Node};

pub struct TransferExporter<'a> {
    model: &'a Model,
}

// Publicly exposed methods
impl<'a> TransferExporter<'a> {
    pub fn new(model: &'a Model) -> Self {
        TransferExporter { model }
    }
    pub fn export(&self) -> Result<Vec<Element>> {
        self.model
            .transfers
            .values()
            .map(|transfer| self.export_transfer(transfer))
            .collect()
    }
}

// Internal methods
impl<'a> TransferExporter<'a> {
    fn export_transfer(&self, transfer: &'a Transfer) -> Result<Element> {
        let element_builder = Element::builder(ObjectType::SiteConnection.to_string())
            .attr("id", self.generate_id(&transfer))
            .attr("version", "any");
        let element_builder = if let Some(walk_transfer_duration_element) =
            self.generate_walk_transfer_duration(transfer.real_min_transfer_time)
        {
            element_builder.append(walk_transfer_duration_element)
        } else {
            element_builder
        };
        let element_builder = element_builder.append(self.generate_from(&transfer.from_stop_id)?);
        let element_builder = element_builder.append(self.generate_to(&transfer.to_stop_id)?);
        Ok(element_builder.build())
    }

    fn generate_id(&self, transfer: &'a Transfer) -> String {
        Exporter::generate_id(
            &format!("{}_{}", transfer.from_stop_id, transfer.to_stop_id),
            ObjectType::SiteConnection,
        )
    }

    fn generate_walk_transfer_duration(
        &self,
        real_min_transfer_time: Option<u32>,
    ) -> Option<Element> {
        real_min_transfer_time
            .map(|time| format!("PT{}S", time))
            .map(|duration| {
                Element::builder("DefaultDuration")
                    .append(Node::Text(duration))
                    .build()
            })
            .map(|duration_element| {
                Element::builder("WalkTransferDuration")
                    .append(duration_element)
                    .build()
            })
    }

    fn generate_from(&self, from_stop_id: &'a str) -> Result<Element> {
        let element = Element::builder("From")
            .append(self.generate_stop_place_ref(from_stop_id)?)
            .append(self.generate_quay_ref(from_stop_id))
            .build();
        Ok(element)
    }

    fn generate_to(&self, to_stop_id: &'a str) -> Result<Element> {
        let element = Element::builder("To")
            .append(self.generate_stop_place_ref(to_stop_id)?)
            .append(self.generate_quay_ref(to_stop_id))
            .build();
        Ok(element)
    }

    fn generate_stop_place_ref(&self, stop_point_id: &'a str) -> Result<Element> {
        let stop_area_id = self
            .model
            .stop_points
            .get(stop_point_id)
            .map(|stop_point| &stop_point.stop_area_id)
            .ok_or_else(|| {
                format_err!(
                    "StopPoint '{}' doesn't have a corresponding StopArea",
                    stop_point_id
                )
            })?;
        let element = Element::builder("StopPlaceRef")
            .attr(
                "ref",
                Exporter::generate_id(stop_area_id, ObjectType::StopPlace),
            )
            .build();
        Ok(element)
    }

    fn generate_quay_ref(&self, stop_point_id: &'a str) -> Element {
        Element::builder("QuayRef")
            .attr(
                "ref",
                Exporter::generate_id(stop_point_id, ObjectType::Quay),
            )
            .build()
    }
}
