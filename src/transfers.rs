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

//! See function generates_transfers

use crate::{
    model::{Collections, Model},
    objects::{Coord, PhysicalMode, StopPoint, Transfer},
    physical_modes_utils::build_stop_point_physical_mode_map,
    Result,
};
use rstar::{RTree, RTreeObject, AABB};
use std::collections::HashMap;
use tracing::info;
use typed_index_collection::{Collection, CollectionWithId, Idx};

///structure for indexing transfers
pub type TransferMap = HashMap<(Idx<StopPoint>, Idx<StopPoint>), Transfer>;

/// The closure that will determine whether a connection should be created between 2 stops.
/// See [generates_transfers](./fn.generates_transfers.html).
pub type NeedTransfer<'a> = Box<dyn 'a + Fn(&Model, Idx<StopPoint>, Idx<StopPoint>) -> bool>;

/// Structure to determine the waiting time for a transfer between 2 physical modes.
pub type WaitingTimesByModes = HashMap<(Idx<PhysicalMode>, Idx<PhysicalMode>), u32>;

/// Build a map from existing transfers
pub fn get_available_transfers(
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

/// Wrapper for stop point with its index for use in R-tree
#[derive(Debug, Clone)]
struct StopPointLocation {
    idx: Idx<StopPoint>,
    coord: Coord,
}

impl RTreeObject for StopPointLocation {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_point([self.coord.lon, self.coord.lat])
    }
}

impl StopPointLocation {
    fn new(idx: Idx<StopPoint>, coord: Coord) -> Self {
        Self { idx, coord }
    }
}

/// Generate missing transfers from stop points within the required distance
///
/// This function uses an R-tree spatial index for efficient proximity queries.
/// R-trees are optimized for spatial data and provide O(log n) query performance.
///
/// Complexity: O(n × log n) for building the tree + O(n × k × log n) for queries
/// where k is the average number of nearby points within max_distance
pub fn generate_missing_transfers_from_sp(
    transfers_map: &TransferMap,
    model: &Model,
    max_distance: f64,
    walking_speed: f64,
    waiting_time: u32,
    need_transfer: Option<NeedTransfer>,
    waiting_time_by_modes: Option<WaitingTimesByModes>,
) -> TransferMap {
    info!("Adding missing transfers from stop points.");
    let mut new_transfers_map = TransferMap::new();
    let sq_max_distance = max_distance * max_distance;

    // Build R-tree for efficient spatial queries
    let stop_locations: Vec<StopPointLocation> = model
        .stop_points
        .iter()
        .filter(|(_, sp)| sp.coord != Coord::default())
        .map(|(idx, sp)| StopPointLocation::new(idx, sp.coord))
        .collect();

    let rtree = RTree::bulk_load(stop_locations);

    let stop_point_physical_mode_map = waiting_time_by_modes
        .is_some()
        .then(|| build_stop_point_physical_mode_map(model));

    // For each stop point, query nearby points from the R-tree
    for (idx1, sp1) in model.stop_points.iter() {
        if sp1.coord == Coord::default() {
            continue;
        }

        // Pre-calculate the approximation (cosinus of latitude) once for this stop
        // This optimizes distance calculations for all nearby points
        let approx = sp1.coord.approx();

        // Query R-tree for points within a bounding box
        // Note: locate_within_distance cannot be used here because it requires
        // Euclidean distance, but we need geographic distance calculation with approx()
        // Convert max_distance from meters to degrees
        // 1 degree latitude ≈ 111km everywhere
        // For longitude, it varies with latitude: 1 degree longitude ≈ 111km × cos(lat)
        // Use latitude conversion for the search box (more conservative)
        let search_distance_lat = max_distance / 111_000.0;
        // For longitude, use the pre-calculated approx (cos of latitude) to get the correct degree distance
        let search_distance_lon = max_distance / (111_000.0 * approx.cos_lat());
        let min_lon = sp1.coord.lon - search_distance_lon;
        let max_lon = sp1.coord.lon + search_distance_lon;
        let min_lat = sp1.coord.lat - search_distance_lat;
        let max_lat = sp1.coord.lat + search_distance_lat;

        let search_box = AABB::from_corners([min_lon, min_lat], [max_lon, max_lat]);

        let sp1_mode_idx = stop_point_physical_mode_map
            .as_ref()
            .and_then(|map| map.get(&idx1));

        // Get all points within the bounding box and filter by actual distance
        for nearby_location in rtree.locate_in_envelope(&search_box) {
            let idx2 = nearby_location.idx;

            if transfers_map.contains_key(&(idx1, idx2)) {
                continue;
            }
            if let Some(ref f) = need_transfer {
                if !f(model, idx1, idx2) {
                    continue;
                }
            }

            // Use the pre-calculated approximation for efficient distance calculation
            let sq_distance = approx.sq_distance_to(&nearby_location.coord);
            if sq_distance > sq_max_distance {
                continue;
            }
            let transfer_time = (sq_distance.sqrt() / walking_speed) as u32;
            let sp2 = &model.stop_points[idx2];

            // Use the specific waiting time for this pair of physical modes, or fall back to the default waiting time if not found
            let specific_waiting_time = sp1_mode_idx
                .and_then(|mode_idx1| {
                    let mode_idx2 = stop_point_physical_mode_map.as_ref()?.get(&idx2)?;
                    waiting_time_by_modes
                        .as_ref()?
                        .get(&(*mode_idx1, *mode_idx2))
                        .copied()
                })
                .unwrap_or(waiting_time);

            new_transfers_map.insert(
                (idx1, idx2),
                Transfer {
                    from_stop_id: sp1.id.clone(),
                    to_stop_id: sp2.id.clone(),
                    min_transfer_time: Some(transfer_time),
                    real_min_transfer_time: Some(transfer_time + specific_waiting_time),
                    equipment_id: None,
                },
            );
        }
    }

    new_transfers_map
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
    waiting_time_by_modes: Option<WaitingTimesByModes>,
) -> Result<Collections> {
    info!("Generating transfers...");

    let mut transfers_map = get_available_transfers(model.transfers.clone(), &model.stop_points);
    let new_transfers_map = generate_missing_transfers_from_sp(
        &transfers_map,
        &model,
        max_distance,
        walking_speed,
        waiting_time,
        need_transfer,
        waiting_time_by_modes,
    );

    transfers_map.extend(new_transfers_map);
    let mut new_transfers: Vec<_> = transfers_map.into_values().collect();
    new_transfers.sort_unstable_by(|t1, t2| {
        (&t1.from_stop_id, &t1.to_stop_id).cmp(&(&t2.from_stop_id, &t2.to_stop_id))
    });

    let mut collections = model.into_collections();
    collections.transfers = Collection::new(new_transfers);

    Ok(collections)
}

