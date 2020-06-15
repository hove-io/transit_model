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

//! See function generates_transfers

use crate::{
    model::Model,
    objects::{StopPoint, Transfer},
    Result,
};
use log::info;
use std::collections::HashMap;
use typed_index_collection::{Collection, CollectionWithId, Idx};

type TransferMap = HashMap<(Idx<StopPoint>, Idx<StopPoint>), Transfer>;

fn make_transfers_map(
    transfers: Collection<Transfer>,
    sp: &CollectionWithId<StopPoint>,
) -> TransferMap {
    transfers
        .into_iter()
        .map(|t| {
            (
                (
                    sp.get_idx(&t.from_stop_id).unwrap(),
                    sp.get_idx(&t.to_stop_id).unwrap(),
                ),
                t,
            )
        })
        .collect()
}

fn generate_transfers_from_sp(
    transfers_map: &mut TransferMap,
    model: &Model,
    max_distance: f64,
    walking_speed: f64,
    waiting_time: u32,
) {
    info!("Adding missing transfers from stop points.");
    let sq_max_distance = max_distance * max_distance;
    for (idx1, sp1) in model.stop_points.iter() {
        let approx = sp1.coord.approx();
        for (idx2, sp2) in model.stop_points.iter() {
            if transfers_map.contains_key(&(idx1, idx2)) {
                continue;
            }
            let sq_distance = approx.sq_distance_to(&sp2.coord);
            if sq_distance > sq_max_distance {
                continue;
            }
            let transfer_time = (sq_distance.sqrt() / walking_speed) as u32;
            transfers_map.insert(
                (idx1, idx2),
                Transfer {
                    from_stop_id: sp1.id.clone(),
                    to_stop_id: sp2.id.clone(),
                    min_transfer_time: Some(transfer_time),
                    real_min_transfer_time: Some(transfer_time + waiting_time),
                    equipment_id: None,
                },
            );
        }
    }
}

/// Generates missing transfers
///
/// The `max_distance` argument allows you to specify the max distance
/// in meters to compute the tranfer.
///
/// The `walking_speed` argument is the walking speed in meters per second.
///
/// The `waiting_time` argument is the waiting transfer_time in seconds at stop.
///
/// `rule_files` are paths to csv files that contains rules for modifying
///
/// # Example
///
/// | from_stop_id | to_stop_id | transfer_time |                                                       |
/// | ------------ | ---------- | ------------- | ----------------------------------------------------- |
/// | SP1          | SP2        |               | no time is specified, this transfer will be removed   |
/// | SP3          | SP2        | 120           | transfer added                                        |
/// | UNKNOWN      | SP2        | 180           | stop `UNKNOWN` is not found, transfer will be ignored |
/// | UNKNOWN      | SP2        |               | stop `UNKNOWN` is not found, transfer will be ignored |
pub fn generates_transfers(
    model: Model,
    max_distance: f64,
    walking_speed: f64,
    waiting_time: u32,
) -> Result<Model> {
    info!("Generating transfers...");
    let mut transfers_map = make_transfers_map(model.transfers.clone(), &model.stop_points);
    generate_transfers_from_sp(
        &mut transfers_map,
        &model,
        max_distance,
        walking_speed,
        waiting_time,
    );

    let mut new_transfers: Vec<_> = transfers_map.into_iter().map(|(_, v)| v).collect();
    new_transfers.sort_unstable_by(|t1, t2| {
        (&t1.from_stop_id, &t1.to_stop_id).cmp(&(&t2.from_stop_id, &t2.to_stop_id))
    });

    let mut collections = model.into_collections();
    collections.transfers = Collection::new(new_transfers);
    Ok(Model::new(collections)?)
}

/// transfers rules
pub mod rules {
    use crate::{
        objects::{Contributor, StopPoint, Transfer},
        report::{Report, TransitModelReportCategory},
        Model, Result,
    };
    use failure::ResultExt;
    use log::info;
    use serde::Deserialize;
    use std::{
        collections::{hash_map::Entry::*, BTreeSet, HashMap, HashSet},
        fs,
        path::{Path, PathBuf},
    };
    use typed_index_collection::{Collection, CollectionWithId, Idx};

    // TODO: see if this can be removed
    type TransferMap = HashMap<(Idx<StopPoint>, Idx<StopPoint>), Transfer>;

