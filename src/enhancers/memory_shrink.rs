use crate::model::Collections;

/// Optimize memory imprint of the `Model`
pub fn memory_shrink(collections: &mut Collections) {
    collections.stop_time_headsigns.shrink_to_fit();
    collections.stop_time_ids.shrink_to_fit();
    collections.stop_time_comments.shrink_to_fit();
    let vj_idxs: Vec<_> = collections.vehicle_journeys.indexes().collect();
    for vj_idx in vj_idxs {
        collections
            .vehicle_journeys
            .index_mut(vj_idx)
            .stop_times
            .shrink_to_fit();
    }
}
