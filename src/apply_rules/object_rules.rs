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
    model::{Collections, CO2_EMISSIONS},
    objects::{
        CommercialMode, Line, Network, ObjectType as ModelObjectType, PhysicalMode, VehicleJourney,
    },
    utils::{Report, ReportType},
    Result,
};
use failure::format_err;
use log::info;
use relational_types::IdxSet;
use serde::Deserialize;
use serde_json::{error::Category, error::Error as SerdeJsonError, Value};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    path::Path,
    result::Result as StdResult,
};

#[derive(Debug, Deserialize)]
struct ObjectProperties {
    #[serde(flatten)]
    properties: HashMap<String, Value>,
    #[serde(default)]
    grouped_from: Vec<String>,
}

fn get_value_string_from_properties(
    property: &serde_json::value::Value,
    key: &str,
    default_value: &str,
) -> String {
    if let Some(value) = property.get(key) {
        String::deserialize(value)
            .map(|opt| opt)
            .unwrap_or_else(|_| default_value.to_string())
    } else {
        default_value.to_string()
    }
}

fn get_opt_value_string_from_properties(
    property: &serde_json::value::Value,
    key: &str,
    default_value: Option<String>,
) -> Option<String> {
    if let Some(value) = property.get(key) {
        Option::<String>::deserialize(value)
            .map(|opt| opt)
            .unwrap_or(default_value)
    } else {
        default_value
    }
}

fn get_opt_value_number_from_properties(
    property: &serde_json::value::Value,
    key: &str,
    default_value: Option<u32>,
) -> Option<u32> {
    if let Some(value) = property.get(key) {
        Option::<u32>::deserialize(value)
            .map(|opt| opt)
            .unwrap_or(default_value)
    } else {
        default_value
    }
}

fn check_and_apply_physical_modes_rules(
    report: &mut Report,
    mut collections: Collections,
    physical_modes_rules: Vec<ObjectProperties>,
    vjs_by_physical_mode: &HashMap<String, IdxSet<VehicleJourney>>,
) -> Result<Collections> {
    info!("Checking physical modes rules.");
    let mut physical_modes_to_remove: HashSet<String> = HashSet::new();
    let mut new_physical_modes: Vec<PhysicalMode> = vec![];

    for pyr in physical_modes_rules.into_iter() {
        let properties = pyr
            .properties
            .get("properties")
            .ok_or_else(|| format_err!("Object \"properties\" is required"))?;

        let physical_mode_id = properties
            .get("physical_mode_id")
            .ok_or_else(|| format_err!("Key \"physical_mode_id is required"))?
            .as_str()
            .unwrap();

        if let Some(mut physical_mode) = collections.physical_modes.get_mut(physical_mode_id) {
            physical_mode.name = get_value_string_from_properties(
                properties,
                "physical_mode_name",
                &physical_mode.name,
            );
        } else if !CO2_EMISSIONS.contains_key(physical_mode_id) {
            report.add_error(
                format!(
                    "The physical mode id \"{}\" not authorised",
                    physical_mode_id
                ),
                ReportType::UnAuthorisedValue,
            );
            continue;
        }
        let mut physical_mode_rule = pyr.grouped_from.is_empty();
        for pm_grouped in &pyr.grouped_from {
            if !collections.physical_modes.contains_id(&pm_grouped) {
                report.add_error(
                    format!("The grouped physical mode \"{}\" don't exist", pm_grouped),
                    ReportType::ObjectNotFound,
                );
            } else {
                if let Some(trips) = vjs_by_physical_mode.get(pm_grouped) {
                    for trip_idx in trips {
                        collections
                            .vehicle_journeys
                            .index_mut(*trip_idx)
                            .physical_mode_id = physical_mode_id.to_string();
                    }
                    physical_modes_to_remove.insert(pm_grouped.to_string());
                }
                physical_mode_rule = true;
            }
        }
        if !physical_mode_rule {
            report.add_error(
                format!(
                    "The rule on the \"{}\" physical mode was not applied",
                    physical_mode_id
                ),
                ReportType::ObjectNotFound,
            );
        } else if !collections.physical_modes.contains_id(physical_mode_id) {
            new_physical_modes.push(serde_json::from_value(properties.clone())?);
        }
    }
    collections
        .physical_modes
        .retain(|cm| !physical_modes_to_remove.contains(&cm.id));

    collections.physical_modes.extend(new_physical_modes);

    Ok(collections)
}

fn read_physical_modes_rules_file<P: AsRef<Path>>(
    report: &mut Report,
    physical_mode_rules_file: P,
) -> Option<Vec<ObjectProperties>> {
    info!("Reading physical modes rules");

    #[derive(Debug, Deserialize)]
    struct PhysicalModesRule {
        #[serde(rename = "physical_modes")]
        physical_modes_rules: Vec<ObjectProperties>,
    }

    match File::open(physical_mode_rules_file) {
        Ok(file) => {
            let rdr: StdResult<PhysicalModesRule, SerdeJsonError> = serde_json::from_reader(file);
            match rdr {
                Ok(val) => Some(val.physical_modes_rules),
                Err(e) => {
                    if !(e.classify() == Category::Data) {
                        report.add_error(format!("{}", e), ReportType::InvalidFile);
                    }
                    None
                }
            }
        }
        Err(_) => None,
    }
}

