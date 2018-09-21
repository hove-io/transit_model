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

use csv;
use failure::ResultExt;
use std::cmp::Ordering;
use std::collections::hash_map::Entry::*;
use std::collections::HashMap;
use std::path;
use std::result::Result as StdResult;

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
    }
}

impl Ord for StopAreaGroupRule {
    fn cmp(&self, other: &StopAreaGroupRule) -> Ordering {
        self.master_stop_area_id.cmp(&other.master_stop_area_id)
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
        .map(|(k, v)| {
            let master = &v.iter().min_by_key(|x| x.1).unwrap().0;
            let others = v
                .iter()
                .filter_map(|ref x| {
                    if x.0 != *master {
                        Some(x.0.clone())
                    } else {
                        None
                    }
                }).collect();
            (
                k.clone(),
                StopAreaGroupRule {
                    master_stop_area_id: master.clone(),
                    to_merge_stop_area_ids: others,
                },
            )
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