#[cfg(test)]
mod tests {
    use super::{
        generate_missing_transfers_from_sp, generates_transfers, get_available_transfers,
        TransferMap, WaitingTimesByModes,
    };
    use crate::model::{BUS_PHYSICAL_MODE, RAPID_TRANSIT_PHYSICAL_MODE};
    use crate::{
        model::Model,
        objects::{Coord, ObjectType, Time, Transfer},
        ModelBuilder,
    };
    use typed_index_collection::Collection;

    // A - B    92
    // A - C    158
    // B - C    66
    fn base_model() -> Model {
        let model_builder = ModelBuilder::default();
        let transit_model = model_builder
            .vj("vj1", |vj_builder| {
                vj_builder
                    .route("route1")
                    .st("A", "10:00:00")
                    .st("B", "11:00:00")
                    .st("C", "12:00:00");
            })
            .add_transfer("A", "B", Time::new(0, 10, 0))
            .build();

        let mut collections = transit_model.into_collections();
        collections.stop_points.get_mut("A").unwrap().coord =
            Coord::from(("2.38951".to_string(), "48.852245".to_string()));
        collections.stop_points.get_mut("B").unwrap().coord =
            Coord::from(("2.390403".to_string(), "48.85165".to_string()));
        collections.stop_points.get_mut("C").unwrap().coord =
            Coord::from(("2.390403".to_string(), "48.85165".to_string()));

        Model::new(collections).unwrap()
    }

    #[test]
    fn test_get_available_transfers() {
        let model = base_model();
        let transfers_map = get_available_transfers(model.transfers.clone(), &model.stop_points);

        let expected = {
            let mut map = TransferMap::new();
            let transfer_a_b = Transfer {
                from_stop_id: "A".to_string(),
                to_stop_id: "B".to_string(),
                min_transfer_time: Some(8),
                real_min_transfer_time: Some(10),
                equipment_id: None,
            };
            map.insert(
                (
                    model.stop_points.get_idx("A").unwrap(),
                    model.stop_points.get_idx("B").unwrap(),
                ),
                transfer_a_b,
            );
            map
        };
        assert_eq!(transfers_map, expected);
    }

