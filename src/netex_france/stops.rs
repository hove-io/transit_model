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

use crate::{objects::StopPoint, Model, Result};
use failure::format_err;
use minidom::{Element, Node};
use proj::Proj;

pub struct StopExporter<'a> {
    model: &'a Model,
    participant_ref: &'a str,
    stop_provider_code: &'a str,
    converter: Proj,
}

// Publicly exposed methods
impl<'a> StopExporter<'a> {
    pub fn new(
        model: &'a Model,
        participant_ref: &'a str,
        stop_provider_code: &'a str,
    ) -> Result<Self> {
        // FIXME: String 'EPSG:4326' is failing at runtime (string below is equivalent but works)
        let from = "+proj=longlat +datum=WGS84 +no_defs"; // See https://epsg.io/4326
        let to = "EPSG:2154";
        let converter = Proj::new_known_crs(from, to, None).ok_or_else(|| {
            format_err!("Proj cannot build a converter from '{}' to '{}'", from, to)
        })?;
        let exporter = StopExporter {
            model,
            participant_ref,
            stop_provider_code,
            converter,
        };
        Ok(exporter)
    }
    pub fn export(&self) -> Result<Vec<Element>> {
        self.model
            .stop_points
            .values()
            .map(|stop_point| self.export_stop_point(stop_point))
            .collect()
    }
}

// Internal methods
impl<'a> StopExporter<'a> {
    fn export_stop_point(&self, stop_point: &'a StopPoint) -> Result<Element> {
        let element_builder = Element::builder("Quay")
            .attr("id", self.generate_id(stop_point))
            .attr("version", "any");
        let element_builder = element_builder.append(self.generate_name(stop_point));
        let element_builder = if let Some(public_code) = self.generate_public_code(stop_point) {
            element_builder.append(public_code)
        } else {
            element_builder
        };
        let element_builder = element_builder.append(self.generate_site_ref(stop_point));
        let element_builder = element_builder.append(self.generate_centroid(stop_point)?);
        let element_builder = element_builder.append(self.generate_transport_mode(stop_point));
        let element_builder = if let Some(tariff_zones) = self.generate_tariff_zones(stop_point) {
            element_builder.append(tariff_zones)
        } else {
            element_builder
        };
        Ok(element_builder.build())
    }

    fn generate_id(&self, stop_point: &'a StopPoint) -> String {
        let id = stop_point.id.replace(':', "_");
        // TODO: Find INSEE code from geolocation of the `stop_point`
        let insee = "XXXXX";
        format!("FR:{}:ZE:{}:{}", insee, id, self.stop_provider_code)
    }

    fn generate_name(&self, stop_point: &'a StopPoint) -> Element {
        Element::builder("Name")
            .append(Node::Text(stop_point.name.to_owned()))
            .build()
    }

    fn generate_public_code(&self, stop_point: &'a StopPoint) -> Option<Element> {
        stop_point.code.as_ref().map(|code| {
            Element::builder("PublicCode")
                .append(Node::Text(code.to_owned()))
                .build()
        })
    }

    fn generate_centroid(&self, stop_point: &'a StopPoint) -> Result<Element> {
        let coord_epsg2154 = self.converter.convert(stop_point.coord)?;
        let coord_text = Node::Text(format!("{} {}", coord_epsg2154.x(), coord_epsg2154.y()));
        let pos = Element::builder("gml:pos")
            .attr("srsName", "EPSG:2154")
            .append(coord_text)
            .build();
        let location = Element::builder("Location").append(pos).build();
        let centroid = Element::builder("Centroid").append(location).build();
        Ok(centroid)
    }

    fn generate_site_ref(&self, _stop_point: &'a StopPoint) -> Element {
        // TODO: Figure out the identifier of the parent stop
        let parent_site_ref = "";
        Element::builder("ParentSiteRef")
            .attr("ref", parent_site_ref)
            .build()
    }

    fn generate_transport_mode(&self, _stop_point: &'a StopPoint) -> Element {
        // TODO: Find most frequent 'physical_mode' for 'stop_point' after converting to NeTEx modes
        let mode = String::new();
        let transport_mode_text = Node::Text(mode);
        Element::builder("TransportMode")
            .append(transport_mode_text)
            .build()
    }

    fn generate_tariff_zones(&self, stop_point: &'a StopPoint) -> Option<Element> {
        stop_point.fare_zone_id.as_ref().map(|fare_zone_id| {
            let tariff_zone_ref = Element::builder("TariffZoneRef")
                .attr("ref", format!("{}:{}", self.participant_ref, fare_zone_id))
                .build();
            Element::builder("tariffZones")
                .append(tariff_zone_ref)
                .build()
        })
    }
}
