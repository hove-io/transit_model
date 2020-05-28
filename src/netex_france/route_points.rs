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

// This module is an implementation of the algorithm described in
// https://github.com/CanalTP/navitia/blob/dev/documentation/rfc/sorting_vehicles_route_schedules.md

use crate::objects::{StopPoint, VehicleJourney};
use typed_index_collection::Idx;

pub fn build_route_points<'m, I>(vehicle_journeys: I) -> Vec<Idx<StopPoint>>
where
    I: IntoIterator<Item = &'m VehicleJourney>,
{
    let mut final_route_points: Vec<Idx<StopPoint>> = Vec::new();
    for vehicle_journey in vehicle_journeys {
        let mut route_points: Vec<Idx<StopPoint>> = Vec::new();
        let mut to_insert: Vec<Idx<StopPoint>> = vehicle_journey
            .stop_times
            .iter()
            .map(|stop_time| stop_time.stop_point_idx)
            .collect();
        for route_point in final_route_points {
            if to_insert.contains(&route_point) {
                // This loop will break eventually because:
                // - each iteration, we remove an item from 'to_insert'
                // - the loop breaks when we find 'route_point'
                // - we know 'route_point' is present (see '.contains()' above)
                loop {
                    let route_point_to_insert = to_insert.remove(0);
                    route_points.push(route_point_to_insert);
                    if route_point_to_insert == route_point {
                        break;
                    }
                }
            } else {
                route_points.push(route_point);
            }
        }
        // Add the remaining StopPoint at the end
        route_points.extend(to_insert);
        final_route_points = route_points;
    }
    final_route_points
}

#[cfg(test)]
mod tests {
    use super::*;
    use typed_index_collection::CollectionWithId;

    fn stop_points() -> CollectionWithId<StopPoint> {
        CollectionWithId::new(vec![
            StopPoint {
                id: String::from("stop_point_1"),
                ..Default::default()
            },
            StopPoint {
                id: String::from("stop_point_2"),
                ..Default::default()
            },
            StopPoint {
                id: String::from("stop_point_3"),
                ..Default::default()
            },
            StopPoint {
                id: String::from("stop_point_4"),
                ..Default::default()
            },
            StopPoint {
                id: String::from("stop_point_5"),
                ..Default::default()
            },
            StopPoint {
                id: String::from("stop_point_6"),
                ..Default::default()
            },
            StopPoint {
                id: String::from("stop_point_7"),
                ..Default::default()
            },
            StopPoint {
                id: String::from("stop_point_8"),
                ..Default::default()
            },
        ])
        .unwrap()
    }

    mod build_route_points {
        use super::*;
        use crate::objects::{CommentLinksT, KeysValues, StopTime, Time};
        use pretty_assertions::assert_eq;

        fn stop_time(
            stop_point_idx: Idx<StopPoint>,
            sequence: u32,
            departure_time: Time,
        ) -> StopTime {
            StopTime {
                id: None,
                stop_point_idx,
                sequence,
                headsign: None,
                arrival_time: Time::new(0, 0, 0),
                departure_time,
                boarding_duration: 0,
                alighting_duration: 0,
                pickup_type: 0,
                drop_off_type: 0,
                datetime_estimated: false,
                local_zone_id: None,
                precision: None,
                comment_links: None,
            }
        }

        fn vehicle_journey(id: String, stop_times: Vec<StopTime>) -> VehicleJourney {
            VehicleJourney {
                id,
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                route_id: String::from("route_id"),
                physical_mode_id: String::from("Bus"),
                dataset_id: String::from("dataset_id"),
                service_id: String::from("service_id"),
                headsign: None,
                short_name: None,
                block_id: None,
                company_id: String::from("company_id"),
                trip_property_id: None,
                geometry_id: None,
                stop_times,
                journey_pattern_id: None,
            }
        }

