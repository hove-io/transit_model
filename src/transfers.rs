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
    objects::{Coord, Pathway, PhysicalMode, StopLocation, StopPoint, Transfer},
    physical_modes_utils::build_stop_point_physical_mode_map,
    report::{Report, TransferReportCategory},
    Result, TRANSFER_MANHATTAN_FACTOR, TRANSFER_MAX_DISTANCE, TRANSFER_WAITING_TIME,
    TRANSFER_WALKING_SPEED,
};
use rstar::{RTree, RTreeObject, AABB};
use rust_decimal::prelude::ToPrimitive;
use std::collections::HashMap;
use std::time::Instant;
use tracing::info;
use typed_index_collection::{Collection, CollectionWithId, Idx};

///structure for indexing transfers
pub type TransferMap = HashMap<(Idx<StopPoint>, Idx<StopPoint>), Transfer>;

/// The closure that will determine whether a connection should be created between 2 stops.
/// See [generates_transfers](./fn.generates_transfers.html).
pub type NeedTransfer<'a> = Box<dyn 'a + Fn(&Model, Idx<StopPoint>, Idx<StopPoint>) -> bool>;

/// Structure to determine the waiting time for a transfer between 2 physical modes.
pub type WaitingTimesByModes = HashMap<(Idx<PhysicalMode>, Idx<PhysicalMode>), u32>;

/// Configuration for transfer generation.
pub struct TransfersConfiguration<'a> {
    /// Maximum total walking distance in meters to consider generating a transfer.
    /// This includes both open-air segments (crow-fly × manhattan_factor) and
    /// indoor pathway segments (like entrances).
    pub max_distance: f64,
    /// Walking speed in meters per second, used to estimate transfer times.
    pub walking_speed: f64,
    /// Waiting time in seconds added to the transfer time at stop.
    pub waiting_time: u32,
    /// Factor applied to the crow-fly distance to compute the manhattan distance.
    pub manhattan_factor: f64,
    /// Additional condition that determines whether a transfer must be created between 2 stop points.
    pub need_transfer: Option<NeedTransfer<'a>>,
    /// Specific waiting time in seconds for each pair of physical modes, used instead of the default `waiting_time` if specified.
    pub waiting_time_by_modes: Option<WaitingTimesByModes>,
}

impl Default for TransfersConfiguration<'_> {
    fn default() -> Self {
        Self {
            max_distance: TRANSFER_MAX_DISTANCE,
            walking_speed: TRANSFER_WALKING_SPEED,
            waiting_time: TRANSFER_WAITING_TIME,
            manhattan_factor: TRANSFER_MANHATTAN_FACTOR,
            need_transfer: None,
            waiting_time_by_modes: None,
        }
    }
}

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

/// Maps each StopPoint to its best pathway (distance, time) to/from each reachable StopLocation.
/// Distance is in meters, time is in seconds.
type PathwayMap = HashMap<Idx<StopPoint>, HashMap<Idx<StopLocation>, (f64, f64)>>;

/// Returns the (distance, time) of a pathway.
/// Distance: `length` if available, otherwise `traversal_time * walking_speed`.
/// Time: `traversal_time` if available, otherwise `length / walking_speed`.
/// Returns `None` if neither `length` nor `traversal_time` is populated.
fn pathway_distance_and_time(pathway: &Pathway, walking_speed: f64) -> Option<(f64, f64)> {
    // We can consider that pathway length is already in Manhattan distance
    let distance = pathway
        .length
        .as_ref()
        .and_then(|length| length.to_f64())
        .or_else(|| {
            pathway
                .traversal_time
                .map(|seconds| f64::from(seconds) * walking_speed)
        })?;
    let time = pathway
        .traversal_time
        .map(f64::from)
        .unwrap_or_else(|| distance / walking_speed);
    Some((distance, time))
}

