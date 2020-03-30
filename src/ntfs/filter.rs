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

//! The `transit_model` crate proposes a model to manage transit data.
//! It can import and export data from [GTFS](http://gtfs.org/) and
//! [NTFS](https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fr.md).

use crate::{objects::VehicleJourney, Model, Result};
use failure::{bail, format_err};
use lazy_static::lazy_static;
use relational_types::IdxSet;
use std::collections::{HashMap, HashSet};
use typed_index_collection::{CollectionWithId, Id, Idx};

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Extract,
    Remove,
}

#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone)]
pub enum ObjectType {
    Network,
    Line,
}

type PropertyValues = HashMap<String, HashSet<String>>;

#[derive(Debug)]
pub struct Filter {
    action: Action,
    filters: HashMap<ObjectType, PropertyValues>,
}

impl Filter {
    pub fn new(action: Action) -> Self {
        Filter {
            action,
            filters: HashMap::new(),
        }
    }

    pub fn add<T: Into<String>, U: Into<String>>(
        &mut self,
        object_type: ObjectType,
        property: T,
        value: U,
    ) {
        let props = self.filters.entry(object_type).or_insert_with(HashMap::new);
        props
            .entry(property.into())
            .or_insert_with(HashSet::new)
            .insert(value.into());
    }
}

type FnFilter = Box<dyn Fn(&Model, &str) -> Result<IdxSet<VehicleJourney>> + Send + Sync>;
lazy_static! {
    static ref PROPERTY_FILTERS: HashMap<ObjectType, HashMap<&'static str, FnFilter>> = {
        let mut m: HashMap<ObjectType, HashMap<&'static str, FnFilter>> = HashMap::new();

        // Network filters
        let mut network_filters: HashMap<&'static str, FnFilter> = HashMap::new();
        network_filters.insert(
            "network_id",
            Box::new(|model, network_id| {
                model
                    .networks
                    .get_idx(&network_id)
                    .ok_or_else(|| format_err!("Network '{}' not found.", network_id))
                    .map(|network_idx| model.get_corresponding_from_idx(network_idx))
            }),
        );
        m.insert(ObjectType::Network, network_filters);

        // Line filters
        let mut line_filters: HashMap<&'static str, FnFilter> = HashMap::new();
        line_filters.insert("line_code",
            Box::new(|model, line_code| {
                Ok(model
                    .lines
                    .values()
                    .filter(|line| line.code.as_deref() == Some(line_code))
                    // Unwrap is safe because we're iterating on model.lines already
                    .map(|line| model.lines.get_idx(&line.id).unwrap())
                    .flat_map(|line_idx| model.get_corresponding_from_idx(line_idx))
                    .collect())
            }),
        );

        line_filters.insert(
            "line_id",
            Box::new(|model, line_id| {
                model
                    .lines
                    .get_idx(&line_id)
                    .ok_or_else(|| format_err!("Line '{}' not found.", line_id))
                    .map(|line_idx| model.get_corresponding_from_idx(line_idx))
            }),
        );
        m.insert(ObjectType::Line, line_filters);
        m
    };
}

fn filter_by_property(
    model: &Model,
    object_type: ObjectType,
    property: &str,
    value: &str,
) -> Result<IdxSet<VehicleJourney>> {
    let filter_function = PROPERTY_FILTERS
        .get(&object_type)
        .ok_or_else(|| format_err!("Object of type '{:?}' are not yet supported", object_type))?
        .get(property)
        .ok_or_else(|| format_err!("Property '{}' not yet supported.", property))?;
    filter_function(model, value)
}

fn filter_from_idxset<T: Id<T>>(
    collection: &mut CollectionWithId<T>,
    idx_set: IdxSet<T>,
    action: Action,
) {
    let ids: Vec<String> = idx_set
        .into_iter()
        .map(|idx| collection[idx].id().to_string())
        .collect();
    let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    collection.retain(|object| match action {
        Action::Extract => id_refs.contains(&object.id()),
        Action::Remove => !id_refs.contains(&object.id()),
    });
}

fn updated_stop_time_attributes<T>(
    vehicle_journeys: &CollectionWithId<VehicleJourney>,
    attributes_map: &HashMap<(Idx<VehicleJourney>, u32), T>,
    old_vj_idx_to_vj_id: &HashMap<Idx<VehicleJourney>, String>,
) -> HashMap<(Idx<VehicleJourney>, u32), T>
where
    T: Clone,
{
    let mut updated_attributes_map = HashMap::new();
    for (&(old_vj_idx, sequence), attribute) in attributes_map {
        if let Some(new_vj_idx) = old_vj_idx_to_vj_id
            .get(&old_vj_idx)
            .and_then(|vj_id| vehicle_journeys.get_idx(vj_id))
        {
            updated_attributes_map.insert((new_vj_idx, sequence), attribute.clone());
        }
    }
    updated_attributes_map
}

/// Extract or remove part of the dataset from property filters on an object (Network, Line, etc.)
pub fn filter(model: Model, filter: &Filter) -> Result<Model> {
    let selected_vjs = filter
        .filters
        .iter()
        .flat_map(|(object_type, property_values)| {
            property_values
                .iter()
                .map(move |(property, values)| (object_type, property, values))
        })
        .flat_map(|(object_type, property, values)| {
            values
                .iter()
                .map(move |value| (object_type, property, value))
        })
        .map(|(object_type, property, value)| {
            filter_by_property(&model, *object_type, property.as_str(), value.as_str())
        })
        .try_fold::<_, _, Result<IdxSet<VehicleJourney>>>(
            IdxSet::new(),
            |mut vehicle_journeys_indexes, idx_set| {
                vehicle_journeys_indexes.extend(idx_set?);
                Ok(vehicle_journeys_indexes)
            },
        )?;

    let mut collections = model.into_collections();
    let old_vj_idx_to_vj_id: HashMap<Idx<VehicleJourney>, String> = collections
        .vehicle_journeys
        .get_id_to_idx()
        .iter()
        .map(|(id, &idx)| (idx, id.clone()))
        .collect();

    filter_from_idxset(
        &mut collections.vehicle_journeys,
        selected_vjs,
        filter.action,
    );

    collections.stop_time_ids = updated_stop_time_attributes(
        &collections.vehicle_journeys,
        &collections.stop_time_ids,
        &old_vj_idx_to_vj_id,
    );
    collections.stop_time_headsigns = updated_stop_time_attributes(
        &collections.vehicle_journeys,
        &collections.stop_time_headsigns,
        &old_vj_idx_to_vj_id,
    );
    collections.stop_time_comments = updated_stop_time_attributes(
        &collections.vehicle_journeys,
        &collections.stop_time_comments,
        &old_vj_idx_to_vj_id,
    );

    if collections.vehicle_journeys.is_empty() {
        bail!("the data does not contain vehicle journeys anymore.")
    }

    Model::new(collections)
}
