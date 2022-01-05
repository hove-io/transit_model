use crate::model::Collections;
use tracing::warn;
use typed_index_collection::CollectionWithId;

pub fn check_stop_times_order(collections: &mut Collections) {
    let vehicle_journeys = collections.vehicle_journeys.take();
    let mut filtered_vjs = Vec::new();
    for mut vj in vehicle_journeys {
        match vj.sort_and_check_stop_times() {
            Ok(_) => filtered_vjs.push(vj),
            Err(e) => warn!("{}", e),
        }
    }
    collections.vehicle_journeys = CollectionWithId::new(filtered_vjs)
        .expect("insert only vehicle journeys that were in a CollectionWithId before");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::objects::{StopPoint, StopTime, Time, VehicleJourney};
    use std::str::FromStr;

    fn collections_from_times(
        (a_arrival, a_departure): (&str, &str),
        (b_arrival, b_departure): (&str, &str),
    ) -> Collections {
        let mut collections = Collections::default();
        let stop_points = CollectionWithId::from(StopPoint {
            id: "sp1".to_string(),
            ..Default::default()
        });
        let stop_point_idx = stop_points.get_idx("sp1").unwrap();
        let stop_times = vec![
            StopTime {
                stop_point_idx,
                sequence: 0,
                arrival_time: Time::from_str(a_arrival).unwrap(),
                departure_time: Time::from_str(a_departure).unwrap(),
                boarding_duration: 0,
                alighting_duration: 0,
                pickup_type: 0,
                drop_off_type: 0,
                local_zone_id: None,
                precision: None,
            },
            StopTime {
                stop_point_idx,
                sequence: 1,
                arrival_time: FromStr::from_str(b_arrival).unwrap(),
                departure_time: FromStr::from_str(b_departure).unwrap(),
                boarding_duration: 0,
                alighting_duration: 0,
                pickup_type: 0,
                drop_off_type: 0,
                local_zone_id: None,
                precision: None,
            },
        ];
        collections.vehicle_journeys = CollectionWithId::from(VehicleJourney {
            id: "vj1".to_string(),
            stop_times,
            ..Default::default()
        });
        collections
    }

    #[test]
    fn valid_vj() {
        let mut collections =
            collections_from_times(("10:00:00", "10:05:00"), ("11:00:00", "11:05:00"));

        check_stop_times_order(&mut collections);

        assert!(collections.vehicle_journeys.contains_id("vj1"));
    }

    #[test]
    fn invalid_growing_stop_times_inside_stop() {
        testing_logger::setup();
        let mut collections =
            collections_from_times(("10:05:00", "10:00:00"), ("11:00:00", "11:05:00"));

        check_stop_times_order(&mut collections);

        assert!(!collections.vehicle_journeys.contains_id("vj1"));
        testing_logger::validate(|captured_logs| {
            let error_log = captured_logs
                .iter()
                .find(|captured_log| captured_log.level == tracing::log::Level::Warn)
                .expect("log error expected");
            assert!(error_log
                .body
                .contains("incoherent stop times \'0\' at time \'10:00:00\' for the trip \'vj1\'"));
        });
    }

    #[test]
    fn invalid_growing_stop_times_over_two_stops() {
        testing_logger::setup();
        let mut collections =
            collections_from_times(("10:00:00", "10:05:00"), ("10:03:00", "10:10:00"));

        check_stop_times_order(&mut collections);

        assert!(!collections.vehicle_journeys.contains_id("vj1"));
        testing_logger::validate(|captured_logs| {
            for log in captured_logs {
                dbg!(&log.body);
            }
            let error_log = captured_logs
                .iter()
                .find(|captured_log| captured_log.level == tracing::log::Level::Warn)
                .expect("log error expected");
            assert!(error_log
                .body
                .contains("incoherent stop times \'0\' at time \'10:05:00\' for the trip \'vj1\'"));
        });
    }
}
