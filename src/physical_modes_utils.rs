// Copyright (C) 2017 Hove and/or its affiliates.
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

//! Utilities for physical modes

use crate::{
    model::{
        Model, AIR_PHYSICAL_MODE, BOAT_PHYSICAL_MODE, BUS_PHYSICAL_MODE,
        BUS_RAPID_TRANSIT_PHYSICAL_MODE, COACH_PHYSICAL_MODE, FERRY_PHYSICAL_MODE,
        FUNICULAR_PHYSICAL_MODE, LOCAL_TRAIN_PHYSICAL_MODE, LONG_DISTANCE_TRAIN_PHYSICAL_MODE,
        METRO_PHYSICAL_MODE, RAIL_SHUTTLE_PHYSICAL_MODE, RAPID_TRANSIT_PHYSICAL_MODE,
        SHUTTLE_PHYSICAL_MODE, SUSPENDED_CABLE_CAR_PHYSICAL_MODE, TAXI_PHYSICAL_MODE,
        TRAIN_PHYSICAL_MODE, TRAMWAY_PHYSICAL_MODE,
    },
    objects::{PhysicalMode, StopPoint},
};
use std::collections::HashMap;
use typed_index_collection::Idx;

/// Returns a priority order for a physical mode, used to pick the most relevant mode
/// when a stop point is served by multiple ones.
/// Lower value means higher priority (e.g. Train = 3, Bus = 7).
///
/// Priority order follows the NTFS specification:
/// see https://github.com/hove-io/ntfs-specification/blob/master/ntfs_fr.md#physical_modestxt-requis
pub fn get_physical_mode_order(physical_mode: &PhysicalMode) -> u8 {
    match physical_mode.id.as_str() {
        AIR_PHYSICAL_MODE => 1,
        BOAT_PHYSICAL_MODE | FERRY_PHYSICAL_MODE => 2,
        LOCAL_TRAIN_PHYSICAL_MODE
        | LONG_DISTANCE_TRAIN_PHYSICAL_MODE
        | RAPID_TRANSIT_PHYSICAL_MODE
        | RAIL_SHUTTLE_PHYSICAL_MODE
        | TRAIN_PHYSICAL_MODE => 3,
        METRO_PHYSICAL_MODE => 4,
        TRAMWAY_PHYSICAL_MODE => 5,
        FUNICULAR_PHYSICAL_MODE | SUSPENDED_CABLE_CAR_PHYSICAL_MODE => 6,
        BUS_PHYSICAL_MODE
        | BUS_RAPID_TRANSIT_PHYSICAL_MODE
        | COACH_PHYSICAL_MODE
        | SHUTTLE_PHYSICAL_MODE
        | TAXI_PHYSICAL_MODE => 7,
        _ => 8,
    }
}

/// Builds a map from each `StopPoint` index to its highest priority `PhysicalMode` index.
///
/// When a stop point is served by multiple physical modes, the one with the lowest order
/// value from `get_physical_mode_order` is selected (e.g. Train wins over Bus).
/// In practice these are often similar modes with the same hierarchy level (e.g. Train, LocalTrain, RapidTransit).
/// Stop points with no associated vehicle journey are absent from the returned map.
pub fn build_stop_point_physical_mode_map(
    model: &Model,
) -> HashMap<Idx<StopPoint>, Idx<PhysicalMode>> {
    model
        .stop_points
        .iter()
        .filter_map(|(stop_point_idx, _)| {
            let physical_mode_idx = model
                .get_corresponding_from_idx::<StopPoint, PhysicalMode>(stop_point_idx)
                .into_iter()
                .min_by_key(|&physical_mode_idx| {
                    get_physical_mode_order(&model.physical_modes[physical_mode_idx])
                })?;
            Some((stop_point_idx, physical_mode_idx))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::build_stop_point_physical_mode_map;
    use crate::physical_modes_utils::{
        BUS_RAPID_TRANSIT_PHYSICAL_MODE, RAPID_TRANSIT_PHYSICAL_MODE,
    };
    use crate::ModelBuilder;

    #[test]
    fn test_build_stop_point_physical_mode_map_single_mode() {
        // Stop point A served only by Bus
        let model = ModelBuilder::default()
            .vj("vj1", |vj| {
                vj.route("route1")
                    .physical_mode(BUS_RAPID_TRANSIT_PHYSICAL_MODE)
                    .st("A", "10:00:00")
                    .st("B", "10:10:00");
            })
            .build();

        let map = build_stop_point_physical_mode_map(&model);

        let sp_a_idx = model.stop_points.get_idx("A").unwrap();
        let sp_b_idx = model.stop_points.get_idx("B").unwrap();
        let bus_idx = model
            .physical_modes
            .get_idx(BUS_RAPID_TRANSIT_PHYSICAL_MODE)
            .unwrap();

        assert_eq!(map.get(&sp_a_idx), Some(&bus_idx));
        assert_eq!(map.get(&sp_b_idx), Some(&bus_idx));
    }

    #[test]
    fn test_build_stop_point_physical_mode_map_picks_highest_priority() {
        // Stop point A is served by both BusRapidTransit and RapidTransit — RapidTransit should win (order 3 < 7)
        let model = ModelBuilder::default()
            .vj("vj1", |vj| {
                vj.route("route1")
                    .physical_mode(RAPID_TRANSIT_PHYSICAL_MODE)
                    .st("A", "10:00:00")
                    .st("B", "10:10:00");
            })
            .vj("vj2", |vj| {
                vj.route("route2")
                    .physical_mode(BUS_RAPID_TRANSIT_PHYSICAL_MODE)
                    .st("A", "11:00:00")
                    .st("B", "11:10:00");
            })
            .build();

        let map = build_stop_point_physical_mode_map(&model);

        assert_eq!(model.vehicle_journeys.len(), 2);

        let sp_a_idx = model.stop_points.get_idx("A").unwrap();
        let rapid_transit_idx = model
            .physical_modes
            .get_idx(RAPID_TRANSIT_PHYSICAL_MODE)
            .unwrap();

        assert_eq!(map.get(&sp_a_idx), Some(&rapid_transit_idx));
    }
}
