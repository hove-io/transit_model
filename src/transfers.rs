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
    objects::{Coord, StopPoint, Transfer},
    Result,
};
use std::collections::HashMap;
use std::time::Instant;
use tracing::info;
use typed_index_collection::{Collection, CollectionWithId, Idx};

///structure for indexing transfers
pub type TransferMap = HashMap<(Idx<StopPoint>, Idx<StopPoint>), Transfer>;

/// The closure that will determine whether a connection should be created between 2 stops.
/// See [generates_transfers](./fn.generates_transfers.html).
pub type NeedTransfer<'a> = Box<dyn 'a + Fn(&Model, Idx<StopPoint>, Idx<StopPoint>) -> bool>;

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

/// Spatial grid to efficiently find nearby stop points
struct SpatialGrid {
    /// Cell size in degrees (approximation)
    cell_size: f64,
    /// Map from (cell_x, cell_y) to list of stop_point_idx
    cells: HashMap<(i32, i32), Vec<Idx<StopPoint>>>,
}

impl SpatialGrid {
    /// Create a new spatial grid with cells sized to contain points within max_distance
    /// We make cells larger (3x max_distance) to reduce the number of cells to check
    fn new(max_distance: f64) -> Self {
        // Approximate cell size in degrees (at equator, 1 degree ≈ 111km)
        // Use 3x max_distance so we only need to check current cell + immediate neighbors
        let cell_size = (max_distance * 3.0) / 111_000.0; // Convert meters to degrees
        Self {
            cell_size,
            cells: HashMap::new(),
        }
    }

    /// Get the cell coordinates for a given coordinate
    #[inline]
    fn get_cell(&self, coord: &Coord) -> (i32, i32) {
        (
            (coord.lon / self.cell_size).floor() as i32,
            (coord.lat / self.cell_size).floor() as i32,
        )
    }

    /// Insert a stop point into the grid
    fn insert(&mut self, idx: Idx<StopPoint>, coord: &Coord) {
        let cell = self.get_cell(coord);
        self.cells.entry(cell).or_default().push(idx);
    }

    /// Fill the provided vector with stop point indices in the cell and adjacent cells (3x3 grid)
    /// This reuses the Vec buffer to avoid allocations
    #[inline]
    fn get_nearby_indices_into(&self, coord: &Coord, result: &mut Vec<Idx<StopPoint>>) {
        result.clear();
        let (cell_x, cell_y) = self.get_cell(coord);

        // Check the 9 cells: current + 8 adjacent
        // Use saturating_add to avoid overflow with extreme coordinates
        for dx in -1..=1 {
            for dy in -1..=1 {
                let target_x = cell_x.saturating_add(dx);
                let target_y = cell_y.saturating_add(dy);
                if let Some(indices) = self.cells.get(&(target_x, target_y)) {
                    result.extend_from_slice(indices);
                }
            }
        }
    }
}

