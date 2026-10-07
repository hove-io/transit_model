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
    objects::{Availability, Coord, Equipment, StopArea, StopLocation, StopPoint, StopType},
    Model, Result,
};
use anyhow::anyhow;
use std::{
    borrow::Borrow,
    collections::{BTreeSet, HashMap},
};
use tracing::warn;

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
type StopAreaEntrances<'a> = HashMap<&'a str, BTreeSet<&'a str>>;
pub struct StopExporter<'a> {
    model: &'a Model,
    stop_point_modes: StopPointModes<'a>,
    stop_area_stop_points: StopAreaStopPoints<'a>,
    stop_area_entrances: StopAreaEntrances<'a>,
}

#[derive(Clone, Copy)]
enum Limitation {
    Wheelchair,
    Audible,
    Visual,
}

impl Limitation {
    fn name(self) -> &'static str {
        match self {
            Limitation::Wheelchair => "WheelchairAccess",
            Limitation::Audible => "AudibleSignalsAvailable",
            Limitation::Visual => "VisualSignsAvailable",
        }
    }

    fn availability(self, equipment: &Equipment) -> Availability {
        match self {
            Limitation::Wheelchair => equipment.wheelchair_boarding,
            Limitation::Audible => equipment.audible_announcement,
            Limitation::Visual => equipment.visual_announcement,
        }
    }
}

// Publicly exposed methods
impl<'a> StopExporter<'a> {
    pub fn new(model: &'a Model) -> Result<Self> {
        let stop_point_modes = Self::build_stop_point_modes(model);
        let stop_area_stop_points = Self::build_stop_area_stop_points(model);
        let stop_area_entrances = Self::build_stop_area_entrances(model);
        let exporter = StopExporter {
            model,
            stop_point_modes,
            stop_area_stop_points,
            stop_area_entrances,
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
        let entrances_elements = self
            .model
            .stop_locations
            .values()
            .filter(|sl| sl.stop_type == StopType::StopEntrance)
            .filter(|sl| {
                // Without this check, the entrance's SiteRef may point to a StopPlace not written
                sl.parent_id.as_ref().is_some_and(|stop_area_id| {
                    self.stop_area_stop_points
                        .get(stop_area_id.as_str())
                        .is_some_and(|stop_point_ids| {
                            stop_point_ids.iter().any(|stop_point_id| {
                                self.stop_point_modes.contains_key(*stop_point_id)
                            })
                        })
                })
            })
            .map(|sl| self.generate_stop_place_entrance(sl))
            .collect::<Result<Vec<Element>>>()?;
        let mut elements = stop_points_elements;
        elements.extend(stop_areas_elements.into_iter().flatten());
        elements.extend(entrances_elements);
        Ok(elements)
    }

    pub(in crate::netex_france) fn generate_stop_place_id(
        stop_area_id: &'a str,
        netex_mode: NetexMode,
    ) -> String {
        Exporter::generate_id(
            &format!("{stop_area_id}_{netex_mode}"),
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
                        .or_default()
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
                    .or_default()
                    .insert(&stop_point.id);
                stop_area_stop_points
            })
    }