/// For a given stop-point → stop-location pathway, keeps only the shortest distance
/// (and its associated time) in the map.
/// It's rare to have multiple pathways from the same stop-point to the same stop-location, but it's possible.
fn insert_min_pathway(
    maps: &mut PathwayMap,
    stop_point_idx: Idx<StopPoint>,
    stop_location_idx: Idx<StopLocation>,
    distance: f64,
    time: f64,
) {
    let stop_location_map = maps.entry(stop_point_idx).or_default();
    let (current_distance, current_time) = stop_location_map
        .entry(stop_location_idx)
        .or_insert((f64::MAX, f64::MAX));
    if distance < *current_distance {
        *current_distance = distance;
        *current_time = time;
    }
}

/// Pre-computes, for every stop-point that has pathways, the minimum pathway
/// (distance, time) to each reachable stop-location, separated by direction (exit / entry).
/// Pathways whose distance alone exceeds `max_distance` are discarded upfront.
///
/// Returns `(exit_maps, entry_maps)` where each is indexed by `Idx<StopPoint>`
/// and contains a `HashMap<Idx<StopLocation>, (f64, f64)>` of best (distance, time) pairs.
fn build_pathway_maps(
    model: &Model,
    walking_speed: f64,
    max_distance: f64,
) -> (PathwayMap, PathwayMap) {
    let mut exit_maps = PathwayMap::new();
    let mut entry_maps = PathwayMap::new();

    for pathway in model.pathways.values() {
        let (distance, time) = match pathway_distance_and_time(pathway, walking_speed) {
            Some((distance, _)) if distance > max_distance => continue,
            Some((distance, time)) => (distance, time),
            None => continue,
        };

        // Case: from=SP, to=SL
        if let Some(stop_point_idx) = model.stop_points.get_idx(&pathway.from_stop_id) {
            if let Some(stop_location_idx) = model.stop_locations.get_idx(&pathway.to_stop_id) {
                // SP → SL = exit
                insert_min_pathway(
                    &mut exit_maps,
                    stop_point_idx,
                    stop_location_idx,
                    distance,
                    time,
                );
                if pathway.is_bidirectional {
                    // SP ← SL = entry
                    insert_min_pathway(
                        &mut entry_maps,
                        stop_point_idx,
                        stop_location_idx,
                        distance,
                        time,
                    );
                }
            }
        }

        // Case: from=SL, to=SP
        if let Some(stop_point_idx) = model.stop_points.get_idx(&pathway.to_stop_id) {
            if let Some(stop_location_idx) = model.stop_locations.get_idx(&pathway.from_stop_id) {
                // SL → SP = entry
                insert_min_pathway(
                    &mut entry_maps,
                    stop_point_idx,
                    stop_location_idx,
                    distance,
                    time,
                );
                if pathway.is_bidirectional {
                    // SL ← SP = exit
                    insert_min_pathway(
                        &mut exit_maps,
                        stop_point_idx,
                        stop_location_idx,
                        distance,
                        time,
                    );
                }
            }
        }
    }

    (exit_maps, entry_maps)
}