fn apply_rules_on_physical_mode<P: AsRef<Path>>(
    physical_modes_rules_file: P,
    vjs_by_physical_mode: &HashMap<String, IdxSet<VehicleJourney>>,
    collections: Collections,
    mut report: &mut Report,
) -> Result<Collections> {
    let physical_modes_rules =
        read_physical_modes_rules_file(&mut report, physical_modes_rules_file);
    match physical_modes_rules {
        Some(res) => check_and_apply_physical_modes_rules(
            &mut report,
            collections,
            res,
            vjs_by_physical_mode,
        ),
        None => {
            info!("no rule on physical mode provided");
            Ok(collections)
        }
    }
}

fn check_and_apply_commercial_modes_rules(
    report: &mut Report,
    mut collections: Collections,
    commercial_modes_rules: Vec<ObjectProperties>,
    lines_by_commercial_mode: &HashMap<String, IdxSet<Line>>,
) -> Result<Collections> {
    info!("Checking commercial modes rules.");
    let mut commercial_modes_to_remove: HashSet<String> = HashSet::new();
    let mut new_commercial_modes: Vec<CommercialMode> = vec![];

    for pyr in commercial_modes_rules.into_iter() {
        let properties = pyr
            .properties
            .get("properties")
            .ok_or_else(|| format_err!("Object \"properties\" is required"))?;

        let commercial_mode_id = properties
            .get("commercial_mode_id")
            .ok_or_else(|| format_err!("Key \"commercial_mode_id is required"))?
            .as_str()
            .unwrap();

        if let Some(mut commercial_mode) = collections.commercial_modes.get_mut(commercial_mode_id)
        {
            commercial_mode.name = get_value_string_from_properties(
                properties,
                "commercial_mode_name",
                &commercial_mode.name,
            );
        }
        let mut commercial_mode_rule = pyr.grouped_from.is_empty();
        for cm_grouped in &pyr.grouped_from {
            if !collections.commercial_modes.contains_id(&cm_grouped) {
                report.add_error(
                    format!("The grouped commercial mode \"{}\" don't exist", cm_grouped),
                    ReportType::ObjectNotFound,
                );
            } else {
                if let Some(lines) = lines_by_commercial_mode.get(cm_grouped) {
                    for line_idx in lines {
                        collections.lines.index_mut(*line_idx).commercial_mode_id =
                            commercial_mode_id.to_string();
                    }
                    commercial_modes_to_remove.insert(cm_grouped.to_string());
                }
                commercial_mode_rule = true;
            }
        }
        if !commercial_mode_rule {
            report.add_error(
                format!(
                    "The rule on the \"{}\" commercial mode was not applied",
                    commercial_mode_id
                ),
                ReportType::ObjectNotFound,
            );
        } else if !collections.commercial_modes.contains_id(commercial_mode_id) {
            new_commercial_modes.push(serde_json::from_value(properties.clone())?);
        }
    }
    collections
        .commercial_modes
        .retain(|cm| !commercial_modes_to_remove.contains(&cm.id));

    collections.commercial_modes.extend(new_commercial_modes);

    Ok(collections)
}

fn read_commercial_modes_rules_file<P: AsRef<Path>>(
    report: &mut Report,
    commercial_mode_rules_file: P,
) -> Option<Vec<ObjectProperties>> {
    info!("Reading commercial modes rules");

    #[derive(Debug, Deserialize)]
    struct CommercialModesRule {
        #[serde(rename = "commercial_modes")]
        commercial_modes_rules: Vec<ObjectProperties>,
    }

    match File::open(commercial_mode_rules_file) {
        Ok(file) => {
            let rdr: StdResult<CommercialModesRule, SerdeJsonError> = serde_json::from_reader(file);
            match rdr {
                Ok(val) => Some(val.commercial_modes_rules),
                Err(e) => {
                    if !(e.classify() == Category::Data) {
                        report.add_error(format!("{}", e), ReportType::InvalidFile);
                    }
                    None
                }
            }
        }
        Err(_) => None,
    }
}

fn apply_rules_on_commercial_mode<P: AsRef<Path>>(
    commercial_modes_rules_file: P,
    lines_by_commercial_mode: &HashMap<String, IdxSet<Line>>,
    collections: Collections,
    mut report: &mut Report,
) -> Result<Collections> {
    let commercial_modes_rules =
        read_commercial_modes_rules_file(&mut report, commercial_modes_rules_file);
    match commercial_modes_rules {
        Some(res) => check_and_apply_commercial_modes_rules(
            &mut report,
            collections,
            res,
            lines_by_commercial_mode,
        ),
        None => {
            info!("no rule on commercial mode provided");
            Ok(collections)
        }
    }
}

