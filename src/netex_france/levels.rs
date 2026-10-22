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
    objects::Level,
    Model,
};

pub struct LevelExporter<'a> {
    model: &'a Model,
}

// Publicly exposed methods
impl<'a> LevelExporter<'a> {
    pub fn new(model: &'a Model) -> Self {
        LevelExporter { model }
    }
    pub fn export(&self) -> Vec<Element> {
        self.model
            .levels
            .values()
            .map(|level| self.export_level(level))
            .collect()
    }
}

// Internal methods
impl<'a> LevelExporter<'a> {
    fn export_level(&self, level: &'a Level) -> Element {
        Element::builder(ObjectType::Level.to_string())
            .attr("id", Exporter::generate_id(&level.id, ObjectType::Level))
            .attr("version", "any")
            .append_all(self.generate_name(level))
            .append(self.generate_relative_level_order(level))
            .build()
    }
    fn generate_name(&self, level: &'a Level) -> Option<Element> {
        let level_name = level.level_name.as_deref()?;
        Some(
            Element::builder("Name")
                .append(Node::Text(level_name.to_owned()))
                .build(),
        )
    }
    // NTFS 'level_index' is a 'f32' but NeTEx's 'RelativeLevelOrder' is an integer.
    fn generate_relative_level_order(&self, level: &'a Level) -> Element {
        Element::builder("RelativeLevelOrder")
            .append(Node::Text((level.level_index as i64).to_string()))
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Collections;

    fn empty_model() -> Model {
        Model::new(Collections::default()).unwrap()
    }

    mod export_level {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn id_version_and_relative_level_order() {
            let model = empty_model();
            let level_exporter = LevelExporter::new(&model);
            let level = Level {
                id: String::from("L1"),
                level_index: 0.0,
                level_name: None,
            };
            let level_element = level_exporter.export_level(&level);
            assert_eq!(level_element.attr("id"), Some("FR:Level:L1:"));
            assert_eq!(level_element.attr("version"), Some("any"));
            let relative_level_order = level_element.nodes().next().unwrap().as_element().unwrap();
            assert_eq!(relative_level_order.name(), "RelativeLevelOrder");
            let relative_level_order_text = relative_level_order
                .nodes()
                .next()
                .unwrap()
                .as_text()
                .unwrap();
            assert_eq!(relative_level_order_text, "0");
        }

        #[test]
        fn negative_relative_level_order() {
            let model = empty_model();
            let level_exporter = LevelExporter::new(&model);
            let level = Level {
                id: String::from("L-1"),
                level_index: -1.0,
                level_name: None,
            };
            let level_element = level_exporter.export_level(&level);
            let relative_level_order = level_element.nodes().next().unwrap().as_element().unwrap();
            let relative_level_order_text = relative_level_order
                .nodes()
                .next()
                .unwrap()
                .as_text()
                .unwrap();
            assert_eq!(relative_level_order_text, "-1");
        }

        #[test]
        fn name_present_when_level_name_is_set() {
            let model = empty_model();
            let level_exporter = LevelExporter::new(&model);
            let level = Level {
                id: String::from("L1"),
                level_index: 1.0,
                level_name: Some(String::from("Niveau1")),
            };
            let level_element = level_exporter.export_level(&level);
            let children: Vec<&str> = level_element
                .nodes()
                .filter_map(|node| node.as_element())
                .map(|element| element.name())
                .collect();
            assert_eq!(children, vec!["Name", "RelativeLevelOrder"]);
            let name_element = level_element.nodes().next().unwrap().as_element().unwrap();
            let name_text = name_element.nodes().next().unwrap().as_text().unwrap();
            assert_eq!(name_text, "Niveau1");
        }

        #[test]
        fn name_absent_when_level_name_is_none() {
            let model = empty_model();
            let level_exporter = LevelExporter::new(&model);
            let level = Level {
                id: String::from("L1"),
                level_index: 1.0,
                level_name: None,
            };
            let level_element = level_exporter.export_level(&level);
            let children: Vec<&str> = level_element
                .nodes()
                .filter_map(|node| node.as_element())
                .map(|element| element.name())
                .collect();
            assert_eq!(children, vec!["RelativeLevelOrder"]);
        }
    }
}