    #[test]
    fn test_generate_missing_transfers_from_sp() {
        let model = base_model();
        let new_transfers_map = generate_missing_transfers_from_sp(
            &TransferMap::new(),
            &model,
            100.0,
            0.7,
            2,
            None,
            None,
        );

        let expected = {
            let mut map = TransferMap::new();
            let transfer_a_b = Transfer {
                from_stop_id: "A".to_string(),
                to_stop_id: "B".to_string(),
                min_transfer_time: Some(132),
                real_min_transfer_time: Some(134),
                equipment_id: None,
            };
            map.insert(
                (
                    model.stop_points.get_idx("A").unwrap(),
                    model.stop_points.get_idx("B").unwrap(),
                ),
                transfer_a_b,
            );
            let transfer_a_a = Transfer {
                from_stop_id: "A".to_string(),
                to_stop_id: "A".to_string(),
                min_transfer_time: Some(0),
                real_min_transfer_time: Some(2),
                equipment_id: None,
            };
            map.insert(
                (
                    model.stop_points.get_idx("A").unwrap(),
                    model.stop_points.get_idx("A").unwrap(),
                ),
                transfer_a_a,
            );
            let transfer_b_c = Transfer {
                from_stop_id: "B".to_string(),
                to_stop_id: "C".to_string(),
                min_transfer_time: Some(0),
                real_min_transfer_time: Some(2),
                equipment_id: None,
            };
            map.insert(
                (
                    model.stop_points.get_idx("B").unwrap(),
                    model.stop_points.get_idx("C").unwrap(),
                ),
                transfer_b_c,
            );
            let transfer_c_a = Transfer {
                from_stop_id: "C".to_string(),
                to_stop_id: "A".to_string(),
                min_transfer_time: Some(132),
                real_min_transfer_time: Some(134),
                equipment_id: None,
            };
            map.insert(
                (
                    model.stop_points.get_idx("C").unwrap(),
                    model.stop_points.get_idx("A").unwrap(),
                ),
                transfer_c_a,
            );
            let transfer_b_a = Transfer {
                from_stop_id: "B".to_string(),
                to_stop_id: "A".to_string(),
                min_transfer_time: Some(132),
                real_min_transfer_time: Some(134),
                equipment_id: None,
            };
            map.insert(
                (
                    model.stop_points.get_idx("B").unwrap(),
                    model.stop_points.get_idx("A").unwrap(),
                ),
                transfer_b_a,
            );
            let transfer_b_b = Transfer {
                from_stop_id: "B".to_string(),
                to_stop_id: "B".to_string(),
                min_transfer_time: Some(0),
                real_min_transfer_time: Some(2),
                equipment_id: None,
            };
            map.insert(
                (
                    model.stop_points.get_idx("B").unwrap(),
                    model.stop_points.get_idx("B").unwrap(),
                ),
                transfer_b_b,
            );
            let transfer_a_c = Transfer {
                from_stop_id: "A".to_string(),
                to_stop_id: "C".to_string(),
                min_transfer_time: Some(132),
                real_min_transfer_time: Some(134),
                equipment_id: None,
            };
            map.insert(
                (
                    model.stop_points.get_idx("A").unwrap(),
                    model.stop_points.get_idx("C").unwrap(),
                ),
                transfer_a_c,
            );
            let transfer_c_b = Transfer {
                from_stop_id: "C".to_string(),
                to_stop_id: "B".to_string(),
                min_transfer_time: Some(0),
                real_min_transfer_time: Some(2),
                equipment_id: None,
            };
            map.insert(
                (
                    model.stop_points.get_idx("C").unwrap(),
                    model.stop_points.get_idx("B").unwrap(),
                ),
                transfer_c_b,
            );
            let transfer_c_c = Transfer {
                from_stop_id: "C".to_string(),
                to_stop_id: "C".to_string(),
                min_transfer_time: Some(0),
                real_min_transfer_time: Some(2),
                equipment_id: None,
            };
            map.insert(
                (
                    model.stop_points.get_idx("C").unwrap(),
                    model.stop_points.get_idx("C").unwrap(),
                ),
                transfer_c_c,
            );
            map
        };

        assert_eq!(new_transfers_map, expected);
    }
    #[test]
    fn test_generates_transfers() {
        let model = base_model();
        let mut collections =
            generates_transfers(model, 100.0, 0.7, 2, None, None).expect("an error occured");

        let mut transfers = Collection::new(vec![
            Transfer {
                from_stop_id: "A".to_string(),
                to_stop_id: "B".to_string(),
                min_transfer_time: Some(8),
                real_min_transfer_time: Some(10),
                equipment_id: None,
            },
            Transfer {
                from_stop_id: "A".to_string(),
                to_stop_id: "A".to_string(),
                min_transfer_time: Some(0),
                real_min_transfer_time: Some(2),
                equipment_id: None,
            },
            Transfer {
                from_stop_id: "B".to_string(),
                to_stop_id: "C".to_string(),
                min_transfer_time: Some(0),
                real_min_transfer_time: Some(2),
                equipment_id: None,
            },
            Transfer {
                from_stop_id: "C".to_string(),
                to_stop_id: "A".to_string(),
                min_transfer_time: Some(132),
                real_min_transfer_time: Some(134),
                equipment_id: None,
            },
            Transfer {
                from_stop_id: "B".to_string(),
                to_stop_id: "A".to_string(),
                min_transfer_time: Some(132),
                real_min_transfer_time: Some(134),
                equipment_id: None,
            },
            Transfer {
                from_stop_id: "B".to_string(),
                to_stop_id: "B".to_string(),
                min_transfer_time: Some(0),
                real_min_transfer_time: Some(2),
                equipment_id: None,
            },
            Transfer {
                from_stop_id: "A".to_string(),
                to_stop_id: "C".to_string(),
                min_transfer_time: Some(132),
                real_min_transfer_time: Some(134),
                equipment_id: None,
            },
            Transfer {
                from_stop_id: "C".to_string(),
                to_stop_id: "B".to_string(),
                min_transfer_time: Some(0),
                real_min_transfer_time: Some(2),
                equipment_id: None,
            },
            Transfer {
                from_stop_id: "C".to_string(),
                to_stop_id: "C".to_string(),
                min_transfer_time: Some(0),
                real_min_transfer_time: Some(2),
                equipment_id: None,
            },
        ]);
        let mut transfers_expected = transfers.take();
        transfers_expected.sort_unstable_by(|t1, t2| {
            (&t1.from_stop_id, &t1.to_stop_id).cmp(&(&t2.from_stop_id, &t2.to_stop_id))
        });

        let mut transfers = collections.transfers.take();
        transfers.sort_unstable_by(|t1, t2| {
            (&t1.from_stop_id, &t1.to_stop_id).cmp(&(&t2.from_stop_id, &t2.to_stop_id))
        });

        assert_eq!(transfers, transfers_expected);
    }

