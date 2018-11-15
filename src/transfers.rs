// Copyright 2017-2018 Kisio Digital and/or its affiliates.
//
// This program is free software: you can redistribute it and/or
// modify it under the terms of the GNU General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see
// <http://www.gnu.org/licenses/>.

//! See function generates_transfers

use collection::{Collection, CollectionWithId, Idx};
use csv;
use failure::ResultExt;
use model::Model;
use objects::{Contributor, StopPoint, Transfer};
use std::collections::hash_map::Entry::*;
use std::collections::BTreeSet;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use utils::{Report, ReportType};
use Result;

#[derive(Deserialize, Debug)]
struct Rule {
    from_stop_id: String,
    to_stop_id: String,
    transfer_time: Option<u32>,
}
/// Represents the type of transfers to generate
#[derive(PartialEq, Debug)]
pub enum TransfersMode {
    /// `IntraContributor` will generate transfers between stop points belonging to the
    /// same contributor
    IntraContributor,
    /// `InterContributor` will generate transfers between stop points belonging to
    /// differents contributors only
    InterContributor,
}

type TransferMap = HashMap<(Idx<StopPoint>, Idx<StopPoint>), Transfer>;

fn stop_points_on_same_contributor(
    model: &Model,
    from_idx: Idx<StopPoint>,
    to_idx: Idx<StopPoint>,
) -> bool {
    let from_contributor: BTreeSet<Idx<Contributor>> = model.get_corresponding_from_idx(from_idx);
    let to_contributor: BTreeSet<Idx<Contributor>> = model.get_corresponding_from_idx(to_idx);
    from_contributor == to_contributor
}

fn read_rules<P: AsRef<Path>>(
    rule_files: Vec<P>,
    model: &Model,
    transfers_mode: &TransfersMode,
    report: &mut Report,
) -> Result<Vec<Rule>> {
    info!("Reading modificaton rules.");
    let mut rules = HashMap::new();
    for rule_path in rule_files {
        let path = rule_path.as_ref();
        let mut rdr = csv::Reader::from_path(&path).with_context(ctx_from_path!(path))?;

        for rule in rdr.deserialize() {
            let rule: Rule = rule.with_context(ctx_from_path!(path))?;
            match (
                model.stop_points.get_idx(&rule.from_stop_id),
                model.stop_points.get_idx(&rule.to_stop_id),
            ) {
                (Some(from), Some(to)) => {
                    let on_same_contributor = stop_points_on_same_contributor(&model, from, to);
                    match (on_same_contributor, transfers_mode) {
                        (true, &TransfersMode::IntraContributor)
                        | (false, &TransfersMode::InterContributor) => {
                            match rules.entry((from, to)) {
                                Occupied(_) => report.add_warning(
                                    format!(
                                        "transfer between stops {} and {} is already declared",
                                        rule.from_stop_id, rule.to_stop_id
                                    ),
                                    ReportType::TransferAlreadyDeclared,
                                ),
                                Vacant(v) => {
                                    v.insert(rule);
                                }
                            }
                        }
                        _ => {
                            let category = match transfers_mode {
                                &TransfersMode::IntraContributor => {
                                    ReportType::TransferIntraIgnored
                                }
                                &TransfersMode::InterContributor => {
                                    ReportType::TransferInterIgnored
                                }
                            };
                            report.add_warning(
                                format!(
                                    "transfer between stops {} and {} is ignored ({:?})",
                                    rule.from_stop_id, rule.to_stop_id, transfers_mode
                                ),
                                category,
                            );
                        }
                    }
                }
                (Some(_), None) => {
                    report.add_warning(
                        format!(
                            "manual transfer references an unexisting stop point ({})",
                            rule.from_stop_id
                        ),
                        ReportType::TransferOnUnexistingStop,
                    );
                }
                (None, Some(_)) => {
                    report.add_warning(
                        format!(
                            "manual transfer references an unexisting stop point ({})",
                            rule.to_stop_id
                        ),
                        ReportType::TransferOnUnexistingStop,
                    );
                }
                _ => {
                    report.add_warning(
                        format!(
                            "manual transfer references unexisting stop points ({} and {})",
                            rule.from_stop_id, rule.to_stop_id
                        ),
                        ReportType::TransferOnUnexistingStop,
                    );
                }
            }
        }
    }
    Ok(rules.into_iter().map(|(_, rule)| rule).collect())
}

