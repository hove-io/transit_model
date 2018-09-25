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

//! [NTFS](https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/ntfs_fr.md)
//! format management.

use collection::{Collection, CollectionWithId, Idx};
use csv;
use failure::ResultExt;
use objects::{StopPoint, Transfer};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use Result;

#[derive(Deserialize, Debug)]
struct Rule {
    from_stop_id: String,
    to_stop_id: String,
    transfer_time: Option<u32>,
}

type TransferMap = HashMap<(Idx<StopPoint>, Idx<StopPoint>), Transfer>;

fn read_rules<P: AsRef<Path>>(
    rule_files: Vec<P>,
    stop_points: &CollectionWithId<StopPoint>,
) -> Result<Vec<Rule>> {
    info!("Reading modificaton rules.");
    let mut rules = vec![];
    for rule_path in rule_files {
        let path = rule_path.as_ref();
        let mut rdr = csv::Reader::from_path(&path).with_context(ctx_from_path!(path))?;

        for rule in rdr.deserialize() {
            let rule: Rule = rule.with_context(ctx_from_path!(path))?;
            match (
                stop_points.get_idx(&rule.from_stop_id),
                stop_points.get_idx(&rule.to_stop_id),
            ) {
                (Some(_), Some(_)) => {
                    rules.push(rule);
                }
                (Some(_), None) => {
                    warn!("stop point {} not found", rule.from_stop_id);
                }
                (None, Some(_)) => {
                    warn!("stop point {} not found", rule.to_stop_id);
                }
                _ => {
                    warn!(
                        "stop points {} and {} not found",
                        rule.from_stop_id, rule.to_stop_id
                    );
                }
            }
        }
    }

    Ok(rules)
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
    stop_points: &CollectionWithId<StopPoint>,
    max_distance: f64,
    walking_speed: f64,
    waiting_time: u32,
) {
    info!("Adding missing transfers from stop points.");
    let sq_max_distance = max_distance * max_distance;
    for (idx1, sp1) in stop_points {
        let approx = sp1.coord.approx();
        for (idx2, sp2) in stop_points.iter() {
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
    transfers: &mut Collection<Transfer>,
    stop_points: &CollectionWithId<StopPoint>,
    max_distance: f64,
    walking_speed: f64,
    waiting_time: u32,
    rules: &[Rule],
) -> Vec<Transfer> {
    let mut transfers_map = make_transfers_map(transfers.take(), &stop_points);
    generate_transfers_from_sp(
        &mut transfers_map,
        stop_points,
        max_distance,
        walking_speed,
        waiting_time,
    );

    if !rules.is_empty() {
        remove_unwanted_transfers(&mut transfers_map, stop_points, rules);
        add_missing_transfers(&mut transfers_map, stop_points, rules, waiting_time);
    }

    let mut transfers: Vec<Transfer> = transfers_map.values().cloned().collect();
    transfers.sort_unstable_by(|t1, t2| {
        (&t1.from_stop_id, &t1.to_stop_id).cmp(&(&t2.from_stop_id, &t2.to_stop_id))
    });
    transfers
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
    transfers: &mut Collection<Transfer>,
    stop_points: &CollectionWithId<StopPoint>,
    max_distance: f64,
    walking_speed: f64,
    waiting_time: u32,
    rule_files: Vec<P>,
) -> Result<()> {
    info!("Generating transfers...");
    let rules = read_rules(rule_files, stop_points)?;
    let new_transfers = do_generates_transfers(
        transfers,
        stop_points,
        max_distance,
        walking_speed,
        waiting_time,
        &rules,
    );

    *transfers = Collection::new(new_transfers);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Rule;
    use collection::{Collection, CollectionWithId};
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
        let mut transfers = Collection::new(vec![
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
            },
        ]).unwrap();

        let transfers =
            super::do_generates_transfers(&mut transfers, &stop_points, 100.0, 0.785, 120, &vec![]);

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
        let mut transfers = Collection::new(vec![
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

        let transfers =
            super::do_generates_transfers(&mut transfers, &stop_points, 100.0, 0.785, 120, &rules);

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