/// Computes the transfer distance (meters) and time (seconds) between two stop points.
///
/// Among all paths whose total distance ≤ max_distance, selects the fastest one.
/// Crow-fly distances are converted to approximate Manhattan distances.
/// Time uses actual pathway traversal times for pathway segments and
/// adjusted crow-fly / walking_speed for open-air segments.
///
/// Returns `(distance_in_meters, time_in_seconds)`.
/// When pathways exist but no valid path is found within max_distance,
/// returns `(f64::MAX, 0)` to signal that no transfer should be created.
///
/// The 4 cases handled:
/// 1. Same stop_area or no pathways → crow-fly × factor (manhattan)
/// 2. Both sides have pathways → fastest valid SP1→exit→(manhattan)→entry→SP2
/// 3. Only SP1 has pathways → SP1→exit→(manhattan)→SP2
/// 4. Only SP2 has pathways → SP1→(manhattan)→entry→SP2
fn compute_transfer_time(
    model: &Model,
    sp1: &StopPoint,
    sp2: &StopPoint,
    sp1_exit_map: &HashMap<Idx<StopLocation>, (f64, f64)>,
    sp2_entry_map: &HashMap<Idx<StopLocation>, (f64, f64)>,
    sp1_sp2_manhattan_distance: f64,
    config: &TransfersConfiguration,
) -> (f64, u32) {
    if sp1.stop_area_id == sp2.stop_area_id || (sp1_exit_map.is_empty() && sp2_entry_map.is_empty())
    {
        return (
            sp1_sp2_manhattan_distance,
            (sp1_sp2_manhattan_distance / config.walking_speed) as u32,
        );
    }

    let mut min_distance = f64::MAX;
    let mut min_time = f64::MAX;

    if !sp1_exit_map.is_empty() && !sp2_entry_map.is_empty() {
        // Try all (exit, entry) combinations with crow-fly × factor (manhattan) between them.
        // When exit == entry (shared stop-location), between distance is 0.
        for (&exit_sl_idx, &(exit_distance, exit_time)) in sp1_exit_map {
            let exit_coord = &model.stop_locations[exit_sl_idx].coord;
            for (&entry_sl_idx, &(entry_distance, entry_time)) in sp2_entry_map {
                let entry_coord = &model.stop_locations[entry_sl_idx].coord;
                let between_dist = exit_coord.distance_to(entry_coord) * config.manhattan_factor;
                let total_dist = exit_distance + between_dist + entry_distance;
                if total_dist > config.max_distance {
                    continue;
                }
                let total_time = exit_time + (between_dist / config.walking_speed) + entry_time;
                if total_time < min_time {
                    min_time = total_time;
                    min_distance = total_dist;
                }
            }
        }
    } else if !sp1_exit_map.is_empty() {
        // SP1 has exits, SP2 has no pathways: SP1 → exit → crow-fly × factor (manhattan) → SP2
        for (&exit_sl_idx, &(exit_distance, exit_time)) in sp1_exit_map {
            let exit_coord = &model.stop_locations[exit_sl_idx].coord;
            let between_dist = exit_coord.distance_to(&sp2.coord) * config.manhattan_factor;
            let total_dist = exit_distance + between_dist;
            if total_dist > config.max_distance {
                continue;
            }
            let total_time = exit_time + (between_dist / config.walking_speed);
            if total_time < min_time {
                min_time = total_time;
                min_distance = total_dist;
            }
        }
    } else {
        // SP1 has no pathways, SP2 has entries: SP1 → crow-fly × factor (manhattan) → entry → SP2
        for (&entry_sl_idx, &(entry_distance, entry_time)) in sp2_entry_map {
            let entry_coord = &model.stop_locations[entry_sl_idx].coord;
            let between_dist = sp1.coord.distance_to(entry_coord) * config.manhattan_factor;
            let total_dist = between_dist + entry_distance;
            if total_dist > config.max_distance {
                continue;
            }
            let total_time = (between_dist / config.walking_speed) + entry_time;
            if total_time < min_time {
                min_time = total_time;
                min_distance = total_dist;
            }
        }
    }

    if min_time < f64::MAX {
        (min_distance, min_time as u32)
    } else {
        // Pathways exist but no valid path within max_distance: no transfer
        (f64::MAX, 0)
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
    config: &TransfersConfiguration,
    report_opt: Option<&mut Report<TransferReportCategory>>,
) -> TransferMap {
    info!("Adding missing transfers from stop points.");
    let bench_start = Instant::now();

    let mut default_report = Report::default();
    let report = report_opt.unwrap_or(&mut default_report);

    let mut new_transfers_map = TransferMap::new();
    let sq_max_crow_fly_distance = (config.max_distance / config.manhattan_factor).powi(2);

    // Build R-tree for efficient spatial queries
    let stop_locations: Vec<StopPointLocation> = model
        .stop_points
        .iter()
        .filter(|(_, sp)| sp.coord != Coord::default())
        .map(|(idx, sp)| StopPointLocation::new(idx, sp.coord))
        .collect();

    let rtree = RTree::bulk_load(stop_locations);

    let stop_point_physical_mode_map = config
        .waiting_time_by_modes
        .is_some()
        .then(|| build_stop_point_physical_mode_map(model));

    let (sp_exit_maps, sp_entry_maps) =
        build_pathway_maps(model, config.walking_speed, config.max_distance);
    let empty_pathway_map = HashMap::new();

    // Debug counters for transfer cases
    let mut case1_count: u32 = 0; // Same stop_area or no pathways
    let mut case2_count: u32 = 0; // Both sides have pathways
    let mut case3_count: u32 = 0; // Only SP1 has pathways
    let mut case4_count: u32 = 0; // Only SP2 has pathways

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
        let max_crow_fly_distance = config.max_distance / config.manhattan_factor;
        let search_distance_lat = max_crow_fly_distance / 111_000.0;
        // For longitude, use the pre-calculated approx (cos of latitude) to get the correct degree distance
        let search_distance_lon = max_crow_fly_distance / (111_000.0 * approx.cos_lat());
        let min_lon = sp1.coord.lon - search_distance_lon;
        let max_lon = sp1.coord.lon + search_distance_lon;
        let min_lat = sp1.coord.lat - search_distance_lat;
        let max_lat = sp1.coord.lat + search_distance_lat;

        let search_box = AABB::from_corners([min_lon, min_lat], [max_lon, max_lat]);

        let sp1_mode_idx = stop_point_physical_mode_map
            .as_ref()
            .and_then(|map| map.get(&idx1));

        let sp1_exit_map = sp_exit_maps.get(&idx1).unwrap_or(&empty_pathway_map);

        // Get all points within the bounding box and filter by actual distance
        for nearby_location in rtree.locate_in_envelope(&search_box) {
            let idx2 = nearby_location.idx;

            if transfers_map.contains_key(&(idx1, idx2)) {
                continue;
            }
            if let Some(ref f) = config.need_transfer {
                if !f(model, idx1, idx2) {
                    continue;
                }
            }

            // Use the pre-calculated approximation for efficient distance calculation
            let sq_distance = approx.sq_distance_to(&nearby_location.coord);
            if sq_distance > sq_max_crow_fly_distance {
                continue;
            }

            let sp1_sp2_manhattan_distance = sq_distance.sqrt() * config.manhattan_factor;
            let sp2 = &model.stop_points[idx2];
            let sp2_entry_map = sp_entry_maps.get(&idx2).unwrap_or(&empty_pathway_map);

            let (transfer_manhattan_distance, transfer_time) = compute_transfer_time(
                model,
                sp1,
                sp2,
                sp1_exit_map,
                sp2_entry_map,
                sp1_sp2_manhattan_distance,
                config,
            );

            if transfer_manhattan_distance > config.max_distance {
                continue;
            }

            // Use the specific waiting time for this pair of physical modes, or fall back to the default waiting time if not found
            let specific_waiting_time = sp1_mode_idx
                .and_then(|mode_idx1| {
                    let mode_idx2 = stop_point_physical_mode_map.as_ref()?.get(&idx2)?;
                    config
                        .waiting_time_by_modes
                        .as_ref()?
                        .get(&(*mode_idx1, *mode_idx2))
                        .copied()
                })
                .unwrap_or(config.waiting_time);

            // Track which case was used for this transfer
            if sp1.stop_area_id == sp2.stop_area_id
                || (sp1_exit_map.is_empty() && sp2_entry_map.is_empty())
            {
                case1_count += 1;
            } else if !sp1_exit_map.is_empty() && !sp2_entry_map.is_empty() {
                case2_count += 1;
            } else if !sp1_exit_map.is_empty() {
                case3_count += 1;
            } else {
                case4_count += 1;
            }

            report.add_info(
                format!("Created transfer from {} to {}", sp1.id, sp2.id),
                TransferReportCategory::Created,
            );

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

    info!(
        "Generate missing-transfers completed in {:.2?}: {} transfers | \
         Case 1 (same area/no pathways): {} | \
         Case 2 (both have pathways): {} | \
         Case 3 (only SP1 has exit pathways): {} | \
         Case 4 (only SP2 has entry pathways): {}",
        bench_start.elapsed(),
        new_transfers_map.len(),
        case1_count,
        case2_count,
        case3_count,
        case4_count,
    );

    new_transfers_map
}

/// Generates transfers between stop points and returns the updated collections.
///
/// This function merges existing transfers with newly generated ones:
/// 1. Existing transfers (from `model.transfers`) are preserved as-is.
/// 2. Missing transfers are generated for pairs of stop points within
///    [`TransfersConfiguration::max_distance`], using a spatial R-tree index
///    for efficiency.
///
/// Transfer times are computed using [`compute_transfer_time`], which accounts
/// for indoor pathway segments (entrances/exits) when available, falling back
/// to crow-fly × manhattan factor for open-air segments.
/// Walking time for open-air segments is derived from the distance divided by
/// [`TransfersConfiguration::walking_speed`].
///
/// A fixed waiting time ([`TransfersConfiguration::waiting_time`]) is added on top
/// of the computed walking time to form the `real_min_transfer_time`. This can be
/// overridden per pair of physical modes via [`WaitingTimesByModes`].
///
/// Stop points with coordinates at `(0, 0)` are considered invalid and are
/// skipped (no transfer is generated to or from them).
///
/// An optional [`NeedTransfer`] closure in the configuration allows adding
/// custom filtering logic (e.g. only connect stop points from different stop areas).
///
/// Returns the model's [`Collections`] with the `transfers` field replaced by
/// the full merged and sorted transfer list.
///
/// # Example
///
/// Given the following existing transfers and a `max_distance` of 500m:
///
/// | from_stop_id | to_stop_id | min_transfer_time |                                                         |
/// | ------------ | ---------- | ----------------- | ------------------------------------------------------- |
/// | SP1          | SP2        | 120               | existing transfer, preserved as-is                      |
/// | SP3          | SP4        | (generated)       | missing transfer within range, generated from crow-fly  |
/// | SP5          | SP6        | (skipped)         | distance exceeds `max_distance`, no transfer created    |
/// | UNKNOWN      | SP2        | 180               | stop `UNKNOWN` not found, transfer ignored              |
pub fn generates_transfers(
    model: Model,
    config: TransfersConfiguration,
    report_opt: Option<&mut Report<TransferReportCategory>>,
) -> Result<Collections> {
    info!("Generating transfers...");

    let mut transfers_map = get_available_transfers(model.transfers.clone(), &model.stop_points);
    let new_transfers_map =
        generate_missing_transfers_from_sp(&transfers_map, &model, &config, report_opt);

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
        TransferMap, TransfersConfiguration, WaitingTimesByModes,
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
        let config = TransfersConfiguration {
            max_distance: 120.0,
            walking_speed: 0.7,
            waiting_time: 2,
            ..Default::default()
        };
        let new_transfers_map =
            generate_missing_transfers_from_sp(&TransferMap::new(), &model, &config, None);

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
        let config = TransfersConfiguration {
            max_distance: 120.0,
            walking_speed: 0.7,
            waiting_time: 2,
            ..Default::default()
        };
        let mut collections = generates_transfers(model, config, None).expect("an error occured");

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

        let config = TransfersConfiguration {
            max_distance: 530.0,
            walking_speed: 1.0,
            waiting_time: 10,
            ..Default::default()
        };

        let model = generates_transfers(model, config, None).unwrap();

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

        let config = TransfersConfiguration {
            max_distance: 500.0,
            walking_speed: 0.785,
            waiting_time: default_waiting_time,
            waiting_time_by_modes: Some(waiting_time_by_modes),
            ..Default::default()
        };

        let collections = generates_transfers(model, config, None).expect("an error occurred");

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

    mod pathway_distance_and_time_tests {
        use super::super::pathway_distance_and_time;
        use crate::objects::Pathway;
        use rust_decimal::Decimal;
        use std::str::FromStr;

        fn make_pathway(length: Option<&str>, traversal_time: Option<u32>) -> Pathway {
            Pathway {
                id: "pw_test".to_string(),
                length: length.map(|l| Decimal::from_str(l).unwrap()),
                traversal_time,
                ..Default::default()
            }
        }

        #[test]
        fn both_length_and_traversal_time() {
            // distance comes from length, time comes from traversal_time
            let pathway = make_pathway(Some("100.0"), Some(60));
            let (distance, time) = pathway_distance_and_time(&pathway, 1.0).unwrap();
            assert_eq!(distance, 100.0);
            assert_eq!(time, 60.0);
        }

        #[test]
        fn length_only_derives_time_from_speed() {
            let pathway = make_pathway(Some("200.0"), None);
            let (distance, time) = pathway_distance_and_time(&pathway, 2.0).unwrap();
            assert_eq!(distance, 200.0);
            assert_eq!(time, 100.0);
        }

        #[test]
        fn traversal_time_only_derives_distance_from_speed() {
            let pathway = make_pathway(None, Some(90));
            let (distance, time) = pathway_distance_and_time(&pathway, 1.5).unwrap();
            assert_eq!(distance, 135.0);
            assert_eq!(time, 90.0);
        }

        #[test]
        fn neither_returns_none() {
            let pathway = make_pathway(None, None);
            assert!(pathway_distance_and_time(&pathway, 1.0).is_none());
        }
    }

    mod build_pathway_maps_tests {
        use super::super::build_pathway_maps;
        use crate::{
            model::Model,
            objects::{Coord, StopType},
            ModelBuilder,
        };

        fn base_model() -> ModelBuilder {
            ModelBuilder::default()
                .stop_area("sa1", |_| {})
                .stop_area("sa2", |_| {})
                .stop_point("SP_A", |sp| {
                    sp.coord = Coord::from(("2.3800".to_string(), "48.8500".to_string()));
                    sp.stop_area_id = "sa1".to_string();
                })
                .stop_point("SP_B", |sp| {
                    sp.coord = Coord::from(("2.3810".to_string(), "48.8500".to_string()));
                    sp.stop_area_id = "sa2".to_string();
                })
                .stop_location("SL_X", |sl| {
                    sl.coord = Coord::from(("2.3805".to_string(), "48.8500".to_string()));
                    sl.parent_id = Some("sa1".to_string());
                })
                .stop_location("SL_Y", |sl| {
                    sl.coord = Coord::from(("2.3808".to_string(), "48.8500".to_string()));
                    sl.parent_id = Some("sa2".to_string());
                })
                .vj("vj1", |vj| {
                    vj.st("SP_A", "10:00:00").st("SP_B", "10:10:00");
                })
        }

        #[test]
        fn unidirectional_sp_to_sl_creates_exit_only() {
            let model = base_model()
                .pathway(
                    "SP_A",
                    StopType::Point,
                    "SL_X",
                    StopType::StopEntrance,
                    100,
                    60,
                    |pw| {
                        pw.is_bidirectional = false;
                    },
                )
                .build();

            let (exit_maps, entry_maps) = build_pathway_maps(&model, 1.0, 500.0);
            assert_eq!(exit_maps.len(), 1);
            assert_eq!(entry_maps.len(), 0);

            let sp_a = model.stop_points.get_idx("SP_A").unwrap();
            let sl_x = model.stop_locations.get_idx("SL_X").unwrap();

            let (distance, time) = exit_maps[&sp_a][&sl_x];
            assert_eq!(distance, 100.0);
            assert_eq!(time, 60.0);
        }

        #[test]
        fn bidirectional_sp_to_sl_creates_exit_and_entry() {
            let model = base_model()
                .pathway(
                    "SP_A",
                    StopType::Point,
                    "SL_X",
                    StopType::StopEntrance,
                    100,
                    60,
                    |_| {},
                )
                .build();

            let (exit_maps, entry_maps) = build_pathway_maps(&model, 1.0, 500.0);
            assert_eq!(exit_maps.len(), 1);
            assert_eq!(entry_maps.len(), 1);

            let sp_a = model.stop_points.get_idx("SP_A").unwrap();
            let sl_x = model.stop_locations.get_idx("SL_X").unwrap();

            let (distance, time) = exit_maps[&sp_a][&sl_x];
            assert_eq!(distance, 100.0);
            assert_eq!(time, 60.0);

            let (distance, time) = entry_maps[&sp_a][&sl_x];
            assert_eq!(distance, 100.0);
            assert_eq!(time, 60.0);
        }

        #[test]
        fn unidirectional_sl_to_sp_creates_entry_only() {
            let model = base_model()
                .pathway(
                    "SL_Y",
                    StopType::StopEntrance,
                    "SP_B",
                    StopType::Point,
                    80,
                    50,
                    |pw| {
                        pw.is_bidirectional = false;
                    },
                )
                .build();

            let (exit_maps, entry_maps) = build_pathway_maps(&model, 1.0, 500.0);
            assert_eq!(exit_maps.len(), 0);
            assert_eq!(entry_maps.len(), 1);

            let sp_b = model.stop_points.get_idx("SP_B").unwrap();
            let sl_y = model.stop_locations.get_idx("SL_Y").unwrap();

            let (distance, time) = entry_maps[&sp_b][&sl_y];
            assert_eq!(distance, 80.0);
            assert_eq!(time, 50.0);
        }

        #[test]
        fn bidirectional_sl_to_sp_creates_exit_and_entry() {
            let model = base_model()
                .pathway(
                    "SL_Y",
                    StopType::StopEntrance,
                    "SP_B",
                    StopType::Point,
                    80,
                    50,
                    |_| {},
                )
                .build();

            let (exit_maps, entry_maps) = build_pathway_maps(&model, 1.0, 500.0);
            assert_eq!(exit_maps.len(), 1);
            assert_eq!(entry_maps.len(), 1);

            let sp_b = model.stop_points.get_idx("SP_B").unwrap();
            let sl_y = model.stop_locations.get_idx("SL_Y").unwrap();

            let (distance, time) = exit_maps[&sp_b][&sl_y];
            assert_eq!(distance, 80.0);
            assert_eq!(time, 50.0);

            let (distance, time) = entry_maps[&sp_b][&sl_y];
            assert_eq!(distance, 80.0);
            assert_eq!(time, 50.0);
        }

        #[test]
        fn pathway_exceeding_max_distance_is_filtered() {
            let model = base_model()
                .pathway(
                    "SP_A",
                    StopType::Point,
                    "SL_X",
                    StopType::StopEntrance,
                    600,
                    300,
                    |pw| {
                        pw.is_bidirectional = false;
                    },
                )
                .build();

            let (exit_maps, entry_maps) = build_pathway_maps(&model, 1.0, 500.0);
            assert_eq!(exit_maps.len(), 0);
            assert_eq!(entry_maps.len(), 0);
        }

        #[test]
        fn keeps_shortest_distance_among_duplicate_pathways() {
            let model = base_model()
                // Long pathway: 200m, 90s
                .pathway(
                    "SP_A",
                    StopType::Point,
                    "SL_X",
                    StopType::StopEntrance,
                    200,
                    90,
                    |pw| {
                        pw.is_bidirectional = false;
                    },
                )
                .build();

            // The builder doesn't allow duplicate IDs, so we add the second via collections
            // Short pathway: 10m, 30s
            let mut collections = model.into_collections();
            collections
                .pathways
                .push(crate::objects::Pathway {
                    id: "SP_A:SL_X:short".to_string(),
                    from_stop_id: "SP_A".to_string(),
                    from_stop_type: StopType::Point,
                    to_stop_id: "SL_X".to_string(),
                    to_stop_type: StopType::StopEntrance,
                    is_bidirectional: false,
                    length: Some(10u32.into()),
                    traversal_time: Some(30),
                    ..Default::default()
                })
                .unwrap();
            let model = Model::new(collections).unwrap();
            assert_eq!(model.pathways.len(), 2);

            let (exit_maps, entry_maps) = build_pathway_maps(&model, 1.0, 500.0);
            assert_eq!(exit_maps.len(), 1);
            assert_eq!(entry_maps.len(), 0);

            let sp_a = model.stop_points.get_idx("SP_A").unwrap();
            let sl_x = model.stop_locations.get_idx("SL_X").unwrap();

            let (distance, time) = exit_maps[&sp_a][&sl_x];
            assert_eq!(distance, 10.0);
            assert_eq!(time, 30.0);
        }

        #[test]
        fn no_pathways_produces_empty_maps() {
            let model = base_model().build();
            let (exit_maps, entry_maps) = build_pathway_maps(&model, 1.0, 500.0);
            assert!(exit_maps.is_empty());
            assert!(entry_maps.is_empty());
        }
    }
}
