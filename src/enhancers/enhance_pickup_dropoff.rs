use crate::{
    model::Collections,
    objects::{StopTime, VehicleJourney},
};
use std::collections::{HashMap, HashSet};
use typed_index_collection::Idx;

/// Forbid pickup on last stop point of vehicle journeys and forbid dropoff
/// on first stop point of vehicle journeys.
///
/// However, there is an exception to this rule for authorized stay-in
/// between vehicle journeys. It is possible to get in the last stop point
/// of a vehicle journey or get out on the first stop point of a vehicle
/// journey, if and only if the 2 stop points are different and times do not
/// overlap.
///
/// WARNING: The current implementation does not handle stay-in for vehicle
/// journeys with different validity patterns.
///
/// Here is examples explaining the different stay-in situations (for
/// pick-up and drop-off, XX means forbidden, ―▶ means authorized).
///
/// Example 1:
/// ##########
///       out          in   out         in
///        X    SP1    |    ▲    SP2    X
///        X           ▼    |           X
///  VJ:1   08:00-09:00      10:00-11:00
///  VJ:2                    10:00-11:00      14:00-15:00
///                         X           ▲    |           X
///                         X           |    ▼   SP3     X
///                         out         in   out         in
///                         |- Stay-In -|
///
/// In this example the stop SP2 is in both VJ, so we can forbid the pick-up
/// for VJ:1 / drop-off for VJ:2 since we don't want to tell a traveler to take VJ:1
/// at SP2 but VJ:2
///
/// Example 2:
/// ##########
///       out          in  out               in
///        X    SP1    |    ▲       SP2      X
///        X           ▼    |                X
///  VJ:1   08:00-09:00      10:00---------12:00
///  VJ:2                           11:00----------13:00      13:00-14:00
///                                   X                 ▲    |           X
///                                   X       SP3       |    ▼    SP4    X
///                                 out                 in  out          in
///                         |--------- Stay In ---------|
///
/// This example show an invalid stay-in since the same vehicule cannot be at both stops.
/// Note the overlap between the departure time of the last stop point SP2
/// of VJ:1 and the arrival time of the first stop point SP3 of VJ:2. In
/// this case, we still apply the default rule.
///
///
/// Example 3:
/// ##########
///       out          in   out         in   out         in   out         in
///        X    SP1    |    ▲    SP2    |    ▲    SP3    |    ▲   SP4     X
///        X           ▼    |           ▼    |           |    |           X
///  VJ:1   08:00-09:00      10:00-11:00     |           ▼    |           X
///  VJ:2                                     12:00-13:00      14:00-15:00
///                         |---------- Stay In ---------|
///
/// Example 3 is the only case were we allow specific pick-up and
/// drop-off.
///
/// Example 4:
/// ##########
///                       SP0               SP1               SP2               SP3
///
///  VJ:1 (Mon-Sun)   09:00-10:00       10:00-11:00
///  VJ:2 (Mon-Fri)                                       12:00-13:00       14:00-15:00
///  VJ:3 (Sat-Sun)                                       12:30-13:30       14:30-15:30
///
/// Example 4 is a valid use case of stay-in
/// The pickup/dropoff will be possible between VJ:1 and VJ:2/VJ:3
pub fn enhance_pickup_dropoff(collections: &mut Collections) {
    let mut allowed_last_pick_up_vj = HashSet::new();
    let mut allowed_first_drop_off_vj = HashSet::new();

    let can_chain_without_overlap = |prev_vj: &VehicleJourney, next_vj: &VehicleJourney| {
        let last_stop = &prev_vj.stop_times.last();
        let first_stop = &next_vj.stop_times.first();
        if let (Some(last_stop), Some(first_stop)) = (last_stop, first_stop) {
            if last_stop.pickup_type == 3
                || first_stop.pickup_type == 3
                || last_stop.drop_off_type == 3
                || first_stop.drop_off_type == 3
            {
                return false;
            }
            if last_stop.stop_point_idx != first_stop.stop_point_idx {
                match (
                    collections.calendars.get(&prev_vj.service_id),
                    collections.calendars.get(&next_vj.service_id),
                ) {
                    (Some(prev), Some(next)) => {
                        // The stay-in is not really possible when timing overlaps
                        // between arrival of first vehicle journey and departure of
                        // next vehicle journey (see Example 2 above).
                        return last_stop.departure_time <= first_stop.arrival_time
                            // for the stay-in to be possible the vj should have at least one date in common
                                && prev.overlaps(next);
                    }
                    _ => return false,
                }
            }
        }
        false
    };
    type BlockId = String;
    let mut vj_by_blocks = HashMap::<BlockId, Vec<(Idx<VehicleJourney>, &VehicleJourney)>>::new();

    for (b, (vj_idx, vj)) in collections
        .vehicle_journeys
        .iter()
        .filter_map(|(vj_idx, vj)| vj.block_id.clone().map(|b| (b, (vj_idx, vj))))
    {
        let other_block_id_vj = vj_by_blocks.entry(b).or_default();

        // for every vj we check if it can really be a stay-in and if the last stop
        // is not in both vj (example 1)
        // Note: this is quadratic but should not be too costly since
        // the number of vj checked should be limited
        for (other_vj_idx, other_vj) in other_block_id_vj.iter_mut() {
            if can_chain_without_overlap(vj, other_vj) {
                allowed_first_drop_off_vj.insert(*other_vj_idx);
                allowed_last_pick_up_vj.insert(vj_idx);
            } else if can_chain_without_overlap(other_vj, vj) {
                allowed_first_drop_off_vj.insert(vj_idx);
                allowed_last_pick_up_vj.insert(*other_vj_idx);
            }
        }
        other_block_id_vj.push((vj_idx, vj));
    }

    let is_route_point =
        |stop_time: &StopTime| stop_time.pickup_type == 3 || stop_time.drop_off_type == 3;
    for vj_idx in collections.vehicle_journeys.indexes() {
        let mut vj = collections.vehicle_journeys.index_mut(vj_idx);

        if !allowed_first_drop_off_vj.contains(&vj_idx) {
            if let Some(st) = vj.stop_times.iter_mut().find(|st| !is_route_point(st)) {
                st.drop_off_type = 1;
            }
        }
        if !allowed_last_pick_up_vj.contains(&vj_idx) {
            if let Some(st) = vj
                .stop_times
                .iter_mut()
                .rev()
                .find(|st| !is_route_point(st))
            {
                st.pickup_type = 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::objects::{Calendar, Date, StopPoint, Time};
    use pretty_assertions::assert_eq;
    use std::collections::BTreeSet;
    use typed_index_collection::CollectionWithId;

    // For testing, we need to configure:
    // - block_id (String)
    // - stop_point_idx (usize -> index of one of the four test stop points)
    // - arrival_time (Time)
    // - departure_time (Time)
    type VjConfig = (String, usize, Time, Time);

    // This creates 2 vehicle journeys, each with 2 stop times. There is 4
    // available test stop points 'sp0' ―▶ 'sp3'. First vehicle journey has
    // a first stop time with 'sp0' and second stop time configurable with
    // 'prev_vj_config'. Second vehicle journey has a first stop time
    // configurable with 'next_vj_config' and second stop time with 'sp3'.
    fn build_vehicle_journeys(
        prev_vj_config: VjConfig,
        next_vj_config: VjConfig,
    ) -> CollectionWithId<VehicleJourney> {
        let mut stop_points = CollectionWithId::default();
        let mut sp_idxs = Vec::new();
        for i in 0..4 {
            let idx = stop_points
                .push(StopPoint {
                    id: format!("sp{}", i),
                    ..Default::default()
                })
                .unwrap();
            sp_idxs.push(idx);
        }
        // First vehicle journey, first stop time
        let stop_time_1 = StopTime {
            stop_point_idx: sp_idxs[0],
            sequence: 0,
            arrival_time: prev_vj_config.2 - Time::new(1, 0, 0),
            departure_time: prev_vj_config.3 - Time::new(1, 0, 0),
            boarding_duration: 0,
            alighting_duration: 0,
            pickup_type: 0,
            drop_off_type: 0,
            local_zone_id: None,
            precision: None,
            geometry_id: None,
        };
        // First vehicle journey, second stop time
        let stop_time_2 = StopTime {
            stop_point_idx: sp_idxs[prev_vj_config.1],
            sequence: 0,
            arrival_time: prev_vj_config.2,
            departure_time: prev_vj_config.3,
            boarding_duration: 0,
            alighting_duration: 0,
            pickup_type: 0,
            drop_off_type: 0,
            local_zone_id: None,
            precision: None,
            geometry_id: None,
        };
        // Second vehicle journey, first stop time
        let next_vj_config_time_1 = StopTime {
            stop_point_idx: sp_idxs[next_vj_config.1],
            sequence: 1,
            arrival_time: next_vj_config.2,
            departure_time: next_vj_config.3,
            boarding_duration: 0,
            alighting_duration: 0,
            pickup_type: 0,
            drop_off_type: 0,
            local_zone_id: None,
            precision: None,
            geometry_id: None,
        };
        // Second vehicle journey, second stop time
        let next_vj_config_time_2 = StopTime {
            stop_point_idx: sp_idxs[3],
            sequence: 1,
            arrival_time: next_vj_config.2 + Time::new(1, 0, 0),
            departure_time: next_vj_config.3 + Time::new(1, 0, 0),
            boarding_duration: 0,
            alighting_duration: 0,
            pickup_type: 0,
            drop_off_type: 0,
            local_zone_id: None,
            precision: None,
            geometry_id: None,
        };

        let vj1 = VehicleJourney {
            id: "vj1".to_string(),
            block_id: Some(prev_vj_config.0),
            stop_times: vec![stop_time_1, stop_time_2],
            ..Default::default()
        };
        let vj2 = VehicleJourney {
            id: "vj2".to_string(),
            block_id: Some(next_vj_config.0),
            stop_times: vec![next_vj_config_time_1, next_vj_config_time_2],
            ..Default::default()
        };
        CollectionWithId::new(vec![vj1, vj2]).unwrap()
    }

    #[test]
    fn no_stay_in() {
        let mut collections = Collections::default();
        let prev_vj_config = (
            "block_id_1".to_string(),
            1,
            Time::new(10, 0, 0),
            Time::new(11, 0, 0),
        );
        let next_vj_config = (
            "block_id_2".to_string(),
            2,
            Time::new(10, 0, 0),
            Time::new(11, 0, 0),
        );
        collections.vehicle_journeys = build_vehicle_journeys(prev_vj_config, next_vj_config);
        enhance_pickup_dropoff(&mut collections);
        let vj1 = collections.vehicle_journeys.get("vj1").unwrap();
        let stop_time = &vj1.stop_times[0];
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(1, stop_time.drop_off_type);
        let stop_time = &vj1.stop_times.last().unwrap();
        assert_eq!(1, stop_time.pickup_type);
        assert_eq!(0, stop_time.drop_off_type);
        let vj2 = collections.vehicle_journeys.get("vj2").unwrap();
        let stop_time = &vj2.stop_times[0];
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(1, stop_time.drop_off_type);
        let stop_time = &vj2.stop_times.last().unwrap();
        assert_eq!(1, stop_time.pickup_type);
        assert_eq!(0, stop_time.drop_off_type);
    }

    // Example 1
    #[test]
    fn stay_in_same_stop() {
        let mut collections = Collections::default();
        let prev_vj_config = (
            "block_id_1".to_string(),
            1,
            Time::new(10, 0, 0),
            Time::new(11, 0, 0),
        );
        let next_vj_config = (
            "block_id_1".to_string(),
            1,
            Time::new(10, 0, 0),
            Time::new(11, 0, 0),
        );
        collections.vehicle_journeys = build_vehicle_journeys(prev_vj_config, next_vj_config);
        let mut dates = BTreeSet::new();
        dates.insert(Date::from_ymd_opt(2020, 1, 1).unwrap());
        collections.calendars = CollectionWithId::new(vec![Calendar {
            id: "default_service".to_owned(),
            dates,
        }])
        .unwrap();
        enhance_pickup_dropoff(&mut collections);
        let vj1 = collections.vehicle_journeys.get("vj1").unwrap();
        let stop_time = &vj1.stop_times[0];
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(1, stop_time.drop_off_type);
        let stop_time = &vj1.stop_times.last().unwrap();
        assert_eq!(1, stop_time.pickup_type);
        assert_eq!(0, stop_time.drop_off_type);
        let vj2 = collections.vehicle_journeys.get("vj2").unwrap();
        let stop_time = &vj2.stop_times[0];
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(1, stop_time.drop_off_type);
        let stop_time = &vj2.stop_times.last().unwrap();
        assert_eq!(1, stop_time.pickup_type);
        assert_eq!(0, stop_time.drop_off_type);
    }

    // Example 2
    #[test]
    fn stay_in_different_stop_overlapping_time() {
        let mut collections = Collections::default();
        let prev_vj_config = (
            "block_id_1".to_string(),
            1,
            Time::new(10, 0, 0),
            Time::new(12, 0, 0),
        );
        let next_vj_config = (
            "block_id_1".to_string(),
            2,
            Time::new(11, 0, 0),
            Time::new(13, 0, 0),
        );
        collections.vehicle_journeys = build_vehicle_journeys(prev_vj_config, next_vj_config);
        let mut dates = BTreeSet::new();
        dates.insert(Date::from_ymd_opt(2020, 1, 1).unwrap());
        collections.calendars = CollectionWithId::new(vec![Calendar {
            id: "default_service".to_owned(),
            dates,
        }])
        .unwrap();
        enhance_pickup_dropoff(&mut collections);
        let vj1 = collections.vehicle_journeys.get("vj1").unwrap();
        let stop_time = &vj1.stop_times[0];
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(1, stop_time.drop_off_type);
        let stop_time = &vj1.stop_times.last().unwrap();
        assert_eq!(1, stop_time.pickup_type);
        assert_eq!(0, stop_time.drop_off_type);
        let vj2 = collections.vehicle_journeys.get("vj2").unwrap();
        let stop_time = &vj2.stop_times[0];
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(1, stop_time.drop_off_type);
        let stop_time = &vj2.stop_times.last().unwrap();
        assert_eq!(1, stop_time.pickup_type);
        assert_eq!(0, stop_time.drop_off_type);
    }

    // Example 3
    #[test]
    fn stay_in_different_stop() {
        let mut collections = Collections::default();
        let prev_vj_config = (
            "block_id_1".to_string(),
            1,
            Time::new(10, 0, 0),
            Time::new(11, 0, 0),
        );
        let next_vj_config = (
            "block_id_1".to_string(),
            2,
            Time::new(12, 0, 0),
            Time::new(13, 0, 0),
        );
        collections.vehicle_journeys = build_vehicle_journeys(prev_vj_config, next_vj_config);
        let mut dates = BTreeSet::new();
        dates.insert(Date::from_ymd_opt(2020, 1, 1).unwrap());
        collections.calendars = CollectionWithId::new(vec![Calendar {
            id: "default_service".to_owned(),
            dates,
        }])
        .unwrap();
        enhance_pickup_dropoff(&mut collections);
        let vj1 = collections.vehicle_journeys.get("vj1").unwrap();
        let stop_time = &vj1.stop_times[0];
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(1, stop_time.drop_off_type);
        let stop_time = &vj1.stop_times.last().unwrap();
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(0, stop_time.drop_off_type);
        let vj2 = collections.vehicle_journeys.get("vj2").unwrap();
        let stop_time = &vj2.stop_times[0];
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(0, stop_time.drop_off_type);
        let stop_time = &vj2.stop_times.last().unwrap();
        assert_eq!(1, stop_time.pickup_type);
        assert_eq!(0, stop_time.drop_off_type);
    }

    // Example 3... but with route points (should not considered it a valid stay-in case)
    // This is the same test as above, just adding an additional route-point.
    #[test]
    fn stay_in_different_stop_but_with_route_point() {
        let mut collections = Collections::default();
        let prev_vj_config = (
            "block_id_1".to_string(),
            1,
            Time::new(10, 0, 0),
            Time::new(11, 0, 0),
        );
        let next_vj_config = (
            "block_id_1".to_string(),
            2,
            Time::new(12, 0, 0),
            Time::new(13, 0, 0),
        );
        collections.vehicle_journeys = build_vehicle_journeys(prev_vj_config, next_vj_config);
        let sp4_idx = collections
            .stop_points
            .push(StopPoint {
                id: String::from("sp4"),
                ..Default::default()
            })
            .unwrap();
        let vj_idx = collections.vehicle_journeys.get_idx("vj1").unwrap();
        let mut vj_mut = collections.vehicle_journeys.index_mut(vj_idx);
        vj_mut.stop_times.push(StopTime {
            stop_point_idx: sp4_idx,
            sequence: 2,
            arrival_time: Time::new(11, 30, 0),
            departure_time: Time::new(11, 30, 0),
            boarding_duration: 0,
            alighting_duration: 0,
            pickup_type: 3,
            drop_off_type: 3,
            local_zone_id: None,
            precision: None,
            geometry_id: None,
        });
        drop(vj_mut);
        let mut dates = BTreeSet::new();
        dates.insert(Date::from_ymd_opt(2020, 1, 1).unwrap());
        collections.calendars = CollectionWithId::new(vec![Calendar {
            id: "default_service".to_owned(),
            dates,
        }])
        .unwrap();
        enhance_pickup_dropoff(&mut collections);
        let vj1 = collections.vehicle_journeys.get("vj1").unwrap();
        let stop_time = &vj1.stop_times[0];
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(1, stop_time.drop_off_type);
        let stop_time = &vj1.stop_times[1];
        assert_eq!(1, stop_time.pickup_type);
        assert_eq!(0, stop_time.drop_off_type);
        let stop_time = &vj1.stop_times[2];
        assert_eq!(3, stop_time.pickup_type);
        assert_eq!(3, stop_time.drop_off_type);
        let vj2 = collections.vehicle_journeys.get("vj2").unwrap();
        let stop_time = &vj2.stop_times[0];
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(1, stop_time.drop_off_type);
        let stop_time = &vj2.stop_times[1];
        assert_eq!(1, stop_time.pickup_type);
        assert_eq!(0, stop_time.drop_off_type);
    }

    #[test]
    fn forbidden_drop_off_should_be_kept() {
        // if restriction are explicitly set they should not be overriden
        let model = transit_model_builder::ModelBuilder::default()
            .vj("vj1", |vj| {
                vj.block_id("block_1")
                    .st("SP1", "10:00:00", "10:01:00")
                    .st_mut("SP2", "11:00:00", "11:01:00", |st| {
                        st.pickup_type = 1;
                        st.drop_off_type = 1;
                    });
            })
            .vj("vj2", |vj| {
                vj.block_id("block_1")
                    .st_mut("SP3", "12:00:00", "12:01:00", |st| {
                        st.drop_off_type = 2; // for fun this has a 'must call' type, we should also keep it
                    })
                    .st("SP4", "13:00:00", "13:01:00");
            })
            .build();
        let vj1 = model.vehicle_journeys.get("vj1").unwrap();
        let stop_time = &vj1.stop_times[0];
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(1, stop_time.drop_off_type); // it has not been explicitly changed so the 1st drop_off is forbidden
                                                // the vj should have the last st pickup forbidden even if it's a
                                                // stay-in because it was explicitly forbidden
        let stop_time = &vj1.stop_times.last().unwrap();
        assert_eq!(1, stop_time.pickup_type);
        assert_eq!(1, stop_time.drop_off_type);
        let vj2 = model.vehicle_journeys.get("vj2").unwrap();
        // the vj should have the first st drop_off forbidden even if it's a
        // stay-in because it was explicitly forbidden
        let stop_time = &vj2.stop_times[0];
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(2, stop_time.drop_off_type);
        let stop_time = &vj2.stop_times.last().unwrap();
        assert_eq!(1, stop_time.pickup_type);
        assert_eq!(0, stop_time.drop_off_type);
    }

    #[test]
    fn block_id_on_overlapping_calendar_ok() {
        // a bit like the example 4 but on less days
        // working days:
        // days: 01 02 03 04
        // VJ:1   X  X  X
        // VJ:2   X  X         <- calendar is included in VJ:1's calendar
        // VJ:3         X  X   <- calendar is overlaping in VJ:1's calendar
        //
        // VJ:3 can sometimes be taken after VJ:1 so we also don't want to forbid
        // pick-up at last stop / drop-off at 1st stop
        let model = transit_model_builder::ModelBuilder::default()
            .calendar("c1", &["2020-01-01", "2020-01-02", "2020-01-03"])
            .calendar("c2", &["2020-01-01", "2020-01-02"])
            .calendar("c3", &["2020-01-03", "2020-01-04"])
            .vj("VJ:1", |vj| {
                vj.block_id("block_1")
                    .calendar("c1")
                    .st("SP1", "10:00:00", "10:01:00")
                    .st("SP2", "11:00:00", "11:01:00");
            })
            .vj("VJ:2", |vj| {
                vj.block_id("block_1")
                    .calendar("c2")
                    .st("SP3", "12:00:00", "12:01:00")
                    .st("SP4", "13:00:00", "13:01:00");
            })
            .vj("VJ:3", |vj| {
                vj.block_id("block_1")
                    .calendar("c3")
                    .st("SP3", "12:30:00", "12:31:00")
                    .st("SP4", "13:30:00", "13:31:00");
            })
            .build();

        let vj1 = model.vehicle_journeys.get("VJ:1").unwrap();
        let stop_time = &vj1.stop_times[0];
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(1, stop_time.drop_off_type);
        let stop_time = &vj1.stop_times.last().unwrap();
        assert_eq!(0, stop_time.pickup_type); // pickup should be possible since the traveler can stay-in the vehicle
        assert_eq!(0, stop_time.drop_off_type);
        let vj2 = model.vehicle_journeys.get("VJ:2").unwrap();
        let stop_time = &vj2.stop_times[0];
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(0, stop_time.drop_off_type); // drop off on first stop possible if anyone took the stay-in
        let stop_time = &vj2.stop_times.last().unwrap();
        assert_eq!(1, stop_time.pickup_type); // impossible to pickup on last stop
        assert_eq!(0, stop_time.drop_off_type);
        let vj3 = model.vehicle_journeys.get("VJ:3").unwrap();
        let stop_time = &vj3.stop_times[0];
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(0, stop_time.drop_off_type); // drop off on first stop possible if anyone took the stay-in
        let stop_time = &vj3.stop_times.last().unwrap();
        assert_eq!(1, stop_time.pickup_type);
        assert_eq!(0, stop_time.drop_off_type);
    }

    #[test]
    fn block_id_on_overlapping_calendar_forbidden_pickup() {
        // like the example 4 but on less days
        // working days:
        // days: 01 02 03 04
        // VJ:1   X  X  X  X
        // VJ:2   X  X  X
        // VJ:3            X
        // VJ:1 has a forbidden pick up at the 2nd stop-time that should be kept
        let model = transit_model_builder::ModelBuilder::default()
            .calendar(
                "c1",
                &["2020-01-01", "2020-01-02", "2020-01-03", "2020-01-04"],
            )
            .calendar("c2", &["2020-01-01", "2020-01-02", "2020-01-03"])
            .calendar("c3", &["2020-01-04"])
            .vj("VJ:1", |vj| {
                vj.block_id("block_1")
                    .calendar("c1")
                    .st("SP1", "10:00:00", "10:01:00")
                    .st_mut("SP2", "11:00:00", "11:01:00", |st| {
                        st.pickup_type = 1;
                    }); // forbidden
            })
            .vj("VJ:2", |vj| {
                vj.block_id("block_1")
                    .calendar("c2")
                    .st("SP3", "12:00:00", "12:01:00")
                    .st("SP4", "13:00:00", "13:01:00");
            })
            .vj("VJ:3", |vj| {
                vj.block_id("block_1")
                    .calendar("c3")
                    .st("SP3", "12:30:00", "12:31:00")
                    .st("SP4", "13:30:00", "13:31:00");
            })
            .build();

        let vj1 = model.vehicle_journeys.get("VJ:1").unwrap();
        let stop_time = &vj1.stop_times[0];
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(1, stop_time.drop_off_type);
        let stop_time = &vj1.stop_times.last().unwrap();
        assert_eq!(1, stop_time.pickup_type); // pickup should not be possible since it has been explicitly forbidden
        assert_eq!(0, stop_time.drop_off_type);
        let vj2 = model.vehicle_journeys.get("VJ:2").unwrap();
        let stop_time = &vj2.stop_times[0];
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(0, stop_time.drop_off_type); // drop off on first stop possible if anyone took the stay-in
        let stop_time = &vj2.stop_times.last().unwrap();
        assert_eq!(1, stop_time.pickup_type); // impossible to pickup on last stop
        assert_eq!(0, stop_time.drop_off_type);
        let vj3 = model.vehicle_journeys.get("VJ:3").unwrap();
        let stop_time = &vj3.stop_times[0];
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(0, stop_time.drop_off_type); // drop off on first stop possible if anyone took the stay-in
        let stop_time = &vj3.stop_times.last().unwrap();
        assert_eq!(1, stop_time.pickup_type);
        assert_eq!(0, stop_time.drop_off_type);
    }

    #[test]
    fn block_id_on_non_overlaping_calendar_ko() {
        // like the example 4 but with non overlaping calendars
        // working days:
        // days: 01 02 03
        // VJ:1   X  X
        // VJ:2         X
        // The pick-up (resp drop-off) at first (resp last) stop should be forbidden
        let model = transit_model_builder::ModelBuilder::default()
            .calendar("c1", &["2020-01-01", "2020-01-02"])
            .calendar("c2", &["2020-01-03"])
            .vj("VJ:1", |vj| {
                vj.block_id("block_1")
                    .calendar("c1")
                    .st("SP1", "10:00:00", "10:01:00")
                    .st("SP2", "11:00:00", "11:01:00");
            })
            .vj("VJ:2", |vj| {
                vj.block_id("block_1")
                    .calendar("c2")
                    .st("SP3", "12:00:00", "12:01:00")
                    .st("SP4", "13:00:00", "13:01:00");
            })
            .build();

        let vj1 = model.vehicle_journeys.get("VJ:1").unwrap();
        let stop_time = &vj1.stop_times[0];
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(1, stop_time.drop_off_type);
        let stop_time = &vj1.stop_times.last().unwrap();
        assert_eq!(1, stop_time.pickup_type);
        assert_eq!(0, stop_time.drop_off_type);
        let vj2 = model.vehicle_journeys.get("VJ:2").unwrap();
        let stop_time = &vj2.stop_times[0];
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(1, stop_time.drop_off_type);
        let stop_time = &vj2.stop_times.last().unwrap();
        assert_eq!(1, stop_time.pickup_type);
        assert_eq!(0, stop_time.drop_off_type);
    }

    #[test]
    fn block_id_on_non_overlaping_calendar_with_overlaping_stops() {
        // tricky test case when there is no perfect response
        //
        // working days:
        // days: 01 02
        // VJ:1   X  X
        // VJ:2   X
        // VJ:3      X
        //
        // and
        // VJ:1  SP1 ---> SP2
        // VJ:2                    SP3 ---> SP4
        // VJ:3           SP2 ---> SP3
        //
        // VJ:1 and VJ:2 can be chained by stay-in so we need to let the pick-up
        // on VJ:1 at SP2 even if we would have wanted to forbid it for the stay-in
        // VJ:1 - VJ:3
        // we can however forbid the drop-off on VJ:3 at SP:2
        let model = transit_model_builder::ModelBuilder::default()
            .calendar("c1", &["2020-01-01", "2020-01-02"])
            .calendar("c2", &["2020-01-01"])
            .calendar("c3", &["2020-01-02"])
            .vj("VJ:1", |vj| {
                vj.block_id("block_1")
                    .calendar("c1")
                    .st("SP1", "10:00:00", "10:01:00")
                    .st("SP2", "11:00:00", "11:01:00");
            })
            .vj("VJ:2", |vj| {
                vj.block_id("block_1")
                    .calendar("c2")
                    .st("SP3", "12:00:00", "12:01:00")
                    .st("SP4", "13:00:00", "13:01:00");
            })
            .vj("VJ:3", |vj| {
                vj.block_id("block_1")
                    .calendar("c3")
                    .st("SP2", "12:00:00", "12:01:00")
                    .st("SP3", "13:00:00", "13:01:00");
            })
            .build();

        let vj1 = model.vehicle_journeys.get("VJ:1").unwrap();
        let stop_time = &vj1.stop_times[0];
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(1, stop_time.drop_off_type);
        let stop_time = &vj1.stop_times.last().unwrap();
        assert_eq!(0, stop_time.pickup_type); // pick-up is authorized
        assert_eq!(0, stop_time.drop_off_type);
        let vj2 = model.vehicle_journeys.get("VJ:2").unwrap();
        let stop_time = &vj2.stop_times[0];
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(0, stop_time.drop_off_type); // drop-off is authorized
        let stop_time = &vj2.stop_times.last().unwrap();
        assert_eq!(1, stop_time.pickup_type);
        assert_eq!(0, stop_time.drop_off_type);
        let vj3 = model.vehicle_journeys.get("VJ:3").unwrap();
        let stop_time = &vj3.stop_times[0];
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(1, stop_time.drop_off_type); // drop-off is forbidden
        let stop_time = &vj3.stop_times.last().unwrap();
        assert_eq!(1, stop_time.pickup_type);
        assert_eq!(0, stop_time.drop_off_type);
    }

    #[test]
    fn ignore_route_points() {
        let model = transit_model_builder::ModelBuilder::default()
            .vj("VJ1:1", |vj| {
                vj.st_mut("SP1", "10:00:00", "10:01:00", |st| {
                    st.pickup_type = 3;
                    st.drop_off_type = 3;
                })
                .st("SP2", "10:30:00", "10:31:00")
                .st("SP3", "11:00:00", "11:01:00")
                .st_mut("SP4", "11:30:00", "11:31:00", |st| {
                    st.pickup_type = 3;
                    st.drop_off_type = 3;
                });
            })
            .build();
        let vj1 = model.vehicle_journeys.get("VJ1:1").unwrap();
        let stop_time = &vj1.stop_times[0];
        assert_eq!(3, stop_time.pickup_type);
        assert_eq!(3, stop_time.drop_off_type);
        let stop_time = &vj1.stop_times[1];
        assert_eq!(0, stop_time.pickup_type);
        assert_eq!(1, stop_time.drop_off_type);
        let stop_time = &vj1.stop_times[2];
        assert_eq!(1, stop_time.pickup_type);
        assert_eq!(0, stop_time.drop_off_type);
        let stop_time = &vj1.stop_times[3];
        assert_eq!(3, stop_time.pickup_type);
        assert_eq!(3, stop_time.drop_off_type);
    }
}
