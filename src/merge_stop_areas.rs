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

//! See function merge_stop_areas

use serde_json;

use crate::model::Collections;
use crate::objects::{CommentLinksT, KeysValues};
use crate::objects::{RestrictionType, StopArea};
use crate::utils::{Report, ReportType};
use crate::Result;
use csv;
use failure::{bail, ResultExt};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path;
use std::path::PathBuf;
use std::result::Result as StdResult;
use transit_model_collection::{Collection, CollectionWithId};

#[derive(Deserialize, Debug)]
struct StopAreaMergeRule {
    #[serde(rename = "stop_area_id")]
    id: String,
    #[serde(rename = "stop_area_name")]
    name: String,
    group: String,
    priority: u32,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct StopAreaGroupRule {
    master_stop_area_id: String,
    to_merge_stop_area_ids: Vec<String>,
}

impl StopAreaGroupRule {
    fn ensure_rule_valid(self, stop_area_ids: &[String], report: &mut Report) -> Result<Self> {
        let mut valid_rule = self.clone();
        let message = format!(
            "rule for master {} contains unexisting stop areas and is no longer valid",
            self.master_stop_area_id
        );
        let number_sa_to_merge = valid_rule.to_merge_stop_area_ids.len();
        valid_rule
            .to_merge_stop_area_ids
            .retain(|id| stop_area_ids.contains(&id));
        let number_existing_sa_to_merge = valid_rule.to_merge_stop_area_ids.len();
        if number_sa_to_merge != number_existing_sa_to_merge {
            report.add_warning(format!("rule for master {} does contains at least one stop area that does not exist anymore", self.master_stop_area_id),
                               ReportType::MissingToMerge);
        }
        if number_existing_sa_to_merge == 0 {
            report.add_error(
                format!(
                    "rule for master {} does not contain any existing stop areas to merge",
                    self.master_stop_area_id
                ),
                ReportType::NothingToMerge,
            );
            bail!(message);
        } else if !stop_area_ids.contains(&valid_rule.master_stop_area_id) {
            if valid_rule.to_merge_stop_area_ids.len() == 1 {
                report.add_error(
                    format!("master {} of rule does not exist anymore and cannot be replaced by an other one", self.master_stop_area_id),
                    ReportType::NoMasterPossible);
                bail!(message);
            }
            report.add_warning(
                format!(
                    "master {} of rule does not exist and has been replaced by an other one",
                    self.master_stop_area_id
                ),
                ReportType::MasterReplaced,
            );
            valid_rule.master_stop_area_id = valid_rule.to_merge_stop_area_ids.remove(0);
        }
        Ok(valid_rule)
    }
}

fn group_rules_from_file_rules(
    file_rules: Vec<StopAreaMergeRule>,
    report: &mut Report,
) -> Vec<StopAreaGroupRule> {
    let mut rules_with_priority: HashMap<String, Vec<(String, u32)>> = HashMap::new();
    for file_rule in file_rules {
        rules_with_priority
            .entry(file_rule.group)
            .or_insert_with(|| vec![])
            .push((file_rule.id.clone(), file_rule.priority));
    }
    let group_rules: HashMap<String, StopAreaGroupRule> = rules_with_priority
        .into_iter()
        .filter_map(|(k, mut stops_with_prio)| {
            stops_with_prio.sort_unstable_by_key(|stop_with_prio| stop_with_prio.1);
            if stops_with_prio.len() == 1 {
                report.add_warning(
                    format!("the rule of group {} contains only the stop area {}", k, stops_with_prio[0].0),
                    ReportType::OnlyOneStopArea);
                return None
            }
            else if stops_with_prio[0].1 == stops_with_prio[1].1 {
                report.add_warning(
                    format!("the rule of group {} contains ambiguous priorities for master: stop {} with {} and stop {} with {}", k, stops_with_prio[0].0, stops_with_prio[0].1, stops_with_prio[1].0, stops_with_prio[1].1),
                    ReportType::AmbiguousPriorities);
            }
            let master = stops_with_prio.remove(0);
            let others = stops_with_prio.into_iter().map(|stop_with_prio| stop_with_prio.0).collect();
            Some((
                k,
                StopAreaGroupRule {
                    master_stop_area_id: master.0,
                    to_merge_stop_area_ids: others,
                },
            ))
        }).collect();
    group_rules.values().cloned().collect()
}

fn read_rules<P: AsRef<path::Path>>(
    paths: Vec<P>,
    report: &mut Report,
) -> Result<Vec<StopAreaGroupRule>> {
    let mut rules: Vec<StopAreaGroupRule> = vec![];
    for rule_path in paths {
        let rule_path = rule_path.as_ref();
        let mut rdr = csv::Reader::from_path(&rule_path).with_context(ctx_from_path!(rule_path))?;
        let file_rules: Vec<StopAreaMergeRule> = rdr
            .deserialize()
            .collect::<StdResult<_, _>>()
            .with_context(ctx_from_path!(rule_path))?;
        rules.extend(group_rules_from_file_rules(file_rules, report));
    }
    Ok(rules)
}

fn generate_automatic_rules(
    stop_areas: &CollectionWithId<StopArea>,
    distance: u32,
) -> Vec<StopAreaGroupRule> {
    let mut stop_area_iter = stop_areas.values();
    let sq_max_distance: f64 = (distance * distance).into();
    let mut automatic_rules = vec![];
    while let Some(top_level_stop_area) = stop_area_iter.next() {
        let approx = top_level_stop_area.coord.approx();
        for bottom_level_stop_area in stop_area_iter.clone() {
            if top_level_stop_area.name == bottom_level_stop_area.name {
                let sq_distance = approx.sq_distance_to(&bottom_level_stop_area.coord);
                if sq_distance <= sq_max_distance {
                    automatic_rules.push(StopAreaGroupRule {
                        master_stop_area_id: top_level_stop_area.id.clone(),
                        to_merge_stop_area_ids: vec![bottom_level_stop_area.id.clone()],
                    });
                }
            }
        }
    }
    automatic_rules
}

fn apply_rules(
    mut collections: Collections,
    rules: Vec<StopAreaGroupRule>,
    report: &mut Report,
) -> Result<Collections> {
    let mut stop_points_updated = collections.stop_points.take();
    let mut geometries_updated = collections.geometries.take();
    let mut lines_updated = collections.lines.take();
    let mut ticket_use_restrictions_updated = collections.ticket_use_restrictions.take();
    let mut routes_updated = collections.routes.take();
    let mut stop_areas_to_remove: HashSet<String> = HashSet::new();
    let mut stop_area_ids = collections
        .stop_areas
        .values()
        .map(|stop_area| stop_area.id.clone())
        .collect::<Vec<String>>();
    for mut rule in rules {
        stop_area_ids.retain(|id| !stop_areas_to_remove.contains(id));
        rule = skip_fail!(rule.ensure_rule_valid(&stop_area_ids, report));
        for stop_point in &mut stop_points_updated {
            if rule
                .to_merge_stop_area_ids
                .contains(&stop_point.stop_area_id)
            {
                stop_point.stop_area_id = rule.master_stop_area_id.clone();
            }
        }
        for line in &mut lines_updated {
            if let Some(ref mut forward) = line.forward_direction {
                if rule.to_merge_stop_area_ids.contains(&forward) {
                    *forward = rule.master_stop_area_id.clone();
                }
            }
            if let Some(ref mut backward) = line.backward_direction {
                if rule.to_merge_stop_area_ids.contains(&backward) {
                    *backward = rule.master_stop_area_id.clone();
                }
            }
        }
        for ticket in &mut ticket_use_restrictions_updated {
            if ticket.restriction_type == RestrictionType::OriginDestination {
                if rule.to_merge_stop_area_ids.contains(&ticket.use_origin) {
                    ticket.use_origin = rule.master_stop_area_id.clone();
                }
                if rule
                    .to_merge_stop_area_ids
                    .contains(&ticket.use_destination)
                {
                    ticket.use_destination = rule.master_stop_area_id.clone();
                }
            }
        }
        for route in &mut routes_updated {
            if let Some(ref mut destination_id) = route.destination_id {
                if rule.to_merge_stop_area_ids.contains(&destination_id) {
                    *destination_id = rule.master_stop_area_id.clone();
                }
            }
        }
        let mut comment_links = CommentLinksT::default();
        let mut object_codes = KeysValues::default();
        let mut object_properties = KeysValues::default();
        for stop_area in collections.stop_areas.values() {
            if rule.to_merge_stop_area_ids.contains(&stop_area.id) {
                comment_links.extend(stop_area.comment_links.clone());
                object_codes.extend(stop_area.codes.clone());
                object_properties.extend(stop_area.object_properties.clone());
                object_codes.insert(("secondary_id".to_string(), stop_area.id.clone().to_string()));
                if let Some(ref geo_id) = stop_area.geometry_id.clone() {
                    geometries_updated.retain(|geo| &geo.id != geo_id);
                }
                stop_areas_to_remove.insert(stop_area.id.clone());
            }
        }
        let mut master_stop_area = collections
            .stop_areas
            .get_mut(&rule.master_stop_area_id)
            .unwrap();
        master_stop_area.comment_links.extend(comment_links);
        master_stop_area.codes.extend(object_codes);
        master_stop_area.object_properties.extend(object_properties);
    }
    let mut stop_areas_updated = collections.stop_areas.take();
    stop_areas_updated.retain(|sa| !stop_areas_to_remove.contains(&sa.id));
    collections.stop_points = CollectionWithId::new(stop_points_updated)?;
    collections.geometries = CollectionWithId::new(geometries_updated)?;
    collections.stop_areas = CollectionWithId::new(stop_areas_updated)?;
    collections.lines = CollectionWithId::new(lines_updated)?;
    collections.ticket_use_restrictions = Collection::new(ticket_use_restrictions_updated);
    collections.routes = CollectionWithId::new(routes_updated)?;
    Ok(collections)
}

/// Merge stop areas using manual rules from a csv file and automatic rules determined from a
/// specific distance
///
/// The `rule_paths` parameter allows you to specify the path of the csv file which contains the
/// manual rules
///
/// The `automatic_max_distance` parameter allows you to specify the max distance
/// in meters to compute a stop area merge
///
/// The `report_path` parameter allows you to specify the path of the file which will contain a
/// report of the errors and warning encountered during the merge
pub fn merge_stop_areas(
    mut collections: Collections,
    rule_paths: Vec<PathBuf>,
    automatic_max_distance: u32,
    report_path: PathBuf,
) -> Result<Collections> {
    let mut report = Report::default();
    let manual_rules = read_rules(rule_paths, &mut report)?;
    collections = apply_rules(collections, manual_rules, &mut report)?;
    let automatic_rules = generate_automatic_rules(&collections.stop_areas, automatic_max_distance);
    collections = apply_rules(collections, automatic_rules, &mut report)?;
    let serialized_report = serde_json::to_string(&report)?;
    fs::write(report_path, serialized_report)?;
    Ok(collections)
}
