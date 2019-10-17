// Copyright 2017 Kisio Digital and/or its affiliates.
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

use crate::{
    minidom_utils::{TryAttribute, TryOnlyChild},
    model::Collections,
    netex_utils,
    netex_utils::{FrameType, Frames},
    objects::{Coord, StopArea},
    Result,
};
use failure::{bail, format_err, ResultExt};
use log::{info, warn};
use minidom::Element;
use proj::Proj;
use std::{fs::File, io::Read};
use transit_model_collection::CollectionWithId;

// load a stop area
// coordinates will be computed with centroid of stop points if the stop area
// has no coordinates
fn load_stop_area(stop_place_elem: &Element, proj: &Proj) -> Result<StopArea> {
    let id: String = stop_place_elem.try_attribute("id")?;
    let coord: Coord = load_coords(stop_place_elem)
        .and_then(|coords| proj.convert(coords.into()))
        .map(Coord::from)
        .unwrap_or_else(|e| {
            warn!("unable to parse coordinates of stop place {}: {}", id, e);
            Coord::default()
        });

    Ok(StopArea {
        id,
        name: stop_place_elem
            .try_only_child("Name")?
            .text()
            .trim()
            .to_string(),
        visible: true,
        coord,
        ..Default::default()
    })
}

// A stop area is a multimodal stop place or a monomodal stoplace
// with ParentSiteRef referencing a nonexistent multimodal stop place
fn load_stop_areas<'a>(
    stop_places: Vec<&'a Element>,
    proj: &Proj,
) -> Result<CollectionWithId<StopArea>> {
    let mut stop_areas = CollectionWithId::default();

    let has_parent_site_ref = |sp: &Element| sp.try_only_child("ParentSiteRef").is_ok();

    for stop_place in stop_places.iter().filter(|sp| !has_parent_site_ref(sp)) {
        stop_areas.push(load_stop_area(stop_place, proj)?)?;
    }

    for stop_place in stop_places.iter().filter(|sp| has_parent_site_ref(sp)) {
        let id: String = stop_place
            .try_only_child("ParentSiteRef")?
            .try_attribute("ref")?;

        if stop_areas.get(&id).is_none() {
            stop_areas.push(load_stop_area(stop_place, proj)?)?;
        }
    }

    Ok(stop_areas)
}

fn load_coords(elem: &Element) -> Result<(f64, f64)> {
    let gml_pos = elem
        .try_only_child("Centroid")?
        .try_only_child("Location")?
        .try_only_child("pos")?
        .text()
        .trim()
        .to_string();
    let coords: Vec<&str> = gml_pos.split_whitespace().collect();
    if coords.len() != 2 {
        bail!("longitude and latitude not found");
    }

    Ok((coords[0].parse()?, coords[1].parse()?))
}

fn load_stops(frames: &Frames) -> Result<CollectionWithId<StopArea>> {
    let stop_places: Vec<_> = frames
        .get(&FrameType::General)
        .unwrap_or(&vec![])
        .iter()
        .flat_map(|e| e.children())
        .filter(|e| e.name() == "members")
        .flat_map(|e| e.children())
        .filter(|e| e.name() == "StopPlace")
        .collect();

    let from = "EPSG:2154";
    let to = "+proj=longlat +datum=WGS84 +no_defs";
    let proj = Proj::new_known_crs(&from, &to, None)
        .ok_or_else(|| format_err!("Proj cannot build a converter from '{}' to '{}'", from, to))?;

    let stop_areas = load_stop_areas(stop_places, &proj)?;

    Ok(stop_areas)
}

pub fn from_path(path: &std::path::Path, collections: &mut Collections) -> Result<()> {
    info!("reading {:?}", path);

    let mut file = File::open(&path).with_context(ctx_from_path!(path))?;
    let mut file_content = String::new();
    file.read_to_string(&mut file_content)?;
    let root: Element = file_content
        .parse()
        .map_err(|e| format_err!("Failed to parse file '{:?}': {}", path, e))?;
    let frames = netex_utils::parse_frames_by_type(
        root.try_only_child("dataObjects")?
            .try_only_child("CompositeFrame")?
            .try_only_child("frames")?,
    )?;

    let stop_areas = load_stops(&frames)?;

    collections.stop_areas.try_merge(stop_areas)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    mod test_coords {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        #[should_panic(expected = "longitude and latitude not found")]
        fn test_load_coords_with_one_coord() {
            let xml = r#"
<Quay>
    <Centroid>
        <Location>
            <gml:pos srsName="EPSG:2154">666944.0</gml:pos>
        </Location>
    </Centroid>
</Quay>"#;
            let root: Element = xml.parse().unwrap();
            load_coords(&root).unwrap();
        }

        #[test]
        #[should_panic]
        fn test_load_unvalid_coords() {
            let xml = r#"
<Quay>
    <Centroid>
        <Location>
            <gml:pos srsName="EPSG:2154">666944.0 ABC</gml:pos>
        </Location>
    </Centroid>
</Quay>"#;
            let root: Element = xml.parse().unwrap();
            load_coords(&root).unwrap();
        }

        #[test]
        fn test_load_coords() {
            let xml = r#"
<Quay>
    <Centroid>
        <Location>
            <gml:pos srsName="EPSG:2154">666944.0 6856019.0</gml:pos>
        </Location>
    </Centroid>
</Quay>"#;
            let root: Element = xml.parse().unwrap();
            let coords = load_coords(&root).unwrap();

            assert_eq!((666944.0, 6856019.0), coords);
        }
    }
}
