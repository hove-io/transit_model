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
    model::{Collections, Model},
    objects::{
        CommercialMode, Line, Network, ObjectType as ModelObjectType, PhysicalMode, VehicleJourney,
    },
    report::{Report, TransitModelReportCategory},
    Result,
};
use failure::format_err;
use log::info;
use relational_types::IdxSet;
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    convert::TryFrom,
    fs::File,
    path::Path,
};

#[derive(Debug, Deserialize)]
pub struct ObjectProperties {
    properties: Value,
    #[serde(default)]
    grouped_from: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ObjectRuleConfiguration {
    #[serde(rename = "networks")]
    pub networks_rules: Option<Vec<ObjectProperties>>,
    #[serde(rename = "commercial_modes")]
    pub commercial_modes_rules: Option<Vec<ObjectProperties>>,
    #[serde(rename = "physical_modes")]
    pub physical_modes_rules: Option<Vec<ObjectProperties>>,
}

impl TryFrom<&Path> for ObjectRuleConfiguration {
    type Error = failure::Error;
    fn try_from(path: &Path) -> Result<Self> {
        info!("Reading object rules");
        File::open(path)
            .map_err(|e| format_err!("{}", e))
            .and_then(|file| {
                serde_json::from_reader::<_, ObjectRuleConfiguration>(file)
                    .map_err(|e| format_err!("{}", e))
            })
    }
}

#[derive(Debug)]
pub struct ObjectRule {
    configuration: ObjectRuleConfiguration,
    lines_by_network: Option<HashMap<String, IdxSet<Line>>>,
    lines_by_commercial_mode: Option<HashMap<String, IdxSet<Line>>>,
    vjs_by_physical_mode: Option<HashMap<String, IdxSet<VehicleJourney>>>,
}

impl ObjectRule {
    pub(crate) fn new(path: &Path, model: &Model) -> Result<Self> {
        let configuration = ObjectRuleConfiguration::try_from(path)?;
        let lines_by_network = if configuration.networks_rules.is_some() {
            Some(
                model
                    .networks
                    .iter()
                    .filter_map(|(idx, obj)| {
                        let lines = model.get_corresponding_from_idx(idx);
                        if lines.is_empty() {
                            None
                        } else {
                            Some((obj.id.clone(), lines))
                        }
                    })
                    .collect(),
            )
        } else {
            None
        };
        let lines_by_commercial_mode = if configuration.commercial_modes_rules.is_some() {
            Some(
                model
                    .commercial_modes
                    .iter()
                    .filter_map(|(idx, obj)| {
                        let lines = model.get_corresponding_from_idx(idx);
                        if lines.is_empty() {
                            None
                        } else {
                            Some((obj.id.clone(), lines))
                        }
                    })
                    .collect(),
            )
        } else {
            None
        };
        let vjs_by_physical_mode = if configuration.physical_modes_rules.is_some() {
            Some(
                model
                    .physical_modes
                    .iter()
                    .filter_map(|(idx, obj)| {
                        let vjs = model.get_corresponding_from_idx(idx);
                        if vjs.is_empty() {
                            None
                        } else {
                            Some((obj.id.clone(), vjs))
                        }
                    })
                    .collect(),
            )
        } else {
            None
        };
        let object_rule = ObjectRule {
            configuration,
            lines_by_network,
            lines_by_commercial_mode,
            vjs_by_physical_mode,
        };
        Ok(object_rule)
    }
}

fn check_and_apply_physical_modes_rules(
    report: &mut Report<TransitModelReportCategory>,
    collections: &mut Collections,
    physical_modes_rules: &[ObjectProperties],
    vjs_by_physical_mode: &HashMap<String, IdxSet<VehicleJourney>>,
) -> Result<()> {
    info!("Checking physical modes rules.");
    let mut physical_modes_to_remove: HashSet<String> = HashSet::new();
    let mut new_physical_modes: Vec<PhysicalMode> = vec![];

    for pyr in physical_modes_rules.iter() {
        let physical_mode_id = pyr
            .properties
            .get("physical_mode_id")
            .ok_or_else(|| format_err!("Key \"physical_mode_id\" is required"))?
            .as_str()
            .ok_or_else(|| format_err!("Value for \"physical_mode_id\" must be filled in"))?;

        if !collections.physical_modes.contains_id(physical_mode_id) {
            new_physical_modes.push(serde_json::from_value(pyr.properties.clone())?)
        }

        let mut physical_mode_rule = false;
        for pm_grouped in &pyr.grouped_from {
            if !collections.physical_modes.contains_id(&pm_grouped) {
                report.add_error(
                    format!("The grouped physical mode \"{}\" don't exist", pm_grouped),
                    TransitModelReportCategory::ObjectNotFound,
                );
            } else if let Some(trips) = vjs_by_physical_mode.get(pm_grouped) {
                for trip_idx in trips {
                    collections
                        .vehicle_journeys
                        .index_mut(*trip_idx)
                        .physical_mode_id = physical_mode_id.to_string();
                }
                physical_mode_rule = true;
                physical_modes_to_remove.insert(pm_grouped.to_string());
            }
        }
        if !physical_mode_rule {
            report.add_error(
                format!(
                    "The rule on the \"{}\" physical mode was not applied",
                    physical_mode_id
                ),
                TransitModelReportCategory::ObjectNotFound,
            );
        }
    }
    collections
        .physical_modes
        .retain(|cm| !physical_modes_to_remove.contains(&cm.id));

    collections.physical_modes.extend(new_physical_modes);

    Ok(())
}

fn check_and_apply_commercial_modes_rules(
    report: &mut Report<TransitModelReportCategory>,
    collections: &mut Collections,
    commercial_modes_rules: &[ObjectProperties],
    lines_by_commercial_mode: &HashMap<String, IdxSet<Line>>,
) -> Result<()> {
    info!("Checking commercial modes rules.");
    let mut commercial_modes_to_remove: HashSet<String> = HashSet::new();
    let mut new_commercial_modes: Vec<CommercialMode> = vec![];

    for pyr in commercial_modes_rules.iter() {
        let commercial_mode_id = pyr
            .properties
            .get("commercial_mode_id")
            .ok_or_else(|| format_err!("Key \"commercial_mode_id\" is required"))?
            .as_str()
            .ok_or_else(|| format_err!("Value for \"commercial_mode_id\" must be filled in"))?;

        if !collections.commercial_modes.contains_id(commercial_mode_id) {
            new_commercial_modes.push(serde_json::from_value(pyr.properties.clone())?)
        }

        let mut commercial_mode_rule = false;
        for cm_grouped in &pyr.grouped_from {
            if !collections.commercial_modes.contains_id(&cm_grouped) {
                report.add_error(
                    format!("The grouped commercial mode \"{}\" don't exist", cm_grouped),
                    TransitModelReportCategory::ObjectNotFound,
                );
            } else if let Some(lines) = lines_by_commercial_mode.get(cm_grouped) {
                for line_idx in lines {
                    collections.lines.index_mut(*line_idx).commercial_mode_id =
                        commercial_mode_id.to_string();
                }
                commercial_mode_rule = true;
                commercial_modes_to_remove.insert(cm_grouped.to_string());
            }
        }
        if !commercial_mode_rule {
            report.add_error(
                format!(
                    "The rule on the \"{}\" commercial mode was not applied",
                    commercial_mode_id
                ),
                TransitModelReportCategory::ObjectNotFound,
            );
        }
    }
    collections
        .commercial_modes
        .retain(|cm| !commercial_modes_to_remove.contains(&cm.id));

    collections.commercial_modes.extend(new_commercial_modes);

    Ok(())
}

fn check_and_apply_networks_rules(
    report: &mut Report<TransitModelReportCategory>,
    collections: &mut Collections,
    networks_rules: &[ObjectProperties],
    lines_by_network: &HashMap<String, IdxSet<Line>>,
) -> Result<()> {
    info!("Checking networks rules.");
    let mut networks_to_remove: HashSet<String> = HashSet::new();
    let mut new_networks: Vec<Network> = vec![];

    for pyr in networks_rules.iter() {
        let network_id = pyr
            .properties
            .get("network_id")
            .ok_or_else(|| format_err!("Key \"network_id\" is required"))?
            .as_str()
            .ok_or_else(|| format_err!("Value for \"network_id\" must be filled in"))?;

        if !collections.networks.contains_id(network_id) {
            new_networks.push(serde_json::from_value(pyr.properties.clone())?)
        }
        let mut network_rule = false;
        for grouped in &pyr.grouped_from {
            if !collections.networks.contains_id(&grouped) {
                report.add_error(
                    format!("The grouped network \"{}\" don't exist", grouped),
                    TransitModelReportCategory::ObjectNotFound,
                );
            } else if let Some(lines) = lines_by_network.get(grouped) {
                for line_idx in lines {
                    collections.lines.index_mut(*line_idx).network_id = network_id.to_string();
                }
                collections
                    .ticket_use_perimeters
                    .values_mut()
                    .filter(|ticket| ticket.object_type == ModelObjectType::Network)
                    .filter(|ticket| &ticket.object_id == grouped)
                    .for_each(|mut ticket| ticket.object_id = network_id.to_string());
                network_rule = true;
                networks_to_remove.insert(grouped.to_string());
            }
        }
        if !network_rule {
            report.add_error(
                format!("The rule on the \"{}\" network was not applied", network_id),
                TransitModelReportCategory::ObjectNotFound,
            );
        }
    }
    collections
        .networks
        .retain(|cm| !networks_to_remove.contains(&cm.id));

    collections.networks.extend(new_networks);

    Ok(())
}

impl ObjectRule {
    pub(crate) fn apply_rules(
        &self,
        collections: &mut Collections,
        report: &mut Report<TransitModelReportCategory>,
    ) -> Result<()> {
        if let (Some(networks_rules), Some(lines_by_network)) =
            (&self.configuration.networks_rules, &self.lines_by_network)
        {
            check_and_apply_networks_rules(report, collections, networks_rules, lines_by_network)?;
        };
        if let (Some(commercial_modes_rules), Some(lines_by_commercial_mode)) = (
            &self.configuration.commercial_modes_rules,
            &self.lines_by_commercial_mode,
        ) {
            check_and_apply_commercial_modes_rules(
                report,
                collections,
                commercial_modes_rules,
                lines_by_commercial_mode,
            )?;
        };
        if let (Some(physical_modes_rules), Some(vjs_by_physical_mode)) = (
            &self.configuration.physical_modes_rules,
            &self.vjs_by_physical_mode,
        ) {
            check_and_apply_physical_modes_rules(
                report,
                collections,
                physical_modes_rules,
                vjs_by_physical_mode,
            )?;
        };
        Ok(())
    }
}
