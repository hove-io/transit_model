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

use collection::CollectionWithId;
use csv;
use failure::ResultExt;
use model::Collections;
use objects::StopArea;
use objects::{CommentLinksT, KeysValues};
use std::cmp::Ordering;
use std::collections::hash_map::Entry::*;
use std::collections::HashMap;
use std::path;
use std::result::Result as StdResult;
use Result;

#[derive(Deserialize, Debug)]
pub struct StopAreaMergeRule {
    #[serde(rename = "stop_area_id")]
    id: String,
    #[serde(rename = "stop_area_name")]
    name: String,
    group: String,
    priority: u16,
}

#[derive(Debug, Clone, Eq)]
pub struct StopAreaGroupRule {
    pub master_stop_area_id: String,
    pub to_merge_stop_area_ids: Vec<String>,
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

fn group_rules_from_file_rules(file_rules: Vec<StopAreaMergeRule>) -> Vec<StopAreaGroupRule> {
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
                warn!("the rule of group {} contains only the stop area {}", k, stops_with_prio[0].0);
                return None
            }
            else if stops_with_prio[0].1 == stops_with_prio[1].1 {
                warn!("the rule of group {} contains ambiguous priorities for master: stop {} with {} and stop {} with {}", k, stops_with_prio[0].0, stops_with_prio[0].1, stops_with_prio[1].0, stops_with_prio[1].1);
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

pub fn read_rules<P: AsRef<path::Path>>(paths: Vec<P>) -> Vec<StopAreaGroupRule> {
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
        rules.extend(group_rules_from_file_rules(file_rules));
    }
    rules
}

pub fn generate_automatic_rules(
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

fn ensure_rule_valid(
    rule: StopAreaGroupRule,
    stop_area_ids: &Vec<String>,
) -> Result<StopAreaGroupRule> {
    let mut valid_rule = rule.clone();
    valid_rule
        .to_merge_stop_area_ids
        .retain(|id| stop_area_ids.contains(&id));
    if valid_rule.to_merge_stop_area_ids.len() == 0 {
        bail!("rule {:?} is no longer valid because none of the stop areas remaining to merge are existing", rule)
    } else if !stop_area_ids.contains(&valid_rule.master_stop_area_id) {
        if valid_rule.to_merge_stop_area_ids.len() == 1 {
            bail!("rule {:?} is no longer valid because master stop area no longer exists and only one candidate is found", rule)
        }
        valid_rule.master_stop_area_id = valid_rule.to_merge_stop_area_ids.remove(0);
    }
    Ok(valid_rule)
}

pub fn apply_rules(mut collections: Collections, rules: Vec<StopAreaGroupRule>) -> Collections {
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
        match ensure_rule_valid(rule, &stop_area_ids) {
            Ok(valid_rule) => rule = valid_rule,
            Err(e) => {
                warn!("{}", e);
                continue;
            }
        }
        println!(
            "look for parent {:?} to merge {:?}",
            rule.master_stop_area_id, rule.to_merge_stop_area_ids
        );
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
