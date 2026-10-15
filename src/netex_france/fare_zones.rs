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
    netex_france::exporter::{Exporter, ObjectType},
    Model,
};
use std::collections::BTreeSet;

pub struct FareZoneExporter<'a> {
    model: &'a Model,
}

// Publicly exposed methods
impl<'a> FareZoneExporter<'a> {
    pub fn new(model: &'a Model) -> Self {
        FareZoneExporter { model }
    }
    pub fn export(&self) -> Vec<Element> {
        self.fare_zone_ids()
            .into_iter()
            .map(|fare_zone_id| self.export_fare_zone(fare_zone_id))
            .collect()
    }
}

// Internal methods
impl<'a> FareZoneExporter<'a> {
    // Returns every distinct `fare_zone_id` found on `stop_points`, sorted.
    // e.g. {"ZoneOffParis", "ZoneParis"}
    fn fare_zone_ids(&self) -> BTreeSet<&'a str> {
        self.model
            .stop_points
            .values()
            .filter_map(|stop_point| stop_point.fare_zone_id.as_deref())
            .collect()
    }
    fn export_fare_zone(&self, fare_zone_id: &'a str) -> Element {
        Element::builder(ObjectType::FareZone.to_string())
            .attr(
                "id",
                Exporter::generate_id(fare_zone_id, ObjectType::FareZone),
            )
            .attr("version", "any")
            .append(self.generate_name(fare_zone_id))
            .build()
    }
    fn generate_name(&self, fare_zone_id: &'a str) -> Element {
        Element::builder("Name")
            .append(Node::Text(fare_zone_id.to_owned()))
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ModelBuilder;

    mod export {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn one_fare_zone_per_distinct_fare_zone_id() {
            let model = ModelBuilder::default()
                .stop_area("sa:SP1", |_| {})
                .stop_point("SP1", |sp| sp.fare_zone_id = Some("ZoneParis".to_string()))
                .stop_area("sa:SP2", |_| {})
                .stop_point("SP2", |sp| {
                    sp.fare_zone_id = Some("ZoneOffParis".to_string())
                })
                .stop_area("sa:SP3", |_| {})
                .stop_point("SP3", |sp| sp.fare_zone_id = Some("ZoneParis".to_string()))
                .vj("vj1", |vj| {
                    vj.st("SP1", "10:00:00")
                        .st("SP2", "10:10:00")
                        .st("SP3", "10:20:00");
                })
                .build();
            let fare_zone_exporter = FareZoneExporter::new(&model);
            let fare_zone_elements = fare_zone_exporter.export();
            let fare_zone_ids: Vec<&str> = fare_zone_elements
                .iter()
                .map(|element| element.attr("id").unwrap())
                .collect();
            assert_eq!(
                fare_zone_ids,
                vec!["FR:FareZone:ZoneOffParis:", "FR:FareZone:ZoneParis:"]
            );
        }

        #[test]
        fn no_fare_zone_when_no_stop_point_has_a_fare_zone_id() {
            let model = ModelBuilder::default()
                .vj("vj1", |vj| {
                    vj.st("SP1", "10:00:00").st("SP2", "10:10:00");
                })
                .build();
            assert_eq!(model.vehicle_journeys.len(), 1);
            assert_eq!(model.stop_points.len(), 2);
            let fare_zone_exporter = FareZoneExporter::new(&model);
            assert!(fare_zone_exporter.export().is_empty());
        }
    }

    mod export_fare_zone {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn id_version_and_name() {
            let model = ModelBuilder::default()
                .stop_area("sa:SP1", |_| {})
                .stop_point("SP1", |sp| sp.fare_zone_id = Some("A".to_string()))
                .stop_area("sa:SP2", |_| {})
                .stop_point("SP2", |_| {})
                .vj("vj1", |vj| {
                    vj.st("SP1", "10:00:00").st("SP2", "10:10:00");
                })
                .build();
            assert_eq!(model.vehicle_journeys.len(), 1);
            assert_eq!(model.stop_points.len(), 2);
            let fare_zone_exporter = FareZoneExporter::new(&model);
            let fare_zone_element = fare_zone_exporter.export_fare_zone("A");
            assert_eq!(fare_zone_element.attr("id"), Some("FR:FareZone:A:"));
            assert_eq!(fare_zone_element.attr("version"), Some("any"));
            let name_element = fare_zone_element
                .nodes()
                .next()
                .unwrap()
                .as_element()
                .unwrap();
            assert_eq!(name_element.name(), "Name");
            let name_text = name_element.nodes().next().unwrap().as_text().unwrap();
            assert_eq!(name_text, "A");
        }
    }
}