        // Use case:
        // The two vehicle journeys have the same journey pattern.
        //
        // Expected result:
        // The order of stop points must be the same as either of the vehicle
        // journey.
        #[test]
        fn same_journey_pattern() {
            let stop_points = stop_points();
            let stop_time_1 = stop_time(
                stop_points.get_idx("stop_point_1").unwrap(),
                0,
                Time::new(1, 0, 0),
            );
            let stop_time_2 = stop_time(
                stop_points.get_idx("stop_point_2").unwrap(),
                0,
                Time::new(2, 0, 0),
            );
            let stop_time_3 = stop_time(
                stop_points.get_idx("stop_point_3").unwrap(),
                0,
                Time::new(3, 0, 0),
            );
            let vehicle_journey_1 = vehicle_journey(
                String::from("vehicle_journey_1"),
                vec![
                    stop_time_1.clone(),
                    stop_time_2.clone(),
                    stop_time_3.clone(),
                ],
            );
            let vehicle_journey_2 = vehicle_journey(
                String::from("vehicle_journey_2"),
                vec![stop_time_1, stop_time_2, stop_time_3],
            );
            let vehicle_journeys = vec![vehicle_journey_1, vehicle_journey_2];
            let route_points = build_route_points(&vehicle_journeys);
            assert_eq!(3, route_points.len());
            let stop_point = &stop_points[route_points[0]];
            assert_eq!("stop_point_1", stop_point.id.as_str());
            let stop_point = &stop_points[route_points[1]];
            assert_eq!("stop_point_2", stop_point.id.as_str());
            let stop_point = &stop_points[route_points[2]];
            assert_eq!("stop_point_3", stop_point.id.as_str());
        }

        // Use case:
        // The two vehicle journey starts from different stop points.
        //
        // Expected result:
        // The two stop points must be ordered in the same order than the
        // vehicle journeys
        #[test]
        fn forked_journey_pattern() {
            let stop_points = stop_points();
            let stop_time_1 = stop_time(
                stop_points.get_idx("stop_point_1").unwrap(),
                0,
                Time::new(1, 0, 0),
            );
            let stop_time_2 = stop_time(
                stop_points.get_idx("stop_point_2").unwrap(),
                0,
                Time::new(2, 0, 0),
            );
            let stop_time_3 = stop_time(
                stop_points.get_idx("stop_point_3").unwrap(),
                0,
                Time::new(3, 0, 0),
            );
            let stop_time_4 = stop_time(
                stop_points.get_idx("stop_point_4").unwrap(),
                0,
                Time::new(4, 0, 0),
            );
            let stop_time_5 = stop_time(
                stop_points.get_idx("stop_point_5").unwrap(),
                0,
                Time::new(5, 0, 0),
            );
            let vehicle_journey_1 = vehicle_journey(
                String::from("vehicle_journey_1"),
                vec![stop_time_1, stop_time_3.clone(), stop_time_4],
            );
            let vehicle_journey_2 = vehicle_journey(
                String::from("vehicle_journey_2"),
                vec![stop_time_2, stop_time_3, stop_time_5],
            );
            let vehicle_journeys = vec![vehicle_journey_1, vehicle_journey_2];
            let route_points = build_route_points(&vehicle_journeys);
            assert_eq!(5, route_points.len());
            let stop_point = &stop_points[route_points[0]];
            assert_eq!("stop_point_1", stop_point.id.as_str());
            let stop_point = &stop_points[route_points[1]];
            assert_eq!("stop_point_2", stop_point.id.as_str());
            let stop_point = &stop_points[route_points[2]];
            assert_eq!("stop_point_3", stop_point.id.as_str());
            let stop_point = &stop_points[route_points[3]];
            assert_eq!("stop_point_4", stop_point.id.as_str());
            let stop_point = &stop_points[route_points[4]];
            assert_eq!("stop_point_5", stop_point.id.as_str());
        }

