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
        NetexMode,
    },
    objects::{Coord, StopArea, StopPoint},
    Model, Result,
};
use failure::format_err;
use log::warn;
use minidom::{Element, Node};
use proj::Proj;
use std::{
    borrow::Borrow,
    collections::{BTreeSet, HashMap},
};

// `stop_point_modes` is storing all the modes for a StopPoint.
//
// A Stop Point can have multiple associated modes in NTM model. We use a
// `BTreeSet` for determinism of the order (so fixtures are always written in
// the same order).
//
// Processing of `stop_point_modes` information is expansive which is the reason
// why we process it at construction of `StopExporter` and then store it.
type StopPointModes<'a> = HashMap<&'a str, BTreeSet<NetexMode>>;
type StopAreaStopPoints<'a> = HashMap<&'a str, BTreeSet<&'a str>>;
pub struct StopExporter<'a> {
    model: &'a Model,
    participant_ref: &'a str,
    converter: Proj,
    stop_point_modes: StopPointModes<'a>,
    stop_area_stop_points: StopAreaStopPoints<'a>,
}

// Publicly exposed methods
impl<'a> StopExporter<'a> {
    pub fn new(model: &'a Model, participant_ref: &'a str) -> Result<Self> {
        let converter = Exporter::get_coordinates_converter()?;
        let stop_point_modes = Self::build_stop_point_modes(model);
        let stop_area_stop_points = Self::build_stop_area_stop_points(model);
        let exporter = StopExporter {
            model,
            participant_ref,
            converter,
            stop_point_modes,
            stop_area_stop_points,
        };
        Ok(exporter)
    }
    pub fn export(&self) -> Result<Vec<Element>> {
        let stop_points_elements = self
            .model
            .stop_points
            .values()
            // Create Quay only for `stop_point` with a NeTEx mode
            .filter(|stop_point| self.stop_point_modes.contains_key(stop_point.id.as_str()))
            .map(|stop_point| self.export_stop_point(stop_point))
            .collect::<Result<Vec<Element>>>()?;
        let stop_areas_elements = self
            .model
            .stop_areas
            .values()
            // Create StopPlace for `stop_area` with at least one `stop_point` with a NeTEx mode
            .filter(|stop_area| {
                if let Some(stop_point_ids) = self.stop_area_stop_points.get(stop_area.id.as_str())
                {
                    let stop_points_with_netex_modes = stop_point_ids
                        .iter()
                        .filter(|stop_point_id| self.stop_point_modes.contains_key(*stop_point_id))
                        .count();
                    stop_points_with_netex_modes > 0
                } else {
                    false
                }
            })
            .map(|stop_area| self.export_stop_area(stop_area))
            .collect::<Result<Vec<Vec<Element>>>>()?;
        let mut elements = stop_points_elements;
        elements.extend(stop_areas_elements.into_iter().flatten());
        Ok(elements)
    }

    pub(in crate::netex_france) fn generate_stop_place_id(
        stop_area_id: &'a str,
        netex_mode: NetexMode,
    ) -> String {
        Exporter::generate_id(
            &format!("{}_{}", stop_area_id, netex_mode),
            ObjectType::StopPlace,
        )
    }
}