fn make_transfers_map(transfers: Vec<Transfer>, sp: &CollectionWithId<StopPoint>) -> TransferMap {
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
        }).collect()
}

fn generate_transfers_from_sp(
    transfers_map: &mut TransferMap,
    model: &Model,
    max_distance: f64,
    walking_speed: f64,
    waiting_time: u32,
    transfers_mode: &TransfersMode,
) {
    info!("Adding missing transfers from stop points.");
    let sq_max_distance = max_distance * max_distance;
    for (idx1, sp1) in model.stop_points.iter() {
        let approx = sp1.coord.approx();
        for (idx2, sp2) in model.stop_points.iter() {
            if transfers_map.contains_key(&(idx1, idx2)) {
                continue;
            }
            let on_same_contributor = stop_points_on_same_contributor(&model, idx1, idx2);
            if on_same_contributor && transfers_mode == &TransfersMode::IntraContributor
                || !on_same_contributor && transfers_mode == &TransfersMode::InterContributor
            {
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
        }).collect();
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
            )).and_modify(|t| {
                t.min_transfer_time = r.transfer_time;
                t.real_min_transfer_time = r.transfer_time;
            }).or_insert_with(|| Transfer {
                from_stop_id: r.from_stop_id.clone(),
                to_stop_id: r.to_stop_id.clone(),
                min_transfer_time: r.transfer_time,
                real_min_transfer_time: r.transfer_time.map(|t| t + waiting_time),
                equipment_id: None,
            });
    }
}