        // Use case:
        // The two vehicle journeys have the same first and last stop point but
        // their middle stop point differ.
        //
        // Expected result:
        // The two moddle stop points must be ordered in the same order than the
        // vehicle journeys
        #[test]
        fn in_between_stop() {
            let stop_points = stop_points();
            let stop_time_1 = stop_time(
                stop_points.get_idx("stop_point_1").unwrap(),
                0,
                Time::new(1, 0, 0),
            );
            let stop_time_2 = stop_time(
                stop_points.get_idx("stop_point_2").unwrap(),
                0,
                Time::new(2, 0, 0),
            );
            let stop_time_3 = stop_time(
                stop_points.get_idx("stop_point_3").unwrap(),
                0,
                Time::new(3, 0, 0),
            );
            let stop_time_4 = stop_time(
                stop_points.get_idx("stop_point_4").unwrap(),
                0,
                Time::new(4, 0, 0),
            );
            let vehicle_journey_1 = vehicle_journey(
                String::from("vehicle_journey_1"),
                vec![stop_time_1.clone(), stop_time_2, stop_time_4.clone()],
            );
            let vehicle_journey_2 = vehicle_journey(
                String::from("vehicle_journey_2"),
                vec![stop_time_1, stop_time_3, stop_time_4],
            );
            let vehicle_journeys = vec![vehicle_journey_1, vehicle_journey_2];
            let route_points = build_route_points(&vehicle_journeys);
            assert_eq!(4, route_points.len());
            let stop_point = &stop_points[route_points[0]];
            assert_eq!("stop_point_1", stop_point.id.as_str());
            let stop_point = &stop_points[route_points[1]];
            assert_eq!("stop_point_2", stop_point.id.as_str());
            let stop_point = &stop_points[route_points[2]];
            assert_eq!("stop_point_3", stop_point.id.as_str());
            let stop_point = &stop_points[route_points[3]];
            assert_eq!("stop_point_4", stop_point.id.as_str());
        }

        // Use case:
        // The two vehicle journeys go through the same stop points but in a
        // different order.
        //
        // Expected result:
        // Some stop points are repeated to respect ordering.
        #[test]
        fn intertwined_stops() {
            let stop_points = stop_points();
            let stop_time_1 = stop_time(
                stop_points.get_idx("stop_point_1").unwrap(),
                0,
                Time::new(1, 0, 0),
            );
            let stop_time_2 = stop_time(
                stop_points.get_idx("stop_point_2").unwrap(),
                0,
                Time::new(2, 0, 0),
            );
            let stop_time_3 = stop_time(
                stop_points.get_idx("stop_point_3").unwrap(),
                0,
                Time::new(3, 0, 0),
            );
            let stop_time_4 = stop_time(
                stop_points.get_idx("stop_point_4").unwrap(),
                0,
                Time::new(4, 0, 0),
            );
            let vehicle_journey_1 = vehicle_journey(
                String::from("vehicle_journey_1"),
                vec![
                    stop_time_1.clone(),
                    stop_time_2.clone(),
                    stop_time_3.clone(),
                    stop_time_4.clone(),
                ],
            );
            let vehicle_journey_2 = vehicle_journey(
                String::from("vehicle_journey_2"),
                vec![stop_time_1, stop_time_3, stop_time_2, stop_time_4],
            );
            let vehicle_journeys = vec![vehicle_journey_1, vehicle_journey_2];
            let route_points = build_route_points(&vehicle_journeys);
            assert_eq!(5, route_points.len());
            let stop_point = &stop_points[route_points[0]];
            assert_eq!("stop_point_1", stop_point.id.as_str());
            let stop_point = &stop_points[route_points[1]];
            assert_eq!("stop_point_3", stop_point.id.as_str());
            let stop_point = &stop_points[route_points[2]];
            assert_eq!("stop_point_2", stop_point.id.as_str());
            let stop_point = &stop_points[route_points[3]];
            assert_eq!("stop_point_3", stop_point.id.as_str());
            let stop_point = &stop_points[route_points[4]];
            assert_eq!("stop_point_4", stop_point.id.as_str());
        }

