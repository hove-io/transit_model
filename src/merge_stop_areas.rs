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

extern crate serde;
extern crate serde_json;

use collection::CollectionWithId;
use csv;
use failure::ResultExt;
use model::Collections;
use objects::StopArea;
use objects::{CommentLinksT, KeysValues};
use std::cmp::Ordering;
use std::collections::hash_map::Entry::*;
use std::collections::HashMap;
use std::fs;
use std::path;
use std::path::PathBuf;
use std::result::Result as StdResult;
use Result;

#[derive(Deserialize, Debug)]
struct StopAreaMergeRule {
    #[serde(rename = "stop_area_id")]
    id: String,
    #[serde(rename = "stop_area_name")]
    name: String,
    group: String,
    priority: u16,
}

#[derive(Debug, Clone, Eq)]
struct StopAreaGroupRule {
    master_stop_area_id: String,
    to_merge_stop_area_ids: Vec<String>,
}

impl StopAreaGroupRule {
    fn ensure_rule_valid(
        self,
        stop_area_ids: &Vec<String>,
        report: &mut MergeStopAreasReport,
    ) -> Result<Self> {
        let mut valid_rule = self.clone();
        let message = format!(
            "rule for master {} contains unexisting stop areas and is no longer valid",
            self.master_stop_area_id
        );
        valid_rule
            .to_merge_stop_area_ids
            .retain(|id| stop_area_ids.contains(&id));
        if valid_rule.to_merge_stop_area_ids.len() == 0 {
            report.add_error(
                format!(
                    "rule for master {} does not contain any existing stop areas to merge",
                    self.master_stop_area_id
                ),
                MergeStopAreasReportType::NothingToMerge,
            );
            bail!(message);
        } else if !stop_area_ids.contains(&valid_rule.master_stop_area_id) {
            if valid_rule.to_merge_stop_area_ids.len() == 1 {
                report.add_error(
                    format!("master {} of rule does not exist anymore and cannot be replaced by an other one", self.master_stop_area_id),
                     MergeStopAreasReportType::NoMasterPossible);
                bail!(message);
            }
            report.add_warning(
                format!(
                    "master {} of rule does not exist and has been replaced by an other one",
                    self.master_stop_area_id
                ),
                MergeStopAreasReportType::MasterReplaced,
            );
            valid_rule.master_stop_area_id = valid_rule.to_merge_stop_area_ids.remove(0);
        }
        Ok(valid_rule)
    }
}

impl PartialEq for StopAreaGroupRule {
    fn eq(&self, other: &StopAreaGroupRule) -> bool {
        self.master_stop_area_id == other.master_stop_area_id
            && self.to_merge_stop_area_ids == other.to_merge_stop_area_ids
    }
}

impl Ord for StopAreaGroupRule {
    fn cmp(&self, other: &StopAreaGroupRule) -> Ordering {
        match self.master_stop_area_id.cmp(&other.master_stop_area_id) {
            Ordering::Equal => self
                .to_merge_stop_area_ids
                .cmp(&other.to_merge_stop_area_ids),
            strict_order => strict_order,
        }
    }
}

impl PartialOrd for StopAreaGroupRule {
    fn partial_cmp(&self, other: &StopAreaGroupRule) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Serialize)]
enum MergeStopAreasReportType {
    OnlyOneStopArea,
    AmbiguousPriorities,
    NothingToMerge,
    NoMasterPossible,
    MasterReplaced,
}

#[derive(Debug, Serialize)]
struct MergeStopAreasReportRow {
    category: MergeStopAreasReportType,
    message: String,
}

#[derive(Debug, Serialize)]
struct MergeStopAreasReport {
    errors: Vec<MergeStopAreasReportRow>,
    warnings: Vec<MergeStopAreasReportRow>,
}

impl MergeStopAreasReport {
    pub fn new() -> Self {
        Self {
            errors: vec![],
            warnings: vec![],
        }
    }
    pub fn add_warning(&mut self, warning: String, warning_type: MergeStopAreasReportType) {
        self.warnings.push(MergeStopAreasReportRow {
            category: warning_type,
            message: warning,
        });
    }
    pub fn add_error(&mut self, error: String, error_type: MergeStopAreasReportType) {
        self.errors.push(MergeStopAreasReportRow {
            category: error_type,
            message: error,
        });
    }
}

