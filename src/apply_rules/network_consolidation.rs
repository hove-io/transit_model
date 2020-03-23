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
    model::Collections,
    objects::{Line, Network, ObjectType as ModelObjectType},
    utils::{Report, ReportType},
    Result,
};
use failure::{bail, format_err};
use log::info;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    path::Path,
};
use transit_model_relations::IdxSet;
use typed_index_collection::CollectionWithId;

#[derive(Clone, Default, Debug, Deserialize)]
struct NetworkConsolidation {
    #[serde(flatten)]
    network: Network,
    #[serde(default)]
    grouped_from: Vec<String>,
}

fn read_networks_consolidation_file<P: AsRef<Path>>(
    networks_consolidation_file: P,
) -> Result<Vec<NetworkConsolidation>> {
    info!("Reading networks consolidation rules.");

    #[derive(Debug, Deserialize)]
    struct Consolidation {
        #[serde(rename = "networks")]
        networks_consolidation: Vec<NetworkConsolidation>,
    }

    let file = File::open(networks_consolidation_file)?;
    let consolidation: Consolidation = serde_json::from_reader(file)
        .map_err(|_| format_err!("unvalid networks configuration file"))?;
    Ok(consolidation.networks_consolidation)
}

fn check_networks_consolidation(
    report: &mut Report,
    networks: &CollectionWithId<Network>,
    networks_consolidation: Vec<NetworkConsolidation>,
) -> Result<Vec<NetworkConsolidation>> {
    info!("Checking networks consolidation.");
    let mut res: Vec<NetworkConsolidation> = vec![];

    for ntw in networks_consolidation.into_iter() {
        let mut network_consolidation = false;
        if networks.get(&ntw.network.id).is_some() {
            bail!(format!("The network \"{}\" already exists", ntw.network.id));
        };

        if ntw.grouped_from.is_empty() {
            report.add_error(
                format!(
                    "The grouped network list is empty for network consolidation \"{}\"",
                    &ntw.network.id
                ),
                ReportType::ObjectNotFound,
            );
            continue;
        }
        for ntw_grouped in &ntw.grouped_from {
            if !networks.contains_id(&ntw_grouped) {
                report.add_error(
                    format!("The grouped network \"{}\" don't exist", ntw_grouped),
                    ReportType::ObjectNotFound,
                );
            } else {
                network_consolidation = true;
            }
        }
        if network_consolidation {
            res.push(ntw);
        } else {
            report.add_error(
                format!(
                    "No network has been consolidated for network \"{}\"",
                    ntw.network.id
                ),
                ReportType::ObjectNotFound,
            );
        }
    }
    Ok(res)
}

fn set_networks_consolidation(
    mut collections: Collections,
    lines_by_network: &HashMap<String, IdxSet<Line>>,
    networks_consolidation: Vec<NetworkConsolidation>,
) -> Result<Collections> {
    let mut networks_to_remove: HashSet<String> = HashSet::new();
    for network in networks_consolidation {
        let network_id = network.network.id.clone();
        collections.networks.push(network.network)?;
        for grouped_from in network.grouped_from {
            if let Some(lines) = lines_by_network.get(&grouped_from) {
                for line_idx in lines {
                    let mut line = collections.lines.index_mut(*line_idx);
                    line.network_id = network_id.to_string();
                }
            }
            networks_to_remove.insert(grouped_from);
        }
    }
    collections
        .networks
        .retain(|ntw| !networks_to_remove.contains(&ntw.id));
    Ok(collections)
}

fn update_ticket_use_perimeters(
    collections: &mut Collections,
    networks_consolidation: &[NetworkConsolidation],
) {
    for network in networks_consolidation {
        let network_id = &network.network.id;
        for grouped_from in &network.grouped_from {
            collections
                .ticket_use_perimeters
                .values_mut()
                .filter(|ticket| {
                    ticket.object_type == ModelObjectType::Network
                        && &ticket.object_id == grouped_from
                })
                .for_each(|mut ticket| ticket.object_id = network_id.to_string());
        }
    }
}

pub fn apply_rules<P: AsRef<Path>>(
    networks_consolidation_file: P,
    lines_by_network: &HashMap<String, IdxSet<Line>>,
    mut collections: Collections,
    mut report: &mut Report,
) -> Result<Collections> {
    let networks_consolidation = read_networks_consolidation_file(networks_consolidation_file)?;
    let networks_consolidation =
        check_networks_consolidation(&mut report, &collections.networks, networks_consolidation)?;

    update_ticket_use_perimeters(&mut collections, &networks_consolidation);
    set_networks_consolidation(collections, &lines_by_network, networks_consolidation)
}