fn do_generates_transfers(
    model: &mut Model,
    max_distance: f64,
    walking_speed: f64,
    waiting_time: u32,
    rules: &[Rule],
    transfers_mode: &TransfersMode,
) -> Result<Vec<Transfer>> {
    let mut transfers_map = make_transfers_map(model.transfers.take(), &model.stop_points);
    generate_transfers_from_sp(
        &mut transfers_map,
        &model,
        max_distance,
        walking_speed,
        waiting_time,
        transfers_mode,
    );

    if !rules.is_empty() {
        remove_unwanted_transfers(&mut transfers_map, &model.stop_points, rules);
        add_missing_transfers(&mut transfers_map, &model.stop_points, rules, waiting_time);
    }

    let mut transfers: Vec<_> = transfers_map.into_iter().map(|(_, v)| v).collect();
    transfers.sort_unstable_by(|t1, t2| {
        (&t1.from_stop_id, &t1.to_stop_id).cmp(&(&t2.from_stop_id, &t2.to_stop_id))
    });
    Ok(transfers)
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
/// `transfers_mode` is the type of transfers to generate
/// tranfers
///
/// # Example
///
/// from_stop_id|to_stop_id|transfer_time| |
/// --|--|--|--
/// SP1|SP2| | no time is specified, this transfer will be removed
/// SP3|SP2|120 | transfer added
/// UNKNOWN|SP2|180 | stop `UNKNOWN` is not found, transfer will be ignored
/// UNKNOWN|SP2| | stop `UNKNOWN` is not found, transfer will be ignored
pub fn generates_transfers<P: AsRef<Path>>(
    model: &mut Model,
    max_distance: f64,
    walking_speed: f64,
    waiting_time: u32,
    rule_files: Vec<P>,
    transfers_mode: TransfersMode,
    report_path: Option<PathBuf>,
) -> Result<()> {
    info!("Generating transfers...");
    let mut report = Report::new();
    let rules = read_rules(rule_files, model, &transfers_mode, &mut report)?;
    let new_transfers = do_generates_transfers(
        model,
        max_distance,
        walking_speed,
        waiting_time,
        &rules,
        &transfers_mode,
    )?;

    model.transfers = Collection::new(new_transfers);
    if let Some(report_path) = report_path {
        let serialized_report = serde_json::to_string(&report)?;
        fs::write(report_path, serialized_report)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Rule, TransfersMode};
    use collection::{Collection, CollectionWithId};
    use model::{Collections, Model};
    use objects::*;

    #[test]
    //                    206m
    // sp_1 *--------------------------------* sp_3
    //       \                        ______/
    //        \                  ____/
    //   65m   \           _____/   146m
    //          \    _____/
    //           \__/
    //           sp_2
    //
    fn test_generates_transfers() {
        let transfers = Collection::new(vec![
            Transfer {
                from_stop_id: "sp_1".to_string(),
                to_stop_id: "sp_2".to_string(),
                min_transfer_time: Some(50),
                real_min_transfer_time: Some(60),
                equipment_id: None,
            },
            Transfer {
                from_stop_id: "sp_1".to_string(),
                to_stop_id: "sp_3".to_string(),
                min_transfer_time: Some(200),
                real_min_transfer_time: Some(210),
                equipment_id: None,
            },
        ]);

        let stop_areas = CollectionWithId::new(vec![
            StopArea {
                id: "sa_1".to_string(),
                name: "sa_name_1".to_string(),
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                visible: true,
                coord: Coord {
                    lon: 2.372075915336609,
                    lat: 48.84608210211328,
                },
                timezone: None,
                geometry_id: None,
                equipment_id: None,
            }]).unwrap();

        let stop_points = CollectionWithId::new(vec![
            StopPoint {
                id: "sp_1".to_string(),
                name: "sp_name_1".to_string(),
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                visible: true,
                coord: Coord {
                    lon: 2.372075915336609,
                    lat: 48.84608210211328,
                },
                timezone: None,
                geometry_id: None,
                equipment_id: None,
                stop_area_id: "sa_1".to_string(),
                fare_zone_id: None,
                stop_type: StopType::Point,
            },
            StopPoint {
                id: "sp_2".to_string(),
                name: "sa_name_2".to_string(),
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                visible: true,
                coord: Coord {
                    lon: 2.371437549591065,
                    lat: 48.845665532277096,
                },
                timezone: None,
                geometry_id: None,
                equipment_id: None,
                stop_area_id: "sa_1".to_string(),
                fare_zone_id: None,
                stop_type: StopType::Point,
            },
            StopPoint {
                id: "sp_3".to_string(),
                name: "sa_name_3".to_string(),
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                visible: true,
                coord: Coord {
                    lon: 2.369517087936402,
                    lat: 48.845301913401144,
                },
                timezone: None,
                geometry_id: None,
                equipment_id: None,
                stop_area_id: "sa_1".to_string(),
                fare_zone_id: None,
                stop_type: StopType::Point,
            },
        ]).unwrap();
        let mut collections = Collections::default();
        collections.transfers = transfers;
        collections.stop_points = stop_points;
        collections.stop_areas = stop_areas;
        let mut model = Model::new(collections).unwrap();

        let transfers = super::do_generates_transfers(
            &mut model,
            100.0,
            0.785,
            120,
            &vec![],
            &TransfersMode::IntraContributor,
        ).unwrap();

        //we keep the 2 first existing transfers
        // transfers sp_2 -> sp_3, sp_3 -> sp_2, sp_3 -> sp_1 are not added,
        // because distances between them are > 100m
        // sp_1 -> sp_3 is kept because it is an existing transfer.
        assert_eq!(
            transfers,
            vec![
                Transfer {
                    from_stop_id: "sp_1".to_string(),
                    to_stop_id: "sp_1".to_string(),
                    min_transfer_time: Some(0),
                    real_min_transfer_time: Some(120),
                    equipment_id: None,
                },
                Transfer {
                    from_stop_id: "sp_1".to_string(),
                    to_stop_id: "sp_2".to_string(),
                    min_transfer_time: Some(50),
                    real_min_transfer_time: Some(60),
                    equipment_id: None,
                },
                Transfer {
                    from_stop_id: "sp_1".to_string(),
                    to_stop_id: "sp_3".to_string(),
                    min_transfer_time: Some(200),
                    real_min_transfer_time: Some(210),
                    equipment_id: None,
                },
                Transfer {
                    from_stop_id: "sp_2".to_string(),
                    to_stop_id: "sp_1".to_string(),
                    min_transfer_time: Some(83),
                    real_min_transfer_time: Some(203),
                    equipment_id: None,
                },
                Transfer {
                    from_stop_id: "sp_2".to_string(),
                    to_stop_id: "sp_2".to_string(),
                    min_transfer_time: Some(0),
                    real_min_transfer_time: Some(120),
                    equipment_id: None,
                },
                Transfer {
                    from_stop_id: "sp_3".to_string(),
                    to_stop_id: "sp_3".to_string(),
                    min_transfer_time: Some(0),
                    real_min_transfer_time: Some(120),
                    equipment_id: None,
                },
            ]
        );
    }

    #[test]
    fn test_generates_transfers_with_modification_rules() {
        let stop_areas = CollectionWithId::new(vec![
            StopArea {
                id: "sa_1".to_string(),
                name: "sa_name_1".to_string(),
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                visible: true,
                coord: Coord {
                    lon: 2.372075915336609,
                    lat: 48.84608210211328,
                },
                timezone: None,
                geometry_id: None,
                equipment_id: None,
            }]).unwrap();
        let transfers = Collection::new(vec![
            Transfer {
                from_stop_id: "sp_1".to_string(),
                to_stop_id: "sp_2".to_string(),
                min_transfer_time: Some(50),
                real_min_transfer_time: Some(60),
                equipment_id: None,
            },
            Transfer {
                from_stop_id: "sp_1".to_string(),
                to_stop_id: "sp_3".to_string(),
                min_transfer_time: Some(200),
                real_min_transfer_time: Some(210),
                equipment_id: None,
            },
        ]);

        let stop_points = CollectionWithId::new(vec![
            StopPoint {
                id: "sp_1".to_string(),
                name: "sp_name_1".to_string(),
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                visible: true,
                coord: Coord {
                    lon: 2.372075915336609,
                    lat: 48.84608210211328,
                },
                timezone: None,
                geometry_id: None,
                equipment_id: None,
                stop_area_id: "sa_1".to_string(),
                fare_zone_id: None,
                stop_type: StopType::Point,
            },
            StopPoint {
                id: "sp_2".to_string(),
                name: "sa_name_2".to_string(),
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                visible: true,
                coord: Coord {
                    lon: 2.371437549591065,
                    lat: 48.845665532277096,
                },
                timezone: None,
                geometry_id: None,
                equipment_id: None,
                stop_area_id: "sa_1".to_string(),
                fare_zone_id: None,
                stop_type: StopType::Point,
            },
            StopPoint {
                id: "sp_3".to_string(),
                name: "sa_name_3".to_string(),
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                visible: true,
                coord: Coord {
                    lon: 2.369517087936402,
                    lat: 48.845301913401144,
                },
                timezone: None,
                geometry_id: None,
                equipment_id: None,
                stop_area_id: "sa_1".to_string(),
                fare_zone_id: None,
                stop_type: StopType::Point,
            },
        ]).unwrap();

        let rules = vec![
            Rule {
                from_stop_id: "sp_3".to_string(),
                to_stop_id: "sp_3".to_string(),
                transfer_time: None,
            },
            Rule {
                from_stop_id: "sp_2".to_string(),
                to_stop_id: "sp_3".to_string(),
                transfer_time: Some(100),
            },
        ];

        let mut collections = Collections::default();
        collections.transfers = transfers;
        collections.stop_points = stop_points;
        collections.stop_areas = stop_areas;
        let mut model = Model::new(collections).unwrap();

        let transfers = super::do_generates_transfers(
            &mut model,
            100.0,
            0.785,
            120,
            &rules,
            &TransfersMode::IntraContributor,
        ).unwrap();

        assert_eq!(
            transfers,
            vec![
                Transfer {
                    from_stop_id: "sp_1".to_string(),
                    to_stop_id: "sp_1".to_string(),
                    min_transfer_time: Some(0),
                    real_min_transfer_time: Some(120),
                    equipment_id: None
                },
                Transfer {
                    from_stop_id: "sp_1".to_string(),
                    to_stop_id: "sp_2".to_string(),
                    min_transfer_time: Some(50),
                    real_min_transfer_time: Some(60),
                    equipment_id: None
                },
                Transfer {
                    from_stop_id: "sp_1".to_string(),
                    to_stop_id: "sp_3".to_string(),
                    min_transfer_time: Some(200),
                    real_min_transfer_time: Some(210),
                    equipment_id: None
                },
                Transfer {
                    from_stop_id: "sp_2".to_string(),
                    to_stop_id: "sp_1".to_string(),
                    min_transfer_time: Some(83),
                    real_min_transfer_time: Some(203),
                    equipment_id: None
                },
                Transfer {
                    from_stop_id: "sp_2".to_string(),
                    to_stop_id: "sp_2".to_string(),
                    min_transfer_time: Some(0),
                    real_min_transfer_time: Some(120),
                    equipment_id: None
                },
                Transfer {
                    from_stop_id: "sp_2".to_string(),
                    to_stop_id: "sp_3".to_string(),
                    min_transfer_time: Some(100),
                    real_min_transfer_time: Some(220),
                    equipment_id: None
                }
            ]
        );
    }
}