// Internal methods
impl<'a> StopExporter<'a> {
    // To find the mode associated to a Stop Area, here is the following
    // sequence of actions:
    // - we need to iterate over all Vehicle Journeys
    // - convert the Physical Mode into a NeTEx mode
    // - iterate over all Stop Times in these Vehicle Journeys (this one is
    //   expansive)
    // - find the corresponding Stop Point
    // - find the corresponding parent Stop Area
    fn build_stop_point_modes(model: &'a Model) -> StopPointModes<'a> {
        model
            .vehicle_journeys
            .values()
            .filter_map(|vehicle_journey| {
                NetexMode::from_physical_mode_id(&vehicle_journey.physical_mode_id)
                    .map(move |netex_mode| (vehicle_journey, netex_mode))
            })
            .flat_map(|(vehicle_journey, netex_mode)| {
                vehicle_journey
                    .stop_times
                    .iter()
                    .map(|stop_time| &stop_time.stop_point_idx)
                    .map(|stop_point_idx| &model.stop_points[*stop_point_idx])
                    .map(move |stop_point| (&stop_point.id, netex_mode))
            })
            .fold(
                HashMap::new(),
                |mut stop_point_modes, (stop_point_id, netex_mode)| {
                    stop_point_modes
                        .entry(stop_point_id)
                        .or_insert_with(BTreeSet::new)
                        .insert(netex_mode);
                    stop_point_modes
                },
            )
    }

    fn build_stop_area_stop_points(model: &'a Model) -> StopAreaStopPoints<'a> {
        model
            .stop_points
            .values()
            .fold(HashMap::new(), |mut stop_area_stop_points, stop_point| {
                stop_area_stop_points
                    .entry(&stop_point.stop_area_id)
                    .or_insert_with(BTreeSet::new)
                    .insert(&stop_point.id);
                stop_area_stop_points
            })
    }

    fn export_stop_point(&self, stop_point: &'a StopPoint) -> Result<Element> {
        let element_builder = Element::builder("Quay")
            .attr(
                "id",
                Exporter::generate_id(&stop_point.id, ObjectType::Quay),
            )
            .attr("version", "any");
        let element_builder = element_builder.append(self.generate_name(&stop_point.name));
        let element_builder = element_builder.append(self.generate_centroid(&stop_point.coord)?);
        let netex_modes = self
            .stop_point_modes
            .get(stop_point.id.as_str())
            .ok_or_else(|| {
                // Should never happen, a Stop Point always have some associated mode
                format_err!("Unable to find modes for Stop Point '{}'", stop_point.id)
            })?;
        if netex_modes.len() > 1 {
            warn!(
                "StopPoint '{}' has more than one associated NeTEx mode: {:?}",
                stop_point.id, netex_modes
            );
        }
        let highest_netex_mode = self.calculate_highest_mode(&netex_modes).ok_or_else(|| {
            // Should never happen, a Stop Point always have at least one associated mode
            format_err!(
                "Unable to resolve main NeTEx mode for Stop Point {}",
                stop_point.id
            )
        })?;
        let element_builder =
            element_builder.append(self.generate_transport_mode(highest_netex_mode));
        let element_builder = if let Some(tariff_zones) = self.generate_tariff_zones(stop_point) {
            element_builder.append(tariff_zones)
        } else {
            element_builder
        };
        let element_builder = if let Some(public_code) = self.generate_public_code(stop_point) {
            element_builder.append(public_code)
        } else {
            element_builder
        };
        Ok(element_builder.build())
    }

    fn export_stop_area(&self, stop_area: &'a StopArea) -> Result<Vec<Element>> {
        if let Some(stop_point_ids) = self.stop_area_stop_points.get(stop_area.id.as_str()) {
            let netex_modes: BTreeSet<NetexMode> = stop_point_ids
                .iter()
                .filter_map(|stop_point_id| self.model.stop_points.get(stop_point_id))
                .filter_map(|stop_point| self.stop_point_modes.get(stop_point.id.as_str()))
                .flatten()
                .copied()
                .collect();
            let mut stop_place_elements = Vec::new();
            let name_element = self.generate_name(&stop_area.name);
            let parent_station_id = Exporter::generate_id(&stop_area.id, ObjectType::StopPlace);
            let parent_site_ref_element = self.generate_parent_site_ref(&parent_station_id);
            let centroid = self.generate_centroid(&stop_area.coord)?;
            for netex_mode in &netex_modes {
                // Get only Stop Points with the current NeTEx mode
                let stop_point_ids = stop_point_ids
                    .iter()
                    .filter(|&stop_point_id| {
                        self.stop_point_modes
                            .get(stop_point_id)
                            .map(|netex_modes| netex_modes.contains(netex_mode))
                            .unwrap_or(false)
                    })
                    .collect::<BTreeSet<_>>();
                let element_builder = Element::builder("StopPlace")
                    .attr(
                        "id",
                        Self::generate_stop_place_id(&stop_area.id, *netex_mode),
                    )
                    .attr("version", "any");
                let element_builder = element_builder.append(name_element.clone());
                let element_builder = element_builder.append(centroid.clone());
                let element_builder = element_builder.append(parent_site_ref_element.clone());
                let element_builder =
                    element_builder.append(self.generate_transport_mode(*netex_mode));
                let element_builder =
                    element_builder.append(self.generate_stop_place_type(*netex_mode));
                let element_builder = element_builder.append(self.generate_quays(stop_point_ids));
                stop_place_elements.push(element_builder.build());
            }
            let element_builder = Element::builder("StopPlace")
                .attr(
                    "id",
                    Exporter::generate_id(&stop_area.id, ObjectType::StopPlace),
                )
                .attr("version", "any");
            let element_builder = element_builder.append(name_element);
            let element_builder = element_builder.append(centroid);
            let highest_netex_mode =
                self.calculate_highest_mode(&netex_modes).ok_or_else(|| {
                    // Should never happen, a Stop Area always have at least one associated mode
                    format_err!(
                        "Unable to resolve main NeTEx mode for Stop Area {}",
                        stop_area.id
                    )
                })?;
            let element_builder =
                element_builder.append(self.generate_transport_mode(highest_netex_mode));
            let element_builder =
                element_builder.append(self.generate_stop_place_type(highest_netex_mode));
            stop_place_elements.push(element_builder.build());
            Ok(stop_place_elements)
        } else {
            Ok(Vec::new())
        }
    }

    fn generate_name(&self, name: &'a str) -> Element {
        Element::builder("Name")
            .append(Node::Text(name.to_owned()))
            .build()
    }

    fn generate_public_code(&self, stop_point: &'a StopPoint) -> Option<Element> {
        stop_point.code.as_ref().map(|code| {
            Element::builder("PublicCode")
                .append(Node::Text(code.to_owned()))
                .build()
        })
    }

    fn generate_centroid(&self, coord: &'a Coord) -> Result<Element> {
        let coord_epsg2154 = self.converter.convert(*coord)?;
        let coord_text = Node::Text(format!("{} {}", coord_epsg2154.x(), coord_epsg2154.y()));
        let pos = Element::builder("gml:pos")
            .attr("srsName", "EPSG:2154")
            .append(coord_text)
            .build();
        let location = Element::builder("Location").append(pos).build();
        let centroid = Element::builder("Centroid").append(location).build();
        Ok(centroid)
    }

    fn generate_parent_site_ref(&self, parent_station_id: &'a str) -> Element {
        Element::builder("ParentSiteRef")
            .attr("ref", parent_station_id)
            .build()
    }

    fn generate_transport_mode(&self, netex_mode: NetexMode) -> Element {
        let transport_mode_text = Node::Text(netex_mode.to_string());
        Element::builder("TransportMode")
            .append(transport_mode_text)
            .build()
    }

    fn calculate_highest_mode(&self, netex_modes: &BTreeSet<NetexMode>) -> Option<NetexMode> {
        // Since `BTreeSet is ordered, the first one in the list is of highest priority
        netex_modes.iter().next().cloned()
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

    fn generate_quays<I, T>(&self, stop_point_ids: I) -> Element
    where
        I: IntoIterator<Item = T>,
        T: Borrow<&'a str>,
    {
        let quays = stop_point_ids
            .into_iter()
            .map(|stop_point_id| Exporter::generate_id(stop_point_id.borrow(), ObjectType::Quay))
            .map(|quay_id| Element::builder("QuayRef").attr("ref", quay_id).build());
        Element::builder("quays").append_all(quays).build()
    }

    fn generate_stop_place_type(&self, netex_mode: NetexMode) -> Element {
        use NetexMode::*;
        let stop_place_type = match netex_mode {
            Air => "Airport",
            Water => "ferryStop",
            Rail => "railStation",
            Metro => "metroStation",
            Tram => "tramStation",
            Funicular => "railStation",
            Cableway => "liftStation",
            Coach => "coachStation",
            Bus => "busStation",
        };
        Element::builder("StopPlaceType")
            .append(Node::Text(stop_place_type.to_owned()))
            .build()
    }
}