    #[derive(Deserialize, Debug)]
    struct Rule {
        from_stop_id: String,
        to_stop_id: String,
        transfer_time: Option<u32>,
    }

    /// Represents the type of transfers to generate
    #[derive(PartialEq, Debug)]
    pub enum TransfersMode {
        /// `All` will generate all transfers
        All,
        /// `IntraContributor` will generate transfers between stop points belonging to the
        /// same contributor
        IntraContributor,
        /// `InterContributor` will generate transfers between stop points belonging to
        /// differents contributors only
        InterContributor,
    }

    /// Apply rules
    pub fn apply_transfer_rules<P: AsRef<Path>>(
        model: Model,
        waiting_time: u32,
        rule_files: Vec<P>,
        transfers_mode: &TransfersMode,
        report_path: Option<PathBuf>,
    ) -> Result<Model> {
        let mut transfers_map = make_transfers_map(model.transfers.clone(), &model.stop_points);
        let mut report = Report::default();
        let rules = read_rules(rule_files, &model, transfers_mode, &mut report)?;
        if !rules.is_empty() {
            remove_unwanted_transfers(&mut transfers_map, &model.stop_points, &rules);
            add_missing_transfers(&mut transfers_map, &model.stop_points, &rules, waiting_time);
        }
        if let Some(report_path) = report_path {
            let serialized_report = serde_json::to_string(&report)?;
            fs::write(report_path, serialized_report)?;
        }

        let mut new_transfers: Vec<_> = transfers_map.into_iter().map(|(_, v)| v).collect();
        new_transfers.sort_unstable_by(|t1, t2| {
            (&t1.from_stop_id, &t1.to_stop_id).cmp(&(&t2.from_stop_id, &t2.to_stop_id))
        });

        let mut collections = model.into_collections();
        collections.transfers = Collection::new(new_transfers);
        Ok(Model::new(collections)?)
    }

    fn filter_transfers() {}

    fn stop_points_need_transfer(
        model: &Model,
        from_idx: Idx<StopPoint>,
        to_idx: Idx<StopPoint>,
        transfers_mode: &TransfersMode,
        report_opt: Option<&mut Report<TransitModelReportCategory>>,
    ) -> bool {
        if *transfers_mode == TransfersMode::All {
            return true;
        }
        let from_contributor: BTreeSet<Idx<Contributor>> =
            model.get_corresponding_from_idx(from_idx);
        let to_contributor: BTreeSet<Idx<Contributor>> = model.get_corresponding_from_idx(to_idx);
        if from_contributor.is_empty() {
            if let Some(report) = report_opt {
                report.add_warning(
                    format!(
                        "stop point {} belongs to none of the trips and will not generate any transfer",
                        model.stop_points[from_idx].id
                    ),
                    TransitModelReportCategory::TransferOnUnreferencedStop,
                );
            }
            return false;
        }
        if to_contributor.is_empty() {
            if let Some(report) = report_opt {
                report.add_warning(
                    format!(
                        "stop point {} belongs to none of the trips and will not generate any transfer",
                        model.stop_points[to_idx].id
                    ),
                    TransitModelReportCategory::TransferOnUnreferencedStop,
                );
            }
            return false;
        }
        match *transfers_mode {
            TransfersMode::All => true,
            TransfersMode::IntraContributor => from_contributor == to_contributor,
            TransfersMode::InterContributor => from_contributor != to_contributor,
        }
    }

    // TODO: see if this can be removed
    fn make_transfers_map(
        transfers: Collection<Transfer>,
        sp: &CollectionWithId<StopPoint>,
    ) -> TransferMap {
        transfers
            .into_iter()
            .map(|t| {
                (
                    (
                        sp.get_idx(&t.from_stop_id).unwrap(),
                        sp.get_idx(&t.to_stop_id).unwrap(),
                    ),
                    t,
                )
            })
            .collect()
    }

