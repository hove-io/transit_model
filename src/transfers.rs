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

/// Identifies which of the 5 computation strategies was used by [`compute_transfer_time`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransferCase {
    /// Case 1: Both stop points belong to the same stop area — crow-fly only.
    SameArea,
    /// Case 2: Neither stop point has pathways — crow-fly only.
    NoPathways,
    /// Case 3: Only SP1 has exit pathway → SP1 → exit → crow-fly → SP2.
    OnlySp1HasExitPathways,
    /// Case 4: Only SP2 has entry pathway → SP1 → crow-fly → entry → SP2.
    OnlySp2HasEntryPathways,
    /// Case 5: Both stop points have pathways → SP1 → exit → crow-fly → entry → SP2.
    /// Exit and entry can be similar (shared stop location) or different.
    BothHavePathways,
}

impl std::fmt::Display for TransferCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransferCase::SameArea => write!(f, "case 1: same stop-area"),
            TransferCase::NoPathways => write!(f, "case 2: different stop-areas, but no pathway"),
            TransferCase::OnlySp1HasExitPathways => {
                write!(f, "case 3: only from-stop has exit pathway")
            }
            TransferCase::OnlySp2HasEntryPathways => {
                write!(f, "case 4: only to-stop has entry pathway")
            }
            TransferCase::BothHavePathways => {
                write!(f, "case 5: from-stop and to-stop have pathways")
            }
        }
    }
}

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
/// Transfers referencing an unknown stop point are silently ignored.
pub fn get_available_transfers(
    transfers: Collection<Transfer>,
    sp: &CollectionWithId<StopPoint>,
) -> TransferMap {
    transfers
        .into_iter()
        .filter_map(|t| {
            let from_idx = sp.get_idx(&t.from_stop_id)?;
            let to_idx = sp.get_idx(&t.to_stop_id)?;
            Some(((from_idx, to_idx), t))
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
    maps.entry(stop_point_idx)
        .or_default()
        .entry(stop_location_idx)
        .and_modify(|(current_distance, current_time)| {
            if distance < *current_distance {
                *current_distance = distance;
                *current_time = time;
            }
        })
        .or_insert((distance, time));
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
        let Some((distance, time)) =
            pathway_distance_and_time(pathway, walking_speed).filter(|&(d, _)| d <= max_distance)
        else {
            continue;
        };

        // Build the list of directions to process.
        // A unidirectional pathway only goes from → to.
        // A bidirectional pathway also applies in the reverse direction (to → from).
        let mut directions = vec![(&pathway.from_stop_id, &pathway.to_stop_id)];

        if pathway.is_bidirectional {
            directions.push((&pathway.to_stop_id, &pathway.from_stop_id));
        }

        for (from_id, to_id) in directions {
            let sp_from = model.stop_points.get_idx(from_id);
            let sl_to = model.stop_locations.get_idx(to_id);

            let sl_from = model.stop_locations.get_idx(from_id);
            let sp_to = model.stop_points.get_idx(to_id);

            // SP → SL : the traveller exits the stop point through a stop location (exit).
            if let (Some(sp_idx), Some(sl_idx)) = (sp_from, sl_to) {
                insert_min_pathway(&mut exit_maps, sp_idx, sl_idx, distance, time);
            }

            // SL → SP : the traveller enters the stop point through a stop location (entry).
            if let (Some(sl_idx), Some(sp_idx)) = (sl_from, sp_to) {
                insert_min_pathway(&mut entry_maps, sp_idx, sl_idx, distance, time);
            }
        }
    }

    (exit_maps, entry_maps)
}

/// Computes the estimated transfer time (seconds) between two stop points.
///
/// Among all possible paths where the total distance is ≤ `max_distance`,
/// this function selects the fastest one. Crow-fly distances are converted
/// to approximate Manhattan distances using a configured factor.
///
/// The time calculation combines actual traversal times for pathway segments
/// and estimated walking time (based on `walking_speed`) for open-air segments.
///
/// # Returns
/// Returns `Some((time_in_seconds, transfer_case))` if a valid path is found within
/// the `max_distance` threshold. Otherwise, returns `None`.
///
/// The distance is intentionally **not** part of the return value: the `max_distance`
/// filtering is handled internally — `None` signals that no valid path exists.
/// The caller therefore has no use for the distance; only the time is needed to build
/// the [`Transfer`] object.
///
/// - `time_in_seconds`: Total estimated walking time along the best path.
/// - `transfer_case`: Identifies which strategy was used (see [`TransferCase`]).
///
/// # The 5 handled cases:
/// 1. Same `stop_area` -> Direct Manhattan distance between SP1 and SP2.
/// 2. No pathways on either side -> Direct Manhattan distance between SP1 and SP2.
/// 3. Exit pathways for SP1 only -> SP1 =[pathway]=> Exit --[Manhattan]--> SP2.
/// 4. Entry pathways for SP2 only -> SP1 --[Manhattan]--> Entry =[pathway]=> SP2.
/// 5. Pathways on both sides -> SP1 =[pathway]=> Exit --[Manhattan]--> Entry =[pathway]=> SP2.
fn compute_transfer_time(
    model: &Model,
    sp1: &StopPoint,
    sp2: &StopPoint,
    sp1_exit_map: &HashMap<Idx<StopLocation>, (f64, f64)>,
    sp2_entry_map: &HashMap<Idx<StopLocation>, (f64, f64)>,
    sp1_sp2_manhattan_distance: f64,
    config: &TransfersConfiguration,
) -> Option<(u32, TransferCase)> {
    if sp1.stop_area_id == sp2.stop_area_id {
        return Some((
            (sp1_sp2_manhattan_distance / config.walking_speed) as u32,
            TransferCase::SameArea,
        ));
    }

    if sp1_exit_map.is_empty() && sp2_entry_map.is_empty() {
        return Some((
            (sp1_sp2_manhattan_distance / config.walking_speed) as u32,
            TransferCase::NoPathways,
        ));
    }

    let case = match (!sp1_exit_map.is_empty(), !sp2_entry_map.is_empty()) {
        (true, true) => TransferCase::BothHavePathways,
        (true, false) => TransferCase::OnlySp1HasExitPathways,
        _ => TransferCase::OnlySp2HasEntryPathways,
    };

    let exits: Vec<(&Coord, f64, f64)> = if sp1_exit_map.is_empty() {
        vec![(&sp1.coord, 0.0, 0.0)]
    } else {
        sp1_exit_map
            .iter()
            .map(|(idx, &(d, t))| (&model.stop_locations[*idx].coord, d, t))
            .collect()
    };

    let entries: Vec<(&Coord, f64, f64)> = if sp2_entry_map.is_empty() {
        vec![(&sp2.coord, 0.0, 0.0)]
    } else {
        sp2_entry_map
            .iter()
            .map(|(idx, &(d, t))| (&model.stop_locations[*idx].coord, d, t))
            .collect()
    };

    let mut best: Option<f64> = None;

    for &(exit_coord, exit_dist, exit_time) in &exits {
        for &(entry_coord, entry_dist, entry_time) in &entries {
            let between_dist = exit_coord.distance_to(entry_coord) * config.manhattan_factor;
            let total_dist = exit_dist + between_dist + entry_dist;

            if total_dist > config.max_distance {
                continue;
            }

            let total_time = exit_time + (between_dist / config.walking_speed) + entry_time;

            if best.is_none_or(|min_t| total_time < min_t) {
                best = Some(total_time);
            }
        }
    }

    best.map(|t| (t as u32, case))
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

    let max_crow_fly_distance = config.max_distance / config.manhattan_factor;
    let sq_max_crow_fly_distance = max_crow_fly_distance.powi(2);

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
    let mut case1_count: u32 = 0; // Same stop-area
    let mut case2_count: u32 = 0; // No pathway on either side
    let mut case3_count: u32 = 0; // Only SP1 (from-stop) has exit pathway
    let mut case4_count: u32 = 0; // Only SP2 (to-stop) has entry pathway
    let mut case5_count: u32 = 0; // Both sides have pathways

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

            let Some((transfer_time, transfer_case)) = compute_transfer_time(
                model,
                sp1,
                sp2,
                sp1_exit_map,
                sp2_entry_map,
                sp1_sp2_manhattan_distance,
                config,
            ) else {
                continue;
            };

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

            match transfer_case {
                TransferCase::SameArea => case1_count += 1,
                TransferCase::NoPathways => case2_count += 1,
                TransferCase::OnlySp1HasExitPathways => case3_count += 1,
                TransferCase::OnlySp2HasEntryPathways => case4_count += 1,
                TransferCase::BothHavePathways => case5_count += 1,
            }
            report.add_info(
                format!(
                    "Created transfer from stop '{}' to stop '{}' ({})",
                    sp1.id, sp2.id, transfer_case
                ),
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
         Case 1 (same stop-area): {} | \
         Case 2 (≠ stop-area, but no pathway): {} | \
         Case 3 (only SP1 has exit): {} | \
         Case 4 (only SP2 has entry): {} | \
         Case 5 (both have pathways): {}",
        bench_start.elapsed(),
        new_transfers_map.len(),
        case1_count,
        case2_count,
        case3_count,
        case4_count,
        case5_count,
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

    mod compute_transfer_time_tests {
        use super::super::{
            build_pathway_maps, compute_transfer_time, TransferCase, TransfersConfiguration,
        };
        use crate::{
            objects::{Coord, ObjectType, StopType},
            ModelBuilder,
        };
        use std::collections::HashMap;

        fn config() -> TransfersConfiguration<'static> {
            TransfersConfiguration {
                max_distance: 500.0,
                walking_speed: 1.5,
                manhattan_factor: 1.2,
                ..Default::default()
            }
        }

        /// Builds a base model with two stop areas, three stop points, and two stop locations.
        ///
        ///   SP_A(sa1) -51m- SP_A2(sa1) --110m-- SL_X(sa1) -66m- SL_Y(sa2) --124m-- SP_B(sa2)
        ///   2.3800          2.3807                2.3822          2.3831              2.3848
        fn base_model() -> ModelBuilder {
            ModelBuilder::default()
                .stop_area("sa1", |_| {})
                .stop_area("sa2", |_| {})
                .stop_point("SP_A", |sp| {
                    sp.coord = Coord::from(("2.3800".to_string(), "48.8500".to_string()));
                    sp.stop_area_id = "sa1".to_string();
                })
                .stop_point("SP_A2", |sp| {
                    sp.coord = Coord::from(("2.3807".to_string(), "48.8500".to_string()));
                    sp.stop_area_id = "sa1".to_string();
                })
                .stop_point("SP_B", |sp| {
                    sp.coord = Coord::from(("2.3848".to_string(), "48.8500".to_string()));
                    sp.stop_area_id = "sa2".to_string();
                })
                .stop_location("SL_X", |sl| {
                    sl.coord = Coord::from(("2.3822".to_string(), "48.8500".to_string()));
                    sl.parent_id = Some("sa1".to_string());
                })
                .stop_location("SL_Y", |sl| {
                    sl.coord = Coord::from(("2.3831".to_string(), "48.8500".to_string()));
                    sl.parent_id = Some("sa2".to_string());
                })
                .add_object_lock(&ObjectType::StopPoint, "SP_A2")
                .vj("vj1", |vj| {
                    vj.st("SP_A", "10:00:00").st("SP_B", "10:10:00");
                })
        }

        // --- Case 1 ---
        #[test]
        fn case1_same_stop_area_returns_crow_fly() {
            let model = base_model().build();
            let sp_a = model.stop_points.get("SP_A").unwrap();
            let sp_a2 = model.stop_points.get("SP_A2").unwrap();
            // Pass 60.0m manhattan (SP_A→SP_A2 ≈51m crow-fly × 1.2 ≈ 61m, rounded for clarity)
            let (time, case) = compute_transfer_time(
                &model,
                sp_a,
                sp_a2,
                &HashMap::new(),
                &HashMap::new(),
                60.0,
                &config(),
            )
            .unwrap();
            assert_eq!(case, TransferCase::SameArea);
            assert_eq!(time, 40); // 60m / 1.5 m/s
        }

        // --- Case 2 ---
        #[test]
        fn case2_no_pathways_returns_crow_fly() {
            let model = base_model().build();
            let sp_a = model.stop_points.get("SP_A").unwrap();
            let sp_b = model.stop_points.get("SP_B").unwrap();
            // Pass 420.0m manhattan (SP_A→SP_B ≈351m crow-fly × 1.2 ≈ 421m, rounded for clarity)
            let (time, case) = compute_transfer_time(
                &model,
                sp_a,
                sp_b,
                &HashMap::new(),
                &HashMap::new(),
                420.0,
                &config(),
            )
            .unwrap();
            assert_eq!(case, TransferCase::NoPathways);
            assert_eq!(time, 280); // 420m / 1.5 m/s
        }

        // --- Case 3 ---
        #[test]
        fn case3_only_sp1_has_exit_pathway() {
            // SP_A =[50m, 40s]=> SL_X --[crow-fly × 1.2]--> SP_B
            let model = base_model()
                .pathway(
                    "SP_A",
                    StopType::Point,
                    "SL_X",
                    StopType::StopEntrance,
                    50,
                    40,
                    |pw| {
                        pw.is_bidirectional = false;
                    },
                )
                .build();

            let (sp_exit_maps, _) =
                build_pathway_maps(&model, config().walking_speed, config().max_distance);

            let idx_a = model.stop_points.get_idx("SP_A").unwrap();
            let sp_a_exit_map = sp_exit_maps.get(&idx_a).unwrap();
            let sp_a = model.stop_points.get("SP_A").unwrap();
            let sp_b = model.stop_points.get("SP_B").unwrap();

            let (time, case) = compute_transfer_time(
                &model,
                sp_a,
                sp_b,
                sp_a_exit_map,
                &HashMap::new(),
                1000.0,
                &config(),
            )
            .unwrap();
            assert_eq!(case, TransferCase::OnlySp1HasExitPathways);
            // SP_A =[pathway: 50m, 40s]=> SL_X --[crow-fly × 1.2]--> SP_B
            // time:  40s (pathway)  +  228m / 1.5 m/s                 ≈ 192s total
            assert_eq!(time, 192);
        }

        #[test]
        fn case3_all_exits_exceed_max_distance_returns_no_transfer() {
            let model = base_model()
                .pathway(
                    "SP_A",
                    StopType::Point,
                    "SL_X",
                    StopType::StopEntrance,
                    300,
                    270,
                    |pw| {
                        pw.is_bidirectional = false;
                    },
                )
                .build();

            let (sp_exit_maps, _) =
                build_pathway_maps(&model, config().walking_speed, config().max_distance);
            let idx_a = model.stop_points.get_idx("SP_A").unwrap();
            let sp_a_exit_map = sp_exit_maps.get(&idx_a).unwrap();
            let sp_a = model.stop_points.get("SP_A").unwrap();
            let sp_b = model.stop_points.get("SP_B").unwrap();

            let result = compute_transfer_time(
                &model,
                sp_a,
                sp_b,
                sp_a_exit_map,
                &HashMap::new(),
                1000.0,
                &config(),
            );
            // SP_A =[pathway: 300m, 270s]=> SL_X --[crow-fly × 1.2]--> SP_B
            // dist:  300m (pathway) + ≈190m (crow-fly) × 1.2 = 228m  = ~528m total
            //        → exceeds max_distance (500m), so no valid path → None
            assert!(result.is_none());
        }

        // --- Case 4 ---
        #[test]
        fn case4_only_sp2_has_entry_pathway() {
            // SP_A --[crow-fly × 1.2]--> SL_Y =[50m, 40s]=> SP_B
            let model = base_model()
                .pathway(
                    "SL_Y",
                    StopType::StopEntrance,
                    "SP_B",
                    StopType::Point,
                    50,
                    40,
                    |pw| {
                        pw.is_bidirectional = false;
                    },
                )
                .build();

            let (_, sp_entry_maps) =
                build_pathway_maps(&model, config().walking_speed, config().max_distance);
            let idx_b = model.stop_points.get_idx("SP_B").unwrap();
            let sp_b_entry_map = sp_entry_maps.get(&idx_b).unwrap();
            let sp_a = model.stop_points.get("SP_A").unwrap();
            let sp_b = model.stop_points.get("SP_B").unwrap();

            let (time, case) = compute_transfer_time(
                &model,
                sp_a,
                sp_b,
                &HashMap::new(),
                sp_b_entry_map,
                1000.0,
                &config(),
            )
            .unwrap();
            assert_eq!(case, TransferCase::OnlySp2HasEntryPathways);
            // SP_A --[crow-fly × 1.2]--> SL_Y =[pathway: 50m, 40s]=> SP_B
            // time:  272m / 1.5 m/s +  40s (pathway)  ≈ 221s total
            assert_eq!(time, 221);
        }

        // --- Case 5 ---
        #[test]
        fn case5_both_have_pathways() {
            // SP_A =[pathway: 30m, 25s]=> SL_X --[crow-fly × 1.2]--> SL_Y =[pathway: 70m, 55s]=> SP_B
            let model = base_model()
                .pathway(
                    "SP_A",
                    StopType::Point,
                    "SL_X",
                    StopType::StopEntrance,
                    30,
                    25,
                    |pw| {
                        pw.is_bidirectional = false;
                    },
                )
                .pathway(
                    "SL_Y",
                    StopType::StopEntrance,
                    "SP_B",
                    StopType::Point,
                    70,
                    55,
                    |pw| {
                        pw.is_bidirectional = false;
                    },
                )
                .build();

            let idx_a = model.stop_points.get_idx("SP_A").unwrap();
            let idx_b = model.stop_points.get_idx("SP_B").unwrap();
            let (sp_exit_maps, sp_entry_maps) =
                build_pathway_maps(&model, config().walking_speed, config().max_distance);
            let sp_a_exit_map = sp_exit_maps.get(&idx_a).unwrap();
            let sp_b_entry_map = sp_entry_maps.get(&idx_b).unwrap();
            let sp_a = model.stop_points.get("SP_A").unwrap();
            let sp_b = model.stop_points.get("SP_B").unwrap();

            let (time, case) = compute_transfer_time(
                &model,
                sp_a,
                sp_b,
                sp_a_exit_map,
                sp_b_entry_map,
                1000.0,
                &config(),
            )
            .unwrap();
            assert_eq!(case, TransferCase::BothHavePathways);
            // SP_A =[pathway: 30m, 25s]=> SL_X --[crow-fly × 1.2]--> SL_Y =[pathway: 70m, 55s]=> SP_B
            // time: 25s (pathway) + 79m / 1.5 m/s ≈ 52s + 55s (pathway) ≈ 133s total
            assert_eq!(time, 132);
        }

        #[test]
        fn case5_all_paths_exceed_max_distance_returns_no_transfer() {
            // SP_A =[pathway: 200m, 150s]=> SL_X --[~66m (crow-fly) × 1.2 ≈ 79m]--> SL_Y =[pathway: 250m, 190s]=> SP_B
            // dist:  200m (pathway)  +  ~79m (crow-fly)  +  250m (pathway)  = ~529m total
            //        → exceeds max_distance (500m), so no valid path → (f64::MAX, 0)
            let model = base_model()
                .pathway(
                    "SP_A",
                    StopType::Point,
                    "SL_X",
                    StopType::StopEntrance,
                    200,
                    150,
                    |pw| {
                        pw.is_bidirectional = false;
                    },
                )
                .pathway(
                    "SL_Y",
                    StopType::StopEntrance,
                    "SP_B",
                    StopType::Point,
                    250,
                    190,
                    |pw| {
                        pw.is_bidirectional = false;
                    },
                )
                .build();

            let idx_a = model.stop_points.get_idx("SP_A").unwrap();
            let idx_b = model.stop_points.get_idx("SP_B").unwrap();
            let (sp_exit_maps, sp_entry_maps) =
                build_pathway_maps(&model, config().walking_speed, config().max_distance);
            let sp_a_exit_map = sp_exit_maps.get(&idx_a).unwrap();
            let sp_b_entry_map = sp_entry_maps.get(&idx_b).unwrap();
            let sp_a = model.stop_points.get("SP_A").unwrap();
            let sp_b = model.stop_points.get("SP_B").unwrap();

            let result = compute_transfer_time(
                &model,
                sp_a,
                sp_b,
                sp_a_exit_map,
                sp_b_entry_map,
                999.0,
                &config(),
            );
            // → exceeds max_distance (500m), so no valid path → None
            assert!(result.is_none());
        }

        #[test]
        fn case5_selects_fastest_full_chain_matrix() {
            // Geographic layout (same latitude, crow-fly distances):
            //
            //   SP_A          SL_X(exit1)         SL_X2(exit2) SL_Y2(ent2) SL_Y(ent1)          SP_B
            //    |───────────────|───────────────────|───────────|───────────|──────────────────|
            //        ~161m              ~51m               ~7m        ~7m           ~124m
            //  2.3800          2.3822              2.3829       2.3830      2.3831            2.3848
            //
            // Pathways:
            //   SP_A =[160m/110s]=> SL_X     SP_A =[210m/145s]=> SL_X2
            //   SL_Y =[120m/ 85s]=> SP_B     SL_Y2=[150m/120s]=> SP_B
            //
            // Matrix (exit_time + crow-fly × 1.2 / 1.5 m/s + entry_time):
            //
            //                   ┃ SL_Y  (entry 85s)               ┃ SL_Y2 (entry 120s)              ┃
            //  ━━━━━━━━━━━━━━━━━╋━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━╋━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┫
            //  SL_X  (exit 110s)┃ 110 + ~66m×1.2/1.5 + 85  = ~248s┃ 110 + ~58m×1.2/1.5 + 120 = ~277s┃
            //  SL_X2 (exit 145s)┃ 145 + ~15m×1.2/1.5 + 85  = ~241s┃ 145 + ~ 7m×1.2/1.5 + 120 = ~271s┃
            //
            // SL_X2→SL_Y wins (~242s): SP_A's longer exit pathway to SL_X2 (145s vs 110s to SL_X)
            // is compensated by a much shorter crow-fly to SL_Y (~15m vs ~66m from SL_X)
            let model = base_model()
                .stop_location("SL_X2", |sl| {
                    sl.coord = Coord::from(("2.3829".to_string(), "48.8500".to_string()));
                    sl.parent_id = Some("sa1".to_string());
                })
                .stop_location("SL_Y2", |sl| {
                    sl.coord = Coord::from(("2.3830".to_string(), "48.8500".to_string()));
                    sl.parent_id = Some("sa2".to_string());
                })
                .pathway(
                    "SP_A",
                    StopType::Point,
                    "SL_X",
                    StopType::StopEntrance,
                    160,
                    110,
                    |pw| {
                        pw.is_bidirectional = false;
                    },
                )
                .pathway(
                    "SP_A",
                    StopType::Point,
                    "SL_X2",
                    StopType::StopEntrance,
                    210,
                    145,
                    |pw| {
                        pw.is_bidirectional = false;
                    },
                )
                .pathway(
                    "SL_Y",
                    StopType::StopEntrance,
                    "SP_B",
                    StopType::Point,
                    120,
                    85,
                    |pw| {
                        pw.is_bidirectional = false;
                    },
                )
                .pathway(
                    "SL_Y2",
                    StopType::StopEntrance,
                    "SP_B",
                    StopType::Point,
                    150,
                    120,
                    |pw| {
                        pw.is_bidirectional = false;
                    },
                )
                .build();

            let idx_a = model.stop_points.get_idx("SP_A").unwrap();
            let idx_b = model.stop_points.get_idx("SP_B").unwrap();
            let (sp_exit_maps, sp_entry_maps) =
                build_pathway_maps(&model, config().walking_speed, config().max_distance);
            let sp_a_exit_map = sp_exit_maps.get(&idx_a).unwrap();
            let sp_b_entry_map = sp_entry_maps.get(&idx_b).unwrap();
            let sp_a = model.stop_points.get("SP_A").unwrap();
            let sp_b = model.stop_points.get("SP_B").unwrap();

            let (time, case) = compute_transfer_time(
                &model,
                sp_a,
                sp_b,
                sp_a_exit_map,
                sp_b_entry_map,
                999.0,
                &config(),
            )
            .unwrap();
            assert_eq!(case, TransferCase::BothHavePathways);
            assert_eq!(time, 241); // SL_X2→SL_Y, not SL_X→SL_Y despite SL_X having the shorter exit pathway
        }
    }
}
