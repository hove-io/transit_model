// Copyright 2017 Kisio Digital and/or its affiliates.
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

//! The `transit_model` crate proposes a model to manage transit data.
//! It can import and export data from [GTFS](http://gtfs.org/) and
//! [NTFS](https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/ntfs_fr.md).

use crate::{Model, Result};
use failure::bail;
use std::collections::HashSet;
use structopt::clap::arg_enum;

arg_enum! {
    #[derive(Debug)]
    pub enum Action {
        Extract,
        Remove,
    }
}

/// Extract or remove networks
pub fn filter(model: Model, action: Action, network_ids: Vec<String>) -> Result<Model> {
    let mut networks = model.networks.clone();
    let n_id_to_old_idx = networks.get_id_to_idx().clone();
    let calendars = model.calendars.clone();
    let vjs = model.vehicle_journeys.clone();

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

    if collections.calendars.is_empty() {
        bail!("the data does not contain services anymore.")
    }

    collections.sanitize()?;

    Model::new(collections)
}