    #[test]
    fn test_generates_transfers_all_within_distance() {
        let model_builder = ModelBuilder::default();
        let model = model_builder
            .stop_area("sa:1", |stop_area| {
                stop_area.name = "sa:1".to_string();
            })
            .stop_area("sa:2", |stop_area| {
                stop_area.name = "sa:2".to_string();
            })
            .stop_point("A", |stop_point| {
                stop_point.coord = ("2.37718".to_string(), "48.84680".to_string()).into();
                stop_point.stop_area_id = "sa:1".to_string();
            })
            .stop_point("B", |stop_point| {
                stop_point.coord = ("2.37146".to_string(), "48.84567".to_string()).into();
                stop_point.stop_area_id = "sa:1".to_string();
            })
            .stop_point("C", |stop_point| {
                stop_point.coord = ("2.37218".to_string(), "48.84665".to_string()).into();
                stop_point.stop_area_id = "sa:2".to_string();
            })
            .stop_point("D", |stop_point| {
                stop_point.coord = ("2.37511".to_string(), "48.84702".to_string()).into();
                stop_point.stop_area_id = "sa:2".to_string();
            })
            .vj("vj1", |vj_builder| {
                vj_builder
                    .st("A", "10:02:00")
                    .st("B", "10:04:00")
                    .st("C", "10:10:00")
                    .st("D", "10:15:00");
            })
            .build();

        let model = generates_transfers(model, 500.0, 1.0, 10, None, None).unwrap();

        assert_eq!(model.transfers.len(), 16);
    }