    fn read_rules<P: AsRef<Path>>(
        rule_files: Vec<P>,
        model: &Model,
        transfers_mode: &TransfersMode,
        report: &mut Report<TransitModelReportCategory>,
    ) -> Result<Vec<Rule>> {
        info!("Reading modificaton rules.");
        let mut rules = HashMap::new();
        for rule_path in rule_files {
            let path = rule_path.as_ref();
            let mut rdr = csv::Reader::from_path(&path)
                .with_context(|_| format!("Error reading {:?}", path))?;

            for rule in rdr.deserialize() {
                let rule: Rule = rule.with_context(|_| format!("Error reading {:?}", path))?;
                match (
                    model.stop_points.get_idx(&rule.from_stop_id),
                    model.stop_points.get_idx(&rule.to_stop_id),
                ) {
                    (Some(from), Some(to)) => {
                        if stop_points_need_transfer(model, from, to, transfers_mode, Some(report))
                        {
                            match rules.entry((from, to)) {
                                Occupied(_) => report.add_warning(
                                    format!(
                                        "transfer between stops {} and {} is already declared",
                                        rule.from_stop_id, rule.to_stop_id
                                    ),
                                    TransitModelReportCategory::TransferAlreadyDeclared,
                                ),
                                Vacant(v) => {
                                    v.insert(rule);
                                }
                            }
                        } else {
                            let category = match *transfers_mode {
                                TransfersMode::IntraContributor => {
                                    TransitModelReportCategory::TransferInterIgnored
                                }
                                TransfersMode::InterContributor => {
                                    TransitModelReportCategory::TransferIntraIgnored
                                }
                                TransfersMode::All => {
                                    TransitModelReportCategory::TransferInterIgnored
                                } // not reachable
                            };
                            report.add_warning(
                                format!(
                                    "transfer between stops {} and {} is ignored",
                                    rule.from_stop_id, rule.to_stop_id
                                ),
                                category,
                            );
                        }
                    }
                    (Some(_), None) => {
                        report.add_warning(
                            format!(
                                "manual transfer references an non-existent stop point ({})",
                                rule.to_stop_id
                            ),
                            TransitModelReportCategory::TransferOnNonExistentStop,
                        );
                    }
                    (None, Some(_)) => {
                        report.add_warning(
                            format!(
                                "manual transfer references an non-existent stop point ({})",
                                rule.from_stop_id
                            ),
                            TransitModelReportCategory::TransferOnNonExistentStop,
                        );
                    }
                    _ => {
                        report.add_warning(
                            format!(
                                "manual transfer references non-existent stop points ({} and {})",
                                rule.from_stop_id, rule.to_stop_id
                            ),
                            TransitModelReportCategory::TransferOnNonExistentStop,
                        );
                    }
                }
            }
        }
        Ok(rules.into_iter().map(|(_, rule)| rule).collect())
    }

    fn remove_unwanted_transfers(
        transfers_map: &mut TransferMap,
        stop_points: &CollectionWithId<StopPoint>,
        rules: &[Rule],
    ) {
        info!("Removing unwanted transfers.");
        let rules_to_remove: HashSet<(Idx<StopPoint>, Idx<StopPoint>)> = rules
            .iter()
            .map(|r| {
                (
                    stop_points.get_idx(&r.from_stop_id).unwrap(),
                    stop_points.get_idx(&r.to_stop_id).unwrap(),
                )
            })
            .collect();
        transfers_map.retain(|_, t| {
            !rules_to_remove.contains(&(
                stop_points.get_idx(&t.from_stop_id).unwrap(),
                stop_points.get_idx(&t.to_stop_id).unwrap(),
            ))
        });
    }

    fn add_missing_transfers(
        transfers_map: &mut TransferMap,
        stop_points: &CollectionWithId<StopPoint>,
        rules: &[Rule],
        waiting_time: u32,
    ) {
        info!("Adding missing transfers.");
        for r in rules.iter().filter(|r| r.transfer_time.is_some()) {
            transfers_map
                .entry((
                    stop_points.get_idx(&r.from_stop_id).unwrap(),
                    stop_points.get_idx(&r.to_stop_id).unwrap(),
                ))
                .and_modify(|t| {
                    t.min_transfer_time = r.transfer_time;
                    t.real_min_transfer_time = r.transfer_time;
                })
                .or_insert_with(|| Transfer {
                    from_stop_id: r.from_stop_id.clone(),
                    to_stop_id: r.to_stop_id.clone(),
                    min_transfer_time: r.transfer_time,
                    real_min_transfer_time: r.transfer_time.map(|t| t + waiting_time),
                    equipment_id: None,
                });
        }
    }
}