        // Use case:
        // One of the vehicle journeys has a cycle (goes twice through the same
        // stop point).
        //
        // Expected result:
        // The repeated stop point should appears twice in the result.
        #[test]
        fn circular_vehicle_journey() {
            let stop_points = stop_points();
            let stop_time_1 = stop_time(
                stop_points.get_idx("stop_point_1").unwrap(),
                0,
                Time::new(1, 0, 0),
            );
            let stop_time_2 = stop_time(
                stop_points.get_idx("stop_point_2").unwrap(),
                0,
                Time::new(2, 0, 0),
            );
            let stop_time_3 = stop_time(
                stop_points.get_idx("stop_point_3").unwrap(),
                0,
                Time::new(3, 0, 0),
            );
            let stop_time_1_bis = stop_time(
                stop_points.get_idx("stop_point_1").unwrap(),
                0,
                Time::new(3, 30, 0),
            );
            let stop_time_4 = stop_time(
                stop_points.get_idx("stop_point_4").unwrap(),
                0,
                Time::new(4, 0, 0),
            );
            let stop_time_5 = stop_time(
                stop_points.get_idx("stop_point_5").unwrap(),
                0,
                Time::new(5, 0, 0),
            );
            let vehicle_journey_1 = vehicle_journey(
                String::from("vehicle_journey_1"),
                vec![
                    stop_time_1.clone(),
                    stop_time_2.clone(),
                    stop_time_3.clone(),
                    stop_time_1_bis,
                    stop_time_4,
                ],
            );
            let vehicle_journey_2 = vehicle_journey(
                String::from("vehicle_journey_2"),
                vec![stop_time_1, stop_time_2, stop_time_3, stop_time_5],
            );
            let vehicle_journeys = vec![vehicle_journey_1, vehicle_journey_2];
            let route_points = build_route_points(&vehicle_journeys);
            assert_eq!(6, route_points.len());
            let stop_point = &stop_points[route_points[0]];
            assert_eq!("stop_point_1", stop_point.id.as_str());
            let stop_point = &stop_points[route_points[1]];
            assert_eq!("stop_point_2", stop_point.id.as_str());
            let stop_point = &stop_points[route_points[2]];
            assert_eq!("stop_point_3", stop_point.id.as_str());
            let stop_point = &stop_points[route_points[3]];
            assert_eq!("stop_point_1", stop_point.id.as_str());
            let stop_point = &stop_points[route_points[4]];
            assert_eq!("stop_point_4", stop_point.id.as_str());
            let stop_point = &stop_points[route_points[5]];
            assert_eq!("stop_point_5", stop_point.id.as_str());
        }

        // Use case:
        // One of the vehicle journeys has an extension in the middle compare to
        // the first vehicle journey.
        //
        // Expected result:
        // The stop points of the extended path stop point should be inserted in
        // the middle.
        #[test]
        fn extended_vehicle_journey() {
            let stop_points = stop_points();
            let stop_time_1 = stop_time(
                stop_points.get_idx("stop_point_1").unwrap(),
                0,
                Time::new(1, 0, 0),
            );
            let stop_time_2 = stop_time(
                stop_points.get_idx("stop_point_2").unwrap(),
                0,
                Time::new(2, 0, 0),
            );
            let stop_time_3 = stop_time(
                stop_points.get_idx("stop_point_3").unwrap(),
                0,
                Time::new(3, 0, 0),
            );
            let stop_time_4 = stop_time(
                stop_points.get_idx("stop_point_4").unwrap(),
                0,
                Time::new(4, 0, 0),
            );
            let vehicle_journey_1 = vehicle_journey(
                String::from("vehicle_journey_1"),
                vec![stop_time_1.clone(), stop_time_4.clone()],
            );
            let vehicle_journey_2 = vehicle_journey(
                String::from("vehicle_journey_2"),
                vec![stop_time_1, stop_time_2, stop_time_3, stop_time_4],
            );
            let vehicle_journeys = vec![vehicle_journey_1, vehicle_journey_2];
            let route_points = build_route_points(&vehicle_journeys);
            assert_eq!(4, route_points.len());
            let stop_point = &stop_points[route_points[0]];
            assert_eq!("stop_point_1", stop_point.id.as_str());
            let stop_point = &stop_points[route_points[1]];
            assert_eq!("stop_point_2", stop_point.id.as_str());
            let stop_point = &stop_points[route_points[2]];
            assert_eq!("stop_point_3", stop_point.id.as_str());
            let stop_point = &stop_points[route_points[3]];
            assert_eq!("stop_point_4", stop_point.id.as_str());
        }
    }
}