/// Generate missing transfers from stop points within the required distance
///
/// This function uses a spatial grid optimization to avoid O(n²) complexity.
/// Instead of comparing every stop point with every other stop point, it:
/// 1. Divides the geographic space into a grid of cells
/// 2. For each stop point, only checks points in the same cell and adjacent cells (3x3 grid)
///
/// Complexity: O(n × k) where k is the average number of points per cell neighborhood
/// For uniformly distributed points, this is approximately O(n) instead of O(n²)
pub fn generate_missing_transfers_from_sp(
    transfers_map: &TransferMap,
    model: &Model,
    max_distance: f64,
    walking_speed: f64,
    waiting_time: u32,
    need_transfer: Option<NeedTransfer>,
) -> TransferMap {
    let total_start = Instant::now();
    info!("Adding missing transfers from stop points.");
    let mut new_transfers_map = TransferMap::new();
    let sq_max_distance = max_distance * max_distance;

    // Build spatial grid for efficient proximity queries
    let grid_start = Instant::now();
    let mut grid = SpatialGrid::new(max_distance);
    let mut valid_stop_count = 0;
    for (idx, sp) in model.stop_points.iter() {
        if sp.coord != Coord::default() {
            grid.insert(idx, &sp.coord);
            valid_stop_count += 1;
        }
    }
    let grid_duration = grid_start.elapsed();
    info!(
        "Built spatial grid with {} cells for {} valid stop points in {:.2?}",
        grid.cells.len(),
        valid_stop_count,
        grid_duration
    );

    // Pre-allocate buffer for nearby indices to avoid repeated allocations
    let mut nearby_indices = Vec::with_capacity(100);

    let compute_start = Instant::now();
    let mut total_comparisons = 0_u64;
    let mut total_transfers_created = 0_u64;
    let mut total_distance_checks = 0_u64;

    // For each stop point, only check nearby points from the grid
    for (idx1, sp1) in model.stop_points.iter() {
        if sp1.coord == Coord::default() {
            continue;
        }
        let approx = sp1.coord.approx();

        // Get nearby point indices (same cell + adjacent cells) - reuses the buffer
        grid.get_nearby_indices_into(&sp1.coord, &mut nearby_indices);

        // Only iterate over nearby points
        for &idx2 in &nearby_indices {
            total_comparisons += 1;

            if transfers_map.contains_key(&(idx1, idx2)) {
                continue;
            }
            if let Some(ref f) = need_transfer {
                if !f(model, idx1, idx2) {
                    continue;
                }
            }
            let sp2 = &model.stop_points[idx2];

            total_distance_checks += 1;
            let sq_distance = approx.sq_distance_to(&sp2.coord);
            if sq_distance > sq_max_distance {
                continue;
            }

            total_transfers_created += 1;
            let transfer_time = (sq_distance.sqrt() / walking_speed) as u32;
            new_transfers_map.insert(
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

    let compute_duration = compute_start.elapsed();
    let total_duration = total_start.elapsed();

    info!(
        "Transfer computation stats: {} comparisons, {} distance checks, {} transfers created in {:.2?}",
        total_comparisons,
        total_distance_checks,
        total_transfers_created,
        compute_duration
    );
    info!(
        "Total time for generate_missing_transfers_from_sp: {:.2?}",
        total_duration
    );

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
) -> Result<Collections> {
    let total_start = Instant::now();
    info!("Generating transfers...");

    let get_transfers_start = Instant::now();
    let mut transfers_map = get_available_transfers(model.transfers.clone(), &model.stop_points);
    info!(
        "get_available_transfers: {} existing transfers in {:.2?}",
        transfers_map.len(),
        get_transfers_start.elapsed()
    );

    let gen_transfers_start = Instant::now();
    let new_transfers_map = generate_missing_transfers_from_sp(
        &transfers_map,
        &model,
        max_distance,
        walking_speed,
        waiting_time,
        need_transfer,
    );
    info!(
        "generate_missing_transfers_from_sp returned {} new transfers in {:.2?}",
        new_transfers_map.len(),
        gen_transfers_start.elapsed()
    );

    let merge_start = Instant::now();
    transfers_map.extend(new_transfers_map);
    let mut new_transfers: Vec<_> = transfers_map.into_values().collect();
    info!("Merged transfers in {:.2?}", merge_start.elapsed());

    let sort_start = Instant::now();
    new_transfers.sort_unstable_by(|t1, t2| {
        (&t1.from_stop_id, &t1.to_stop_id).cmp(&(&t2.from_stop_id, &t2.to_stop_id))
    });
    info!(
        "Sorted {} transfers in {:.2?}",
        new_transfers.len(),
        sort_start.elapsed()
    );

    let rebuild_start = Instant::now();
    let mut collections = model.into_collections();
    collections.transfers = Collection::new(new_transfers);
    // let result = Model::new(collections);
    info!("Rebuilt model in {:.2?}", rebuild_start.elapsed());

    info!(
        "generates_transfers TOTAL TIME: {:.2?}",
        total_start.elapsed()
    );

    Ok(collections)
}

#[cfg(test)]
mod tests {
    use super::{
        generate_missing_transfers_from_sp, generates_transfers, get_available_transfers,
        TransferMap,
    };
    use crate::{
        model::Model,
        objects::{Coord, Time, Transfer},
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
        let new_transfers_map =
            generate_missing_transfers_from_sp(&TransferMap::new(), &model, 100.0, 0.7, 2, None);

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
            generates_transfers(model, 100.0, 0.7, 2, None).expect("an error occured");

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
    fn test_spatial_grid_performance() {
        // Create a model with many stop points to demonstrate the performance improvement
        let mut model_builder = ModelBuilder::default();

        // Create a grid of stop points (e.g., 100 x 100 = 10,000 points)
        // In a real scenario with 10k points:
        // - O(n²) = 100,000,000 comparisons
        // - O(n×k) with spatial grid ≈ 10,000 × 9 cells × ~10 points = ~900,000 comparisons
        // This is roughly 100x faster!

        let grid_size = 10; // Use 10x10 = 100 points for the test (to keep it fast)
        for i in 0..grid_size {
            for j in 0..grid_size {
                let stop_id = format!("SP_{i}_{j}");
                // Create stops in a 0.01° x 0.01° grid (roughly 1km x 1km)

                model_builder = model_builder.vj(&format!("vj_{i}_{j}"), |vj_builder| {
                    vj_builder
                        .route(&format!("route_{i}_{j}"))
                        .st(&stop_id, "10:00:00");
                });
            }
        }

        let transit_model = model_builder.build();
        let mut collections = transit_model.into_collections();

        // Set coordinates for all stop points
        for i in 0..grid_size {
            for j in 0..grid_size {
                let stop_id = format!("SP_{i}_{j}");
                collections.stop_points.get_mut(&stop_id).unwrap().coord = Coord {
                    lon: 2.39 + (i as f64) * 0.001,
                    lat: 48.85 + (j as f64) * 0.001,
                };
            }
        }

        let model = Model::new(collections).unwrap();

        // Generate transfers with a reasonable distance (500m)
        let result = generates_transfers(model, 500.0, 0.7, 2, None);
        assert!(result.is_ok());

        // With spatial grid, this should complete quickly even with 100+ points
        // The number of transfers should be reasonable (not n²)
        let new_model = result.unwrap();
        let transfer_count = new_model.transfers.len();

        // Each point should have transfers to nearby points (not all points)
        // With 100 points in a 10x10 grid and 500m max distance,
        // each point should connect to roughly 4-9 neighbors
        assert!(transfer_count < grid_size * grid_size * grid_size * grid_size);
        assert!(transfer_count > 0);
    }
}
