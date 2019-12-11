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
use failure::bail;
use std::collections::{HashMap, HashSet};
use transit_model_collection::{CollectionWithId, Idx};

#[derive(Debug)]
pub enum Action {
    Extract,
    Remove,
}

/// Extract or remove networks
pub fn filter(model: Model, action: Action, network_ids: Vec<String>) -> Result<Model> {
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

    let mut networks = model.networks.clone();
    let n_id_to_old_idx = networks.get_id_to_idx().clone();
    let calendars = model.calendars.clone();
    let vjs = model.vehicle_journeys.clone();
    let old_vj_idx_to_vj_id: HashMap<Idx<VehicleJourney>, String> = model
        .vehicle_journeys
        .get_id_to_idx()
        .iter()
        .map(|(id, &idx)| (idx, id.clone()))
        .collect();

    let network_ids: HashSet<String> = network_ids
        .into_iter()
        .map(|id| match networks.get(&id) {
            Some(_) => Ok(id),
            None => bail!("network {} not found.", id),
        })
        .collect::<Result<HashSet<String>>>()?;

    match action {
        Action::Extract => networks.retain(|n| network_ids.contains(&n.id)),
        Action::Remove => networks.retain(|n| !network_ids.contains(&n.id)),
    }

    let network_idx = networks.values().map(|n| n_id_to_old_idx[&n.id]).collect();
    let calendars_used = model.get_corresponding(&network_idx);
    let vjs_used = model.get_corresponding(&network_idx);

    let mut collections = model.into_collections();

    collections
        .calendars
        .retain(|c| calendars_used.contains(&calendars.get_idx(&c.id).unwrap()));

    collections
        .vehicle_journeys
        .retain(|c| vjs_used.contains(&vjs.get_idx(&c.id).unwrap()));

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

    if collections.calendars.is_empty() {
        bail!("the data does not contain services anymore.")
    }

    collections.sanitize()?;
    Model::new(collections)
}
