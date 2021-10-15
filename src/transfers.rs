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
    objects::{Coord, StopPoint, Transfer},
    Result,
};
use std::collections::HashMap;
use tracing::{info, warn};
use typed_index_collection::{Collection, CollectionWithId, Idx};

type TransferMap = HashMap<(Idx<StopPoint>, Idx<StopPoint>), Transfer>;

/// The closure that will determine whether a connection should be created between 2 stops.
/// See [generates_transfers](./fn.generates_transfers.html).
pub type NeedTransfer<'a> = Box<dyn 'a + Fn(&Model, Idx<StopPoint>, Idx<StopPoint>) -> bool>;

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
    need_transfer: Option<NeedTransfer>,
) {
    info!("Adding missing transfers from stop points.");
    let sq_max_distance = max_distance * max_distance;
    for (idx1, sp1) in model.stop_points.iter() {
        if sp1.coord == Coord::default() {
            warn!("Stop Point {} geolocation is (0, 0), no transfer from this StopPoint will be generated.", sp1.id);
            continue;
        }
        let approx = sp1.coord.approx();
        for (idx2, sp2) in model.stop_points.iter() {
            if sp2.coord == Coord::default() {
                warn!("Stop Point {} geolocation is (0, 0), no transfer to this StopPoint will be generated.", sp2.id);
                continue;
            }
            if transfers_map.contains_key(&(idx1, idx2)) {
                continue;
            }
            if let Some(ref f) = need_transfer {
                if !f(model, idx1, idx2) {
                    continue;
                }
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
/// `need_transfer` Additional condition that determines whether a transfer
/// must be created between 2 stop points. By default transfers that do not
/// already exist and where the distance is less than `max_distance` will be created.
/// If you need an additional condition, you can use this parameter. For instance
/// you could create transfers between 2 stop points of different contributors only.
///
/// WARNING: if geolocation of either `StopPoint` is (0, 0), it's considered
/// incorrect and transfer is not generated to or from this `StopPoint`.
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
    need_transfer: Option<NeedTransfer>,
) -> Result<Model> {
    info!("Generating transfers...");
    let mut transfers_map = make_transfers_map(model.transfers.clone(), &model.stop_points);
    generate_transfers_from_sp(
        &mut transfers_map,
        &model,
        max_distance,
        walking_speed,
        waiting_time,
        need_transfer,
    );

    let mut new_transfers: Vec<_> = transfers_map.into_iter().map(|(_, v)| v).collect();
    new_transfers.sort_unstable_by(|t1, t2| {
        (&t1.from_stop_id, &t1.to_stop_id).cmp(&(&t2.from_stop_id, &t2.to_stop_id))
    });

    let mut collections = model.into_collections();
    collections.transfers = Collection::new(new_transfers);
    Model::new(collections)
}