    fn build_stop_area_entrances(model: &'a Model) -> StopAreaEntrances<'a> {
        model
            .stop_locations
            .values()
            .filter(|sl| sl.stop_type == StopType::StopEntrance)
            .fold(HashMap::new(), |mut stop_area_entrances, stop_location| {
                if let Some(stop_area_id) = stop_location.parent_id.as_ref() {
                    stop_area_entrances
                        .entry(stop_area_id)
                        .or_default()
                        .insert(&stop_location.id);
                };
                stop_area_entrances
            })
    }

    fn export_stop_point(&self, stop_point: &'a StopPoint) -> Result<Element> {
        let element_builder = Element::builder("Quay")
            .attr(
                "id",
                Exporter::generate_id(&stop_point.id, ObjectType::Quay),
            )
            .attr("version", "any");
        let element_builder =
            element_builder.append_all(Exporter::generate_key_list(&stop_point.codes)); // must be first child (XSD order)
        let element_builder = element_builder.append(self.generate_name(&stop_point.name));
        let element_builder =
            if let Some(centroid_element) = self.generate_centroid(&stop_point.coord) {
                element_builder.append(centroid_element)
            } else {
                element_builder
            };
        let element_builder = element_builder.append_all(self.generate_accessibility(
            &stop_point.id,
            stop_point.equipment_id.as_ref(),
            &[
                Limitation::Wheelchair,
                Limitation::Audible,
                Limitation::Visual,
            ],
        ));
        let netex_modes = self
            .stop_point_modes
            .get(stop_point.id.as_str())
            .ok_or_else(|| {
                // Should never happen, a Stop Point always have some associated mode
                anyhow!("Unable to find modes for Stop Point '{}'", stop_point.id)
            })?;
        if netex_modes.len() > 1 {
            warn!(
                "StopPoint '{}' has more than one associated NeTEx mode: {:?}",
                stop_point.id, netex_modes
            );
        }
        let highest_netex_mode =
            NetexMode::calculate_highest_mode(netex_modes).ok_or_else(|| {
                // Should never happen, a Stop Point always have at least one associated mode
                anyhow!(
                    "Unable to resolve main NeTEx mode for Stop Point {}",
                    stop_point.id,
                )
            })?;
        if !self.model.stop_areas.contains_id(&stop_point.stop_area_id) {
            return Err(anyhow!(
                "Stop Point '{}' references unknown Stop Area '{}'",
                stop_point.id,
                stop_point.stop_area_id
            ));
        }
        let stop_place_id =
            Self::generate_stop_place_id(&stop_point.stop_area_id, highest_netex_mode);
        let element_builder = element_builder.append(self.generate_site_ref(&stop_place_id));
        let element_builder =
            element_builder.append(self.generate_transport_mode(highest_netex_mode));
        let element_builder = if let Some(tariff_zones) = self.generate_tariff_zones(stop_point) {
            element_builder.append(tariff_zones)
        } else {
            element_builder
        };
        let element_builder =
            if let Some(public_code) = self.generate_public_code(stop_point.code.as_deref()) {
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
            let centroid = self.generate_centroid(&stop_area.coord);
            // *** Monomodal stopplaces generation ***
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

                let element_builder = if let Some(centroid_element) = centroid.as_ref() {
                    element_builder.append(centroid_element.clone())
                } else {
                    element_builder
                };
                let element_builder =
                    element_builder.append(self.generate_type_of_place_refs("monomodalStopPlace"));
                let element_builder = element_builder.append(parent_site_ref_element.clone());
                let element_builder =
                    element_builder.append(self.generate_transport_mode(*netex_mode));
                let element_builder =
                    element_builder.append(self.generate_stop_place_type(*netex_mode));
                let element_builder = element_builder.append(self.generate_quays(stop_point_ids));
                stop_place_elements.push(element_builder.build());
            }
            // *** Multimodal stopplaces generation ***
            let element_builder = Element::builder("StopPlace")
                .attr(
                    "id",
                    Exporter::generate_id(&stop_area.id, ObjectType::StopPlace),
                )
                .attr("version", "any");
            let element_builder =
                element_builder.append_all(Exporter::generate_key_list(&stop_area.codes)); // must be first child (XSD order)
            let element_builder = element_builder.append(name_element);
            let element_builder = if let Some(centroid_element) = centroid {
                element_builder.append(centroid_element)
            } else {
                element_builder
            };
            let element_builder =
                element_builder.append(self.generate_type_of_place_refs("multimodalStopPlace"));
            let element_builder = element_builder.append_all(self.generate_accessibility(
                &stop_area.id,
                stop_area.equipment_id.as_ref(),
                &[
                    Limitation::Wheelchair,
                    Limitation::Audible,
                    Limitation::Visual,
                ],
            ));
            let element_builder = if let Some(entrances) = self.generate_entrances(&stop_area.id) {
                element_builder.append(entrances)
            } else {
                element_builder
            };
            let highest_netex_mode =
                NetexMode::calculate_highest_mode(&netex_modes).ok_or_else(|| {
                    // Should never happen, a Stop Area always have at least one associated mode
                    anyhow!(
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

    fn generate_public_code(&self, code: Option<&str>) -> Option<Element> {
        code.map(|code| {
            Element::builder("PublicCode")
                .append(Node::Text(code.to_owned()))
                .build()
        })
    }

    fn generate_centroid(&self, coord: &'a Coord) -> Option<Element> {
        if *coord != Coord::default() {
            let longitude = Element::builder("Longitude")
                .append(Node::Text(coord.lon.to_string()))
                .build();
            let latitude = Element::builder("Latitude")
                .append(Node::Text(coord.lat.to_string()))
                .build();
            let location = Element::builder("Location")
                .append(longitude)
                .append(latitude)
                .build();
            let centroid = Element::builder("Centroid").append(location).build();
            return Some(centroid);
        }
        None
    }

    fn generate_accessibility(
        &self,
        owner_id: &str,
        equipment_id: Option<&String>,
        limitations: &[Limitation],
    ) -> Option<Element> {
        fn generate_limitation(name: &str, availability: Availability) -> Element {
            let value = match availability {
                Availability::Available => "true",
                Availability::NotAvailable => "false",
                _ => "unknown",
            };
            Element::builder(name)
                .append(Node::Text(value.to_owned()))
                .build()
        }
        equipment_id
            .and_then(|id| self.model.equipments.get(id))
            .map(|equipment| {
                let (availabilities, limitation_elements): (Vec<Availability>, Vec<Element>) =
                    limitations
                        .iter()
                        .map(|limitation| {
                            let availability = limitation.availability(equipment);
                            (
                                availability,
                                generate_limitation(limitation.name(), availability),
                            )
                        })
                        .unzip();
                Element::builder("AccessibilityAssessment")
                    .attr(
                        "id",
                        Exporter::generate_id(
                            &format!("{}_{}", owner_id, equipment.id),
                            ObjectType::AccessibilityAssessment,
                        ),
                    )
                    .attr("version", "any")
                    .append(self.mobility_impaired_access(&availabilities))
                    .append(
                        Element::builder("limitations")
                            .append(
                                Element::builder("AccessibilityLimitation")
                                    .append_all(limitation_elements)
                                    .build(),
                            )
                            .build(),
                    )
                    .build()
            })
    }
    fn mobility_impaired_access(&self, availabilities: &[Availability]) -> Element {
        let mut available = 0;
        let mut not_available = 0;
        for availability in availabilities {
            match availability {
                Availability::Available => available += 1,
                Availability::NotAvailable => not_available += 1,
                _ => {}
            }
        }
        let total = availabilities.len();
        let impaired_access = match (available, not_available) {
            (a, _) if a == total => "true",
            (_, n) if n == total => "false",
            (a, _) if a > 0 => "partial",
            _ => "unknown",
        };
        Element::builder("MobilityImpairedAccess")
            .append(Node::Text(impaired_access.to_owned()))
            .build()
    }

    fn generate_site_ref(&self, stop_place_id: &'a str) -> Element {
        Element::builder("SiteRef")
            .attr("ref", stop_place_id)
            .build()
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

    fn generate_entrances(&self, stop_area_id: &'a str) -> Option<Element> {
        let entrance_refs = self
            .stop_area_entrances
            .get(stop_area_id)
            .into_iter()
            .flatten()
            .filter_map(|sl_id| self.model.stop_locations.get(sl_id))
            .map(|sl| self.generate_entrance_ref(sl));
        let entrances = Element::builder("entrances")
            .append_all(entrance_refs)
            .build();
        if entrances.children().is_empty() {
            None
        } else {
            Some(entrances)
        }
    }

    fn generate_entrance_ref(&self, stop_location: &'a StopLocation) -> Element {
        Element::builder("EntranceRef")
            .attr(
                "ref",
                Exporter::generate_id(&stop_location.id, ObjectType::StopPlaceEntrance),
            )
            .build()
    }

    fn generate_stop_place_entrance(&self, stop_location: &'a StopLocation) -> Result<Element> {
        let stop_area_id = stop_location.parent_id.as_deref().ok_or_else(|| {
            // Should never happen: NTFS and GTFS readers require a parent StopArea
            anyhow!(
                "Stop Location '{}' has no parent Stop Area",
                stop_location.id
            )
        })?;
        let element_builder = Element::builder("StopPlaceEntrance")
            .attr(
                "id",
                Exporter::generate_id(&stop_location.id, ObjectType::StopPlaceEntrance),
            )
            .attr("version", "any")
            .append(self.generate_name(&stop_location.name));
        let element_builder =
            if let Some(centroid_element) = self.generate_centroid(&stop_location.coord) {
                element_builder.append(centroid_element)
            } else {
                element_builder
            };
        let element_builder = element_builder.append_all(self.generate_accessibility(
            &stop_location.id,
            stop_location.equipment_id.as_ref(),
            &[Limitation::Wheelchair],
        ));
        let stop_place_id = Exporter::generate_id(stop_area_id, ObjectType::StopPlace);
        let element_builder = element_builder.append(self.generate_site_ref(&stop_place_id));
        let element_builder =
            if let Some(public_code) = self.generate_public_code(stop_location.code.as_deref()) {
                element_builder.append(public_code)
            } else {
                element_builder
            };
        let element_builder = element_builder
            .append(self.generate_is_entry_exit("IsEntry"))
            .append(self.generate_is_entry_exit("IsExit"));

        Ok(element_builder.build())
    }

    fn generate_is_entry_exit(&self, node_name: &'a str) -> Element {
        Element::builder(node_name)
            .append(Node::Text("true".to_string()))
            .build()
    }

    fn generate_tariff_zones(&self, stop_point: &'a StopPoint) -> Option<Element> {
        stop_point.fare_zone_id.as_ref().map(|fare_zone_id| {
            // Profil 2.4 requires FareZoneRef for this element, but the XSD only
            // allows TariffZoneRef here (tariffZoneRefs_RelStructure); it can
            // reference a FareZone (specialization of TariffZone, carried by fare.xml).
            let tariff_zone_ref = Element::builder("TariffZoneRef")
                .attr(
                    "ref",
                    Exporter::generate_id(fare_zone_id, ObjectType::FareZone),
                )
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
            // Profil NeTEx-fr 2.4 maps funicular to metroStation
            Funicular => "metroStation",
            Cableway => "liftStation",
            Coach => "coachStation",
            Bus => "onstreetBus",
        };
        Element::builder("StopPlaceType")
            .append(Node::Text(stop_place_type.to_owned()))
            .build()
    }

    fn generate_type_of_place_refs(&self, type_of_place_ref: &str) -> Element {
        let type_of_place_ref_element = Element::builder("TypeOfPlaceRef")
            .attr("ref", type_of_place_ref)
            .build();
        Element::builder("placeTypes")
            .append(type_of_place_ref_element)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod valid_impaired_access {
        use super::*;
        use crate::model::{Collections, Model};
        use pretty_assertions::assert_eq;
        use Availability::*;

        fn get_mobility_impaired_access(element: Element) -> String {
            element
                .nodes()
                .next()
                .unwrap()
                .as_text()
                .unwrap()
                .to_string()
        }

        fn get_mobility_impaired_access_value(
            stop_exporter: &StopExporter,
            (w, v, a): (Availability, Availability, Availability),
        ) -> String {
            get_mobility_impaired_access(stop_exporter.mobility_impaired_access(&[w, v, a]))
        }

        #[test]
        fn test_impaired_access_true() {
            let model = Model::new(Collections::default()).unwrap();
            let stop_exporter = StopExporter::new(&model).unwrap();
            assert_eq!(
                "true",
                get_mobility_impaired_access_value(
                    &stop_exporter,
                    (Available, Available, Available)
                )
            );
        }

        #[test]
        fn test_impaired_access_false() {
            let model = Model::new(Collections::default()).unwrap();
            let stop_exporter = StopExporter::new(&model).unwrap();
            assert_eq!(
                "false",
                get_mobility_impaired_access_value(
                    &stop_exporter,
                    (NotAvailable, NotAvailable, NotAvailable)
                )
            );
        }

        #[test]
        fn test_impaired_access_partial() {
            let model = Model::new(Collections::default()).unwrap();
            let stop_exporter = StopExporter::new(&model).unwrap();
            assert_eq!(
                "partial",
                get_mobility_impaired_access_value(
                    &stop_exporter,
                    (InformationNotAvailable, InformationNotAvailable, Available)
                )
            );
        }

        #[test]
        fn test_impaired_access_unknown() {
            let model = Model::new(Collections::default()).unwrap();
            let stop_exporter = StopExporter::new(&model).unwrap();
            assert_eq!(
                "unknown",
                get_mobility_impaired_access_value(
                    &stop_exporter,
                    (NotAvailable, NotAvailable, InformationNotAvailable)
                )
            );
        }
    }
}
