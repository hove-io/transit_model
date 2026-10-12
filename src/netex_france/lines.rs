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
        offer::{Offer, OfferExporter},
        NetexMode,
    },
    objects::Line,
    Model, Result,
};
use anyhow::anyhow;
use std::cmp::Reverse;
use std::collections::{BTreeSet, HashMap};
use typed_index_collection::Idx;

// `line_modes` is storing all the Netex modes for a Line.
// A line can have multiple associated modes in NTM model (through trips).
pub type LineModes<'a> = HashMap<&'a str, BTreeSet<NetexMode>>;

// `line_companies` is storing all the company ids for a Line.
// A line can be operated by several companies (through trips).
type LineCompanies<'a> = HashMap<&'a str, Vec<&'a str>>;

pub struct LineExporter<'a> {
    model: &'a Model,
    line_modes: LineModes<'a>,
    line_companies: LineCompanies<'a>,
    offer_exporter: OfferExporter<'a>,
}

impl<'a> LineExporter<'a> {
    pub fn new(model: &'a Model) -> Result<Self> {
        let line_modes = Self::build_line_modes(model);
        let line_companies = Self::build_line_companies(model);
        let offer_exporter = OfferExporter::new(model)?;
        Ok(LineExporter {
            model,
            line_modes,
            line_companies,
            offer_exporter,
        })
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

    // Returns, for each Line, the ids of its companies ordered by decreasing
    // number of VehicleJourneys.
    // e.g. {"L1": ["RATP", "SNCF"], "L2": ["RATP"]}
    pub fn build_line_companies(model: &'a Model) -> LineCompanies<'a> {
        // e.g. {"L1": {"RATP": 120, "SNCF": 30}, "L2": {"RATP": 5}}
        let company_counts_by_line: HashMap<&'a str, HashMap<&'a str, usize>> = model
            .vehicle_journeys
            .values()
            .filter_map(|vehicle_journey| {
                model
                    .routes
                    .get(&vehicle_journey.route_id)
                    .map(|route| (&route.line_id, &vehicle_journey.company_id))
            })
            .fold(
                HashMap::new(),
                |mut line_companies, (line_id, company_id)| {
                    let vehicle_journey_count_by_company_id: &mut HashMap<&str, usize> =
                        line_companies.entry(line_id).or_default();
                    *vehicle_journey_count_by_company_id
                        .entry(company_id)
                        .or_insert(0) += 1;
                    line_companies
                },
            );
        company_counts_by_line
            .into_iter()
            .map(|(line_id, vehicle_journey_count_by_company_id)| {
                let mut company_ids: Vec<&str> = vehicle_journey_count_by_company_id
                    .keys()
                    .copied()
                    .collect();
                // Sort by decreasing VJ count (`Reverse`); if equal, by ascending `company_id`.
                company_ids.sort_by_key(|&company_id| {
                    (
                        Reverse(vehicle_journey_count_by_company_id[company_id]),
                        company_id,
                    )
                });
                (line_id, company_ids)
            })
            .collect()
    }

    /// Returns everything for this line, split per the target `line_*.xml`
    /// frames: `Line` + Route/RoutePoint/... in `structure`,
    /// ServiceJourney in `schedule`.
    pub fn export(&self, line_idx: Idx<Line>) -> Result<Offer> {
        let line = &self.model.lines[line_idx];
        let mut structure = vec![self.export_line(line)?];
        let offer = self
            .offer_exporter
            .export_offer(line_idx, &self.line_modes)?;
        structure.extend(offer.structure);
        Ok(Offer {
            structure,
            schedule: offer.schedule,
        })
    }

    fn export_line(&self, line: &'a Line) -> Result<Element> {
        let element_builder = Element::builder(ObjectType::Line.to_string())
            .attr("id", Exporter::generate_id(&line.id, ObjectType::Line))
            .attr("version", "any");
        let element_builder = element_builder.append_all(Exporter::generate_key_list(&line.codes)); // must be first child (XSD order)

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
        let element_builder = if let Some(operator_ref) = self.generate_operator_ref(line) {
            element_builder.append(operator_ref)
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

    fn generate_operator_ref(&self, line: &'a Line) -> Option<Element> {
        let main_company_id = self
            .line_companies
            .get(line.id.as_str())?
            .first()
            .copied()?;
        Some(
            Element::builder("OperatorRef")
                .attr(
                    "ref",
                    Exporter::generate_id(main_company_id, ObjectType::Operator),
                )
                .build(),
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ModelBuilder;

    mod build_line_companies {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn single_company() {
            let model = ModelBuilder::default()
                .vj("vj1", |vj| {
                    vj.line("L1")
                        .company("A")
                        .st("A", "10:00:00")
                        .st("B", "11:00:00");
                })
                .vj("vj2", |vj| {
                    vj.line("L1")
                        .company("A")
                        .st("A", "10:00:00")
                        .st("B", "11:00:00");
                })
                .build();
            let line_companies = LineExporter::build_line_companies(&model);
            assert_eq!(line_companies.get("L1"), Some(&vec!["A"]));
        }

        #[test]
        fn company_with_more_vehicle_journeys_is_first() {
            let model = ModelBuilder::default()
                .vj("vj1", |vj| {
                    vj.line("L1")
                        .company("A")
                        .st("A", "10:00:00")
                        .st("B", "11:00:00");
                })
                .vj("vj2", |vj| {
                    vj.line("L1")
                        .company("A")
                        .st("A", "10:00:00")
                        .st("B", "11:00:00");
                })
                .vj("vj3", |vj| {
                    vj.line("L1")
                        .company("B")
                        .st("A", "10:00:00")
                        .st("B", "11:00:00");
                })
                .build();
            let line_companies = LineExporter::build_line_companies(&model);
            assert_eq!(line_companies.get("L1"), Some(&vec!["A", "B"]));
        }

        #[test]
        fn smallest_company_id_is_first_when_counts_are_equal() {
            let model = ModelBuilder::default()
                .vj("vj1", |vj| {
                    vj.line("L1")
                        .company("B")
                        .st("A", "10:00:00")
                        .st("B", "11:00:00");
                })
                .vj("vj2", |vj| {
                    vj.line("L1")
                        .company("A")
                        .st("A", "10:00:00")
                        .st("B", "11:00:00");
                })
                .build();
            let line_companies = LineExporter::build_line_companies(&model);
            assert_eq!(line_companies.get("L1"), Some(&vec!["A", "B"]));
        }

        #[test]
        fn each_line_has_its_own_companies() {
            let model = ModelBuilder::default()
                .vj("vj1", |vj| {
                    vj.route("R1")
                        .line("L1")
                        .company("A")
                        .st("A", "10:00:00")
                        .st("B", "11:00:00");
                })
                .vj("vj2", |vj| {
                    vj.route("R2")
                        .line("L2")
                        .company("B")
                        .st("A", "10:00:00")
                        .st("B", "11:00:00");
                })
                .build();
            let line_companies = LineExporter::build_line_companies(&model);
            assert_eq!(line_companies.get("L1"), Some(&vec!["A"]));
            assert_eq!(line_companies.get("L2"), Some(&vec!["B"]));
        }
    }
}
