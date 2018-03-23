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
use navitia_model::objects::*;
use navitia_model::collection::{CollectionWithId, Id, Idx};
use navitia_model::relations::IdxSet;
use navitia_model::model::{GetCorresponding, Model};

fn get<T, U>(idx: Idx<T>, collection: &CollectionWithId<U>, objects: &Model) -> Vec<String>
where
    U: Id<U>,
    IdxSet<T>: GetCorresponding<U>,
{
    objects
        .get_corresponding_from_idx(idx)
        .iter()
        .map(|idx| collection[*idx].id().to_string())
        .collect()
}

#[test]
fn minimal() {
    let ntm = navitia_model::ntfs::read("fixtures/minimal_ntfs/").unwrap();

    assert_eq!(6, ntm.stop_areas.len());
    assert_eq!(10, ntm.stop_points.len());
    assert_eq!(3, ntm.commercial_modes.len());
    assert_eq!(3, ntm.lines.len());
    assert_eq!(6, ntm.routes.len());
    assert_eq!(3, ntm.physical_modes.len());
    assert_eq!(6, ntm.vehicle_journeys.len());
    assert_eq!(1, ntm.networks.len());
    assert_eq!(1, ntm.companies.len());
    assert_eq!(1, ntm.contributors.len());
    assert_eq!(1, ntm.datasets.len());
    assert_eq!(0, ntm.geometries.len());

    let gdl = ntm.stop_areas.get_idx("GDL").unwrap();
    assert_eq!(3, ntm.get_corresponding_from_idx::<_, StopPoint>(gdl).len());
    assert_eq!(
        get(gdl, &ntm.physical_modes, &ntm),
        &["Bus", "Metro", "RapidTransit"]
    );
    assert_eq!(
        get(gdl, &ntm.commercial_modes, &ntm),
        &["Bus", "Metro", "RER"]
    );
    assert_eq!(get(gdl, &ntm.networks, &ntm), &["TGN"]);
    assert_eq!(get(gdl, &ntm.contributors, &ntm), &["TGC"]);

    let rera = ntm.lines.get_idx("RERA").unwrap();
    assert_eq!(
        get(rera, &ntm.physical_modes, &ntm),
        &["Bus", "RapidTransit"]
    );
    assert_eq!(get(rera, &ntm.commercial_modes, &ntm), &["RER"]);
    assert_eq!(get(rera, &ntm.networks, &ntm), &["TGN"]);
    assert_eq!(get(rera, &ntm.contributors, &ntm), &["TGC"]);
    assert_eq!(get(rera, &ntm.routes, &ntm), &["RERAF", "RERAB"]);
    assert_eq!(
        get(rera, &ntm.vehicle_journeys, &ntm),
        &["RERAF1", "RERAB1"]
    );
    assert_eq!(
        get(rera, &ntm.stop_points, &ntm),
        &["GDLR", "NATR", "CDGR", "DEFR"]
    );
    assert_eq!(
        get(rera, &ntm.stop_areas, &ntm),
        &["GDL", "NAT", "CDG", "DEF"]
    );
}

#[test]
fn ntfs() {
    let pt_objects = navitia_model::ntfs::read("fixtures/ntfs/").unwrap();

    // comments
    assert_eq!(1, pt_objects.comments.len());
    let rera_lines_idx = pt_objects.lines.get_idx("RERA").unwrap();
    let rera_comment_indexes = &pt_objects.lines[rera_lines_idx].comment_links;
    for comment in pt_objects.comments.iter_from(rera_comment_indexes) {
        assert_eq!(comment.id.to_string(), "RERACOM1");
    }
}
