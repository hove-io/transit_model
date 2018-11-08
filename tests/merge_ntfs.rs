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

extern crate navitia_model;
use navitia_model::collection::CollectionWithId;
use navitia_model::collection::Idx;
use navitia_model::model::Collections;
use navitia_model::objects::StopPoint;
use navitia_model::objects::VehicleJourney;

#[test]
#[should_panic(expected = "TGC already found")] // first collision is on contributor id
fn merge_collections_with_collisions() {
    let mut collections = Collections::default();
    let input_collisions = ["fixtures/ntfs", "fixtures/ntfs"];
    for input_directory in input_collisions.iter() {
        let to_append_model = navitia_model::ntfs::read(input_directory).unwrap();
        collections
            .merge(to_append_model.into_collections())
            .unwrap();
    }
}

#[test]
fn merge_collections_ok() {
    let mut collections = Collections::default();
    let input_collisions = ["fixtures/ntfs", "fixtures/merge-ntfs"];
    for input_directory in input_collisions.iter() {
        let to_append_model = navitia_model::ntfs::read(input_directory).unwrap();

        collections
            .merge(to_append_model.into_collections())
            .unwrap();
    }
    assert_eq!(collections.contributors.len(), 2);
    assert_eq!(collections.datasets.len(), 2);
    assert_eq!(collections.networks.len(), 3);
    assert_eq!(collections.commercial_modes.len(), 6);
    assert_eq!(collections.lines.len(), 6);
    assert_eq!(collections.routes.len(), 8);
    assert_eq!(collections.vehicle_journeys.len(), 8);

    fn get_stop_point_idxs(
        col: &CollectionWithId<VehicleJourney>,
        id: &str,
    ) -> Vec<Idx<StopPoint>> {
        col.get(id)
            .unwrap()
            .stop_times
            .iter()
            .map(|st| st.stop_point_idx)
            .collect()
    }

    assert_eq!(
        get_stop_point_idxs(&collections.vehicle_journeys, "RERAB1"),
        vec![
            collections.stop_points.get_idx("DEFR").unwrap(),
            collections.stop_points.get_idx("CDGR").unwrap(),
            collections.stop_points.get_idx("GDLR").unwrap(),
            collections.stop_points.get_idx("NATR").unwrap(),
        ]
    );
    assert_eq!(
        get_stop_point_idxs(&collections.vehicle_journeys, "OIF:77100911-1_1420-1"),
        vec![
            collections.stop_points.get_idx("OIF:SP:10:10").unwrap(),
            collections.stop_points.get_idx("OIF:SP:10:100").unwrap()
        ]
    );
    assert_eq!(collections.physical_modes.len(), 6);
    assert_eq!(collections.stop_areas.len(), 7);
    assert_eq!(collections.stop_points.len(), 12);
    assert_eq!(collections.feed_infos.len(), 10);
    let calendar_vec = collections.calendars.into_vec();
    assert_eq!(calendar_vec[0].dates.len(), 261);
    assert_eq!(calendar_vec[1].dates.len(), 6);
    assert_eq!(collections.companies.len(), 3);
    assert_eq!(collections.comments.len(), 6);
    assert_eq!(collections.equipments.len(), 0);
    assert_eq!(collections.transfers.len(), 0);
    assert_eq!(collections.trip_properties.len(), 0);
    assert_eq!(collections.geometries.len(), 0);
    assert_eq!(collections.admin_stations.len(), 0);
}