    #[test]
    fn test_generates_transfers_with_waiting_time_by_modes_matching() {
        // A, B served by Bus — C, D served by RapidTransit — E has no vehicle journey
        // B, C and E are very close, so transfers between them will be generated
        // A and D are too far from any other stop
        // Bus -> RapidTransit and RapidTransit -> Bus have distinct specific waiting times
        // Transfers involving E always fall back to default waiting time (no mode in map)
        let mut collections = ModelBuilder::default()
            .vj("vj1", |vj| {
                vj.route("route1")
                    .physical_mode(BUS_PHYSICAL_MODE)
                    .st("A", "10:00:00")
                    .st("B", "10:10:00");
            })
            .vj("vj2", |vj| {
                vj.route("route2")
                    .physical_mode(RAPID_TRANSIT_PHYSICAL_MODE)
                    .st("C", "11:00:00")
                    .st("D", "11:10:00");
            })
            .stop_area("SA:E", |_| {})
            .stop_point("E", |sp_builder| {
                sp_builder.stop_area_id = "SA:E".to_string()
            })
            .add_object_lock(&ObjectType::StopPoint, "E")
            .build()
            .into_collections();

        // Note: if geolocation is (0, 0), it's considered incorrect and transfer is not generated
        collections.stop_points.get_mut("A").unwrap().coord =
            Coord::from(("2.26723".to_string(), "48.84720".to_string()));
        collections.stop_points.get_mut("B").unwrap().coord =
            Coord::from(("2.35156".to_string(), "48.85762".to_string()));
        collections.stop_points.get_mut("C").unwrap().coord =
            Coord::from(("2.35180".to_string(), "48.85751".to_string())); // very close to B to ensure transfer is generated
        collections.stop_points.get_mut("D").unwrap().coord =
            Coord::from(("2.40242".to_string(), "48.86149".to_string()));
        collections.stop_points.get_mut("E").unwrap().coord =
            Coord::from(("2.35109".to_string(), "48.85767".to_string())); // very close to B to ensure transfer is generated

        let model = Model::new(collections).unwrap();

        assert_eq!(model.stop_points.len(), 5);

        let bus_idx = model.physical_modes.get_idx(BUS_PHYSICAL_MODE).unwrap();
        let rapid_transit_idx = model
            .physical_modes
            .get_idx(RAPID_TRANSIT_PHYSICAL_MODE)
            .unwrap();

        let bus_to_rapid = 180;
        let rapid_to_bus = 240;
        let default_waiting_time = 120;
        let mut waiting_time_by_modes = WaitingTimesByModes::new();
        waiting_time_by_modes.insert((bus_idx, rapid_transit_idx), bus_to_rapid);
        waiting_time_by_modes.insert((rapid_transit_idx, bus_idx), rapid_to_bus);

        let collections = generates_transfers(
            model,
            500.0,
            0.785,
            default_waiting_time,
            None,
            Some(waiting_time_by_modes),
        )
        .expect("an error occurred");

        // Self-transfers for all stops + B<->C + B<->E + C<->E
        assert_eq!(collections.transfers.len(), 11);

        let expected_waiting_times = [
            ("A", "A", default_waiting_time), // Bus -> Bus
            ("B", "B", default_waiting_time), // Bus -> Bus
            ("B", "C", bus_to_rapid),         // Bus -> RapidTransit
            ("B", "E", default_waiting_time), // Bus -> no mode (E not in map)
            ("C", "B", rapid_to_bus),         // RapidTransit -> Bus
            ("C", "C", default_waiting_time), // RapidTransit -> RapidTransit
            ("C", "E", default_waiting_time), // RapidTransit -> no mode (E not in map)
            ("D", "D", default_waiting_time), // RapidTransit -> RapidTransit
            ("E", "E", default_waiting_time), // no mode -> no mode
            ("E", "B", default_waiting_time), // no mode -> Bus (E not in map)
            ("E", "C", default_waiting_time), // no mode -> RapidTransit (E not in map)
        ];

        for (from, to, expected_wt) in expected_waiting_times {
            let transfer = collections
                .transfers
                .iter()
                .find(|(_, t)| t.from_stop_id == from && t.to_stop_id == to)
                .map(|(_, t)| t)
                .unwrap_or_else(|| panic!("transfer {from}->{to} not found", from = from, to = to));

            assert_eq!(
                transfer.real_min_transfer_time,
                transfer.min_transfer_time.map(|t| t + expected_wt),
                "transfer {from}->{to}: expected waiting time {expected_wt}"
            );
        }
    }
}