fn group_rules_from_file_rules(
    file_rules: Vec<StopAreaMergeRule>,
    report: &mut MergeStopAreasReport,
) -> Vec<StopAreaGroupRule> {
    let ref mut rules_with_priority: HashMap<String, Vec<(String, u16)>> = HashMap::new();
    for file_rule in file_rules {
        match rules_with_priority.entry(file_rule.group.clone()) {
            Occupied(mut entry) => entry.get_mut().push((file_rule.id, file_rule.priority)),
            Vacant(entry) => {
                entry.insert(vec![(file_rule.id, file_rule.priority)]);
                ()
            }
        }
    }
    let group_rules: HashMap<String, StopAreaGroupRule> = rules_with_priority
        .iter()
        .filter_map(|(k, v)| {
            let mut stops_with_prio = v.clone();
            stops_with_prio.sort_unstable_by_key(|stop_with_prio| stop_with_prio.1);
            if stops_with_prio.len() == 1 {
                report.add_warning(
                    format!("the rule of group {} contains only the stop area {}",
                        k, stops_with_prio[0].0),
                    MergeStopAreasReportType::OnlyOneStopArea);
                return None
            }
            else if stops_with_prio[0].1 == stops_with_prio[1].1 {
                report.add_warning(
                    format!("the rule of group {} contains ambiguous priorities for master: stop {} with {} and stop {} with {}",
                        k, stops_with_prio[0].0, stops_with_prio[0].1, stops_with_prio[1].0, stops_with_prio[1].1),
                    MergeStopAreasReportType::AmbiguousPriorities);
            }
            let master = stops_with_prio.remove(0);
            let others = stops_with_prio.into_iter().map(|stop_with_prio| stop_with_prio.0).collect();
            Some((
                k.clone(),
                StopAreaGroupRule {
                    master_stop_area_id: master.0,
                    to_merge_stop_area_ids: others,
                },
            ))
        }).collect();
    group_rules.values().into_iter().cloned().collect()
}

fn read_rules<P: AsRef<path::Path>>(
    paths: Vec<P>,
    report: &mut MergeStopAreasReport,
) -> Vec<StopAreaGroupRule> {
    let mut rules: Vec<StopAreaGroupRule> = vec![];
    for rule_path in paths {
        let rule_path = rule_path.as_ref();
        let mut rdr = csv::Reader::from_path(&rule_path)
            .with_context(ctx_from_path!(rule_path))
            .unwrap();
        let file_rules: Vec<StopAreaMergeRule> = rdr
            .deserialize()
            .collect::<StdResult<_, _>>()
            .with_context(ctx_from_path!(rule_path))
            .unwrap();
        rules.extend(group_rules_from_file_rules(file_rules, report));
    }
    rules
}

fn generate_automatic_rules(
    stop_areas: &CollectionWithId<StopArea>,
    distance: u16,
) -> Vec<StopAreaGroupRule> {
    let mut stop_area_iter = stop_areas.values();
    let sq_max_distance: f64 = (distance * distance).into();
    let mut automatic_rules = vec![];
    while let Some(top_level_stop_area) = stop_area_iter.next() {
        for bottom_level_stop_area in stop_area_iter.clone() {
            if top_level_stop_area.name == bottom_level_stop_area.name {
                let approx = top_level_stop_area.coord.approx();
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
    report: &mut MergeStopAreasReport,
) -> Collections {
    let mut stop_points_updated = collections.stop_points.take();
    let mut geometries_updated = collections.geometries.take();
    let mut lines_updated = collections.lines.take();
    let mut stop_areas_to_remove: Vec<String> = Vec::new();
    let mut stop_area_ids = collections
        .stop_areas
        .values()
        .map(|stop_area| stop_area.id.clone())
        .collect::<Vec<String>>();
    for mut rule in rules {
        stop_area_ids.retain(|id| !stop_areas_to_remove.contains(&id));
        rule = skip_fail!(rule.ensure_rule_valid(&stop_area_ids, report));
        for stop_point in stop_points_updated.iter_mut() {
            if rule
                .to_merge_stop_area_ids
                .contains(&stop_point.stop_area_id)
            {
                stop_point.stop_area_id = rule.master_stop_area_id.clone();
            }
        }
        for line in lines_updated.iter_mut() {
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
                stop_areas_to_remove.push(stop_area.id.clone());
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
    collections.stop_points = CollectionWithId::new(stop_points_updated).unwrap();
    collections.geometries = CollectionWithId::new(geometries_updated).unwrap();
    collections.stop_areas = CollectionWithId::new(stop_areas_updated).unwrap();
    collections.lines = CollectionWithId::new(lines_updated).unwrap();
    collections
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
    automatic_max_distance: u16,
    report_path: PathBuf,
) -> Collections {
    let mut report = MergeStopAreasReport::new();
    let manual_rules = read_rules(rule_paths, &mut report);
    collections = apply_rules(collections, manual_rules, &mut report);
    let automatic_rules = generate_automatic_rules(&collections.stop_areas, automatic_max_distance);
    collections = apply_rules(collections, automatic_rules, &mut report);
    let serialized_report = serde_json::to_string(&report).unwrap();
    fs::write(report_path, serialized_report).expect("Unable to write report file");
    collections
}
