// Copyright 2017-2018 Kisio Digital and/or its affiliates.
//
// This program is free software: you can redistribute it and/or
// modify it under the terms of the GNU General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see
// <http://www.gnu.org/licenses/>.

//! [NTFS](https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/ntfs_fr.md)
//! format management.

use collection::{Collection, CollectionWithId, Idx};
use objects::{StopPoint, Transfer};
use std::collections::HashSet;

fn make_transfers_set(
    transfers: &Collection<Transfer>,
    sp: &CollectionWithId<StopPoint>,
) -> HashSet<(Idx<StopPoint>, Idx<StopPoint>)> {
    transfers
        .values()
        .map(|t| {
            (
                sp.get_idx(&t.from_stop_id).unwrap(),
                sp.get_idx(&t.to_stop_id).unwrap(),
            )
        }).collect()
}

/// Generates missing transfers
///
/// The `max_distance` argument allows you to specify the max distance
/// in meters to compute the tranfer.
///
/// The `walking_speed` argument is the walking speed in meters per second.
pub fn generates_transfers(
    transfers: &mut Collection<Transfer>,
    stop_points: &CollectionWithId<StopPoint>,
    max_distance: f64,
    walking_speed: f64,
    waiting_time: u32,
) {
    let transfers_set = make_transfers_set(&transfers, &stop_points);
    let sq_max_distance = max_distance * max_distance;
    for (idx1, sp1) in stop_points {
        let approx = sp1.coord.approx();
        for (_, sp2) in stop_points
            .iter()
            .filter(|&(idx2, _)| !transfers_set.contains(&(idx1, idx2)))
        {
            let sq_distance = approx.sq_distance_to(&sp2.coord);
            if sq_distance > sq_max_distance {
                continue;
            }
            let transfer_time = (sq_distance.sqrt() / walking_speed) as u32;
            transfers.push(Transfer {
                from_stop_id: sp1.id.clone(),
                to_stop_id: sp2.id.clone(),
                min_transfer_time: Some(transfer_time),
                real_min_transfer_time: Some(transfer_time + waiting_time),
                equipment_id: None,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use collection::{Collection, CollectionWithId};
    use objects::*;

    #[test]
    //                    206m
    // sp_1 *--------------------------------* sp_3
    //       \                        ______/
    //        \                  ____/
    //   65m   \           _____/   146m
    //          \    _____/
    //           \__/
    //           sp_2
    //
    fn test_generates_transfers() {
        let mut transfers = Collection::new(vec![
            Transfer {
                from_stop_id: "sp_1".to_string(),
                to_stop_id: "sp_2".to_string(),
                min_transfer_time: Some(50),
                real_min_transfer_time: Some(60),
                equipment_id: None,
            },
            Transfer {
                from_stop_id: "sp_1".to_string(),
                to_stop_id: "sp_3".to_string(),
                min_transfer_time: Some(200),
                real_min_transfer_time: Some(210),
                equipment_id: None,
            },
        ]);

        let stop_points = CollectionWithId::new(vec![
            StopPoint {
                id: "sp_1".to_string(),
                name: "sp_name_1".to_string(),
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                visible: true,
                coord: Coord {
                    lon: 2.372075915336609,
                    lat: 48.84608210211328,
                },
                timezone: None,
                geometry_id: None,
                equipment_id: None,
                stop_area_id: "sa_1".to_string(),
                fare_zone_id: None,
            },
            StopPoint {
                id: "sp_2".to_string(),
                name: "sa_name_2".to_string(),
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                visible: true,
                coord: Coord {
                    lon: 2.371437549591065,
                    lat: 48.845665532277096,
                },
                timezone: None,
                geometry_id: None,
                equipment_id: None,
                stop_area_id: "sa_1".to_string(),
                fare_zone_id: None,
            },
            StopPoint {
                id: "sp_3".to_string(),
                name: "sa_name_3".to_string(),
                codes: KeysValues::default(),
                object_properties: KeysValues::default(),
                comment_links: CommentLinksT::default(),
                visible: true,
                coord: Coord {
                    lon: 2.369517087936402,
                    lat: 48.845301913401144,
                },
                timezone: None,
                geometry_id: None,
                equipment_id: None,
                stop_area_id: "sa_1".to_string(),
                fare_zone_id: None,
            },
        ]).unwrap();

        super::generates_transfers(&mut transfers, &stop_points, 100.0, 0.785, 120);
        let transfers = transfers.values().collect::<Vec<_>>();

        //we keep the 2 first existing transfers
        // transfers sp_2 -> sp_3, sp_3 -> sp_2, sp_3 -> sp_1 are not added,
        // because distances between them are > 100m
        // sp_1 -> sp_3 is kept because it is an existing transfer.
        assert_eq!(
            transfers,
            vec![
                &Transfer {
                    from_stop_id: "sp_1".to_string(),
                    to_stop_id: "sp_2".to_string(),
                    min_transfer_time: Some(50),
                    real_min_transfer_time: Some(60),
                    equipment_id: None,
                },
                &Transfer {
                    from_stop_id: "sp_1".to_string(),
                    to_stop_id: "sp_3".to_string(),
                    min_transfer_time: Some(200),
                    real_min_transfer_time: Some(210),
                    equipment_id: None,
                },
                &Transfer {
                    from_stop_id: "sp_1".to_string(),
                    to_stop_id: "sp_1".to_string(),
                    min_transfer_time: Some(0),
                    real_min_transfer_time: Some(120),
                    equipment_id: None,
                },
                &Transfer {
                    from_stop_id: "sp_2".to_string(),
                    to_stop_id: "sp_1".to_string(),
                    min_transfer_time: Some(83),
                    real_min_transfer_time: Some(203),
                    equipment_id: None,
                },
                &Transfer {
                    from_stop_id: "sp_2".to_string(),
                    to_stop_id: "sp_2".to_string(),
                    min_transfer_time: Some(0),
                    real_min_transfer_time: Some(120),
                    equipment_id: None,
                },
                &Transfer {
                    from_stop_id: "sp_3".to_string(),
                    to_stop_id: "sp_3".to_string(),
                    min_transfer_time: Some(0),
                    real_min_transfer_time: Some(120),
                    equipment_id: None,
                },
            ]
        );
    }
}