fn check_and_apply_networks_rules(
    report: &mut Report,
    mut collections: Collections,
    networks_rules: Vec<ObjectProperties>,
    lines_by_network: &HashMap<String, IdxSet<Line>>,
) -> Result<Collections> {
    info!("Checking networks rules.");
    let mut networks_to_remove: HashSet<String> = HashSet::new();
    let mut new_networks: Vec<Network> = vec![];

    for pyr in networks_rules.into_iter() {
        let properties = pyr
            .properties
            .get("properties")
            .ok_or_else(|| format_err!("Object \"properties\" is required"))?;

        let network_id = properties
            .get("network_id")
            .ok_or_else(|| format_err!("Key \"network_id is required"))?
            .as_str()
            .unwrap();

        if let Some(mut network) = collections.networks.get_mut(network_id) {
            network.name =
                get_value_string_from_properties(properties, "network_name", &network.name);
            network.url = get_opt_value_string_from_properties(
                properties,
                "network_url",
                network.url.clone(),
            );
            network.timezone = get_opt_value_string_from_properties(
                properties,
                "network_timezone",
                network.timezone.clone(),
            );
            network.lang = get_opt_value_string_from_properties(
                properties,
                "network_lang",
                network.lang.clone(),
            );
            network.phone = get_opt_value_string_from_properties(
                properties,
                "network_phone",
                network.phone.clone(),
            );
            network.address = get_opt_value_string_from_properties(
                properties,
                "network_address",
                network.address.clone(),
            );
            network.sort_order = get_opt_value_number_from_properties(
                properties,
                "network_sort_order",
                network.sort_order,
            );
        }
        let mut network_rule = pyr.grouped_from.is_empty();
        for grouped in &pyr.grouped_from {
            if !collections.networks.contains_id(&grouped) {
                report.add_error(
                    format!("The grouped network \"{}\" don't exist", grouped),
                    ReportType::ObjectNotFound,
                );
            } else {
                if let Some(lines) = lines_by_network.get(grouped) {
                    for line_idx in lines {
                        collections.lines.index_mut(*line_idx).network_id = network_id.to_string();
                    }

                    collections
                        .ticket_use_perimeters
                        .values_mut()
                        .filter(|ticket| ticket.object_type == ModelObjectType::Network)
                        .filter(|ticket| &ticket.object_id == grouped)
                        .for_each(|mut ticket| ticket.object_id = network_id.to_string());
                    networks_to_remove.insert(grouped.to_string());
                }
                network_rule = true;
            }
        }
        if !network_rule {
            report.add_error(
                format!("The rule on the \"{}\" network was not applied", network_id),
                ReportType::ObjectNotFound,
            );
        } else if !collections.networks.contains_id(network_id) {
            new_networks.push(serde_json::from_value(properties.clone())?);
        }
    }
    collections
        .networks
        .retain(|cm| !networks_to_remove.contains(&cm.id));

    collections.networks.extend(new_networks);

    Ok(collections)
}

fn read_networks_rules_file<P: AsRef<Path>>(
    report: &mut Report,
    network_rules_file: P,
) -> Option<Vec<ObjectProperties>> {
    info!("Reading networks rules");

    #[derive(Debug, Deserialize)]
    struct NetworksRule {
        #[serde(rename = "networks")]
        networks_rules: Vec<ObjectProperties>,
    }

    match File::open(network_rules_file) {
        Ok(file) => {
            let rdr: StdResult<NetworksRule, SerdeJsonError> = serde_json::from_reader(file);
            match rdr {
                Ok(val) => Some(val.networks_rules),
                Err(e) => {
                    if !(e.classify() == Category::Data) {
                        report.add_error(format!("{}", e), ReportType::InvalidFile);
                    }
                    None
                }
            }
        }
        Err(_) => None,
    }
}

fn apply_rules_on_networks<P: AsRef<Path>>(
    networks_rules_file: P,
    lines_by_network: &HashMap<String, IdxSet<Line>>,
    collections: Collections,
    mut report: &mut Report,
) -> Result<Collections> {
    let networks_rules = read_networks_rules_file(&mut report, networks_rules_file);
    match networks_rules {
        Some(res) => {
            check_and_apply_networks_rules(&mut report, collections, res, lines_by_network)
        }
        None => {
            info!("no rule on network provided");
            Ok(collections)
        }
    }
}

pub fn apply_rules<P: AsRef<Path>>(
    object_rules_file: P,
    lines_by_network: &HashMap<String, IdxSet<Line>>,
    lines_by_commercial_mode: &HashMap<String, IdxSet<Line>>,
    vjs_by_physical_mode: &HashMap<String, IdxSet<VehicleJourney>>,
    mut collections: Collections,
    mut report: &mut Report,
) -> Result<Collections> {
    collections = apply_rules_on_networks(
        &object_rules_file,
        lines_by_network,
        collections,
        &mut report,
    )
    .unwrap();
    collections = apply_rules_on_commercial_mode(
        &object_rules_file,
        lines_by_commercial_mode,
        collections,
        &mut report,
    )
    .unwrap();
    apply_rules_on_physical_mode(
        &object_rules_file,
        vjs_by_physical_mode,
        collections,
        &mut report,
    )
}
