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

use super::EUROPE_PARIS_TIMEZONE;
use crate::{
    common_format::Availability,
    minidom_utils::{TryAttribute, TryOnlyChild},
    model::Collections,
    netex_utils,
    netex_utils::{FrameType, Frames},
    objects::{Codes, Coord, Equipment, StopArea, StopPoint, StopType},
    Result,
};
use failure::{bail, format_err, ResultExt};
use log::{debug, info, warn};
use minidom::Element;
use proj::Proj;
use std::{
    collections::{BTreeSet, HashMap},
    fs::File,
    io::Read,
};
use transit_model_collection::CollectionWithId;

// load a stop area with wrong coordinates
// coordinates will be copmuted with centroid of stop points
fn load_stop_area(stop_place_elem: &Element, id: String) -> Result<StopArea> {
    // add object property
    let mut object_properties = BTreeSet::default();
    let type_of_place_ref: String = stop_place_elem
        .try_only_child("placeTypes")?
        .try_only_child("TypeOfPlaceRef")?
        .try_attribute("ref")?;
    object_properties.insert(("Netex_StopType".to_string(), type_of_place_ref));

    Ok(StopArea {
        id,
        name: stop_place_elem
            .try_only_child("Name")?
            .text()
            .trim()
            .to_string(),
        visible: true,
        object_properties,
        ..Default::default()
    })
}

// A stop area is a LDA or a ZDE without ParentSiteRef
fn load_stop_areas<'a>(
    stop_places: impl Iterator<Item = &'a &'a Element>,
    map_lda_zde: &mut HashMap<String, String>,
    map_quay_lda: &mut HashMap<String, String>,
) -> Result<CollectionWithId<StopArea>> {
    let mut stop_areas = CollectionWithId::default();
    for stop_place in stop_places {
        let id = stop_place.try_attribute("id")?;

        // ZDL with ParentSiteRef
        if let Ok(parent_site_ref) = stop_place.try_only_child("ParentSiteRef") {
            let parent_site_ref: String = parent_site_ref.try_attribute("ref")?;
            map_lda_zde.insert(parent_site_ref.clone(), id);

            for quay_ref in stop_place.try_only_child("quays")?.children() {
                map_quay_lda.insert(quay_ref.try_attribute("ref")?, parent_site_ref.clone());
            }

            continue;
        }

        if stop_place
            .try_only_child("placeTypes")?
            .try_only_child("TypeOfPlaceRef")?
            .try_attribute::<String>("ref")?
            == "ZDL"
        {
            for quay_ref in stop_place.try_only_child("quays")?.children() {
                map_quay_lda.insert(quay_ref.try_attribute("ref")?, id.clone());
            }
        }

        stop_areas.push(load_stop_area(stop_place, id)?)?;
    }

    Ok(stop_areas)
}

fn add_stop_area_codes(
    stop_areas: &mut CollectionWithId<StopArea>,
    map_lda_zde: HashMap<String, String>,
) {
    for (lda_id, zde_id) in map_lda_zde {
        if let Some(mut sa) = stop_areas.get_mut(&lda_id) {
            sa.codes_mut().insert(("Netex_ZDL".to_string(), zde_id));
        } else {
            warn!(
                "parent LDA (stop area) {} for ZDE {} not found",
                lda_id, zde_id
            );
        }
    }
}

fn load_coords(quay: &Element) -> Result<(f64, f64)> {
    let gml_pos = quay
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

use geo::algorithm::centroid::Centroid;
use geo_types::MultiPoint;

fn update_stop_area_coords(
    stop_areas: &mut CollectionWithId<StopArea>,
    stop_points: &CollectionWithId<StopPoint>,
) {
    let mut updated_stop_areas = stop_areas.take();
    for stop_area in &mut updated_stop_areas {
        if let Some(coord) = stop_points
            .values()
            .filter(|sp| sp.stop_area_id == stop_area.id)
            .map(|sp| (sp.coord.lon, sp.coord.lat))
            .collect::<MultiPoint<_>>()
            .centroid()
            .map(|c| Coord {
                lon: c.x(),
                lat: c.y(),
            })
        {
            stop_area.coord = coord;
        } else {
            debug!("failed to calculate a centroid of stop area {} because it does not refer to any corresponding stop point", stop_area.id);
        }
    }

    // this does not fail as updated_stop_areas comes from a CollectionWithId
    // and stop area ids have not been modified
    *stop_areas = CollectionWithId::new(updated_stop_areas).unwrap();
}

fn avaibility(quay: &Element) -> Result<Availability> {
    let avaibility = match quay
        .try_only_child("AccessibilityAssessment")?
        .try_only_child("MobilityImpairedAccess")?
        .text()
        .trim()
    {
        "true" => Availability::Available,
        "false" => Availability::NotAvailable,
        _ => Availability::InformationNotAvailable,
    };

    Ok(avaibility)
}

fn get_or_create_equipment<'a>(
    quay: &Element,
    equipments: &'a mut HashMap<Availability, Equipment>,
    id_incr: &mut u8,
) -> Result<&'a mut Equipment> {
    let avaibility = avaibility(quay)?;
    let equipment = equipments.entry(avaibility).or_insert_with(|| {
        *id_incr += 1;
        Equipment {
            id: id_incr.to_string(),
            wheelchair_boarding: avaibility,
            ..Default::default()
        }
    });
    Ok(equipment)
}

fn load_stop_points<'a>(
    quays: impl Iterator<Item = &'a &'a Element>,
    stop_areas: &mut CollectionWithId<StopArea>,
    map_quay_lda: &mut HashMap<String, String>,
) -> Result<(CollectionWithId<StopPoint>, CollectionWithId<Equipment>)> {
    let mut stop_points = CollectionWithId::default();
    let mut equipments: HashMap<Availability, Equipment> = HashMap::new();
    let mut id_incr = 0u8;
    let from = "EPSG:2154";
    let to = "+proj=longlat +datum=WGS84 +no_defs";
    let proj = Proj::new_known_crs(&from, &to, None)
        .ok_or_else(|| format_err!("Proj cannot build a converter from '{}' to '{}'", from, to))?;

    for quay in quays {
        let id: String = quay.try_attribute("id")?;
        let coords = skip_fail!(load_coords(quay).map_err(|e| format_err!(
            "unable to parse coordinates of quay {}: {}",
            id,
            e
        )));

        let mut stop_point = StopPoint {
            id: quay.try_attribute("id")?,
            name: quay.try_only_child("Name")?.text().trim().to_string(),
            visible: true,
            coord: proj.convert((coords.0, coords.1).into()).map(Coord::from)?,
            stop_area_id: "default_id".to_string(),
            timezone: Some(EUROPE_PARIS_TIMEZONE.to_string()),
            stop_type: StopType::Point,
            ..Default::default()
        };

        let mut stop_point = if let Some(stop_area_id) =
            map_quay_lda.get(&id).and_then(|stop_area_id| {
                stop_areas
                    .get(&stop_area_id)
                    .map(|_| stop_area_id.to_string())
            }) {
            StopPoint {
                stop_area_id,
                ..stop_point
            }
        } else {
            let stop_area = StopArea::from(stop_point.clone());
            stop_point.stop_area_id = stop_area.id.clone();
            stop_areas.push(stop_area)?;
            stop_point
        };

        let associated_equipment = get_or_create_equipment(quay, &mut equipments, &mut id_incr)?;
        stop_point.equipment_id = Some(associated_equipment.id.clone());

        stop_points.push(stop_point)?;
    }

    let mut equipments: Vec<_> = equipments.into_iter().map(|(_, e)| e).collect();
    equipments.sort_unstable_by(|tp1, tp2| tp1.id.cmp(&tp2.id));

    Ok((stop_points, CollectionWithId::new(equipments)?))
}

fn load_stops(
    frames: &Frames,
) -> Result<(
    CollectionWithId<StopArea>,
    CollectionWithId<StopPoint>,
    CollectionWithId<Equipment>,
)> {
    let member_children: Vec<_> = frames
        .get(&FrameType::General)
        .unwrap_or(&vec![])
        .iter()
        .flat_map(|e| e.children())
        .filter(|e| e.name() == "members")
        .flat_map(|e| e.children())
        .collect();

    // for stop areas's object codes
    let mut map_lda_zde: HashMap<String, String> = HashMap::default();
    // relation between a stop point (quay) and its parent stop area (lda)
    let mut map_quay_lda: HashMap<String, String> = HashMap::default();

    let mut stop_areas = load_stop_areas(
        member_children.iter().filter(|e| e.name() == "StopPlace"),
        &mut map_lda_zde,
        &mut map_quay_lda,
    )?;
    add_stop_area_codes(&mut stop_areas, map_lda_zde);

    let (stop_points, equipments) = load_stop_points(
        member_children.iter().filter(|e| e.name() == "Quay"),
        &mut stop_areas,
        &mut map_quay_lda,
    )?;

    update_stop_area_coords(&mut stop_areas, &stop_points);

    Ok((stop_areas, stop_points, equipments))
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

    let (stop_areas, stop_points, equipments) = load_stops(&frames)?;

    collections.stop_areas.try_merge(stop_areas)?;
    collections.stop_points.try_merge(stop_points)?;
    collections.equipments.try_merge(equipments)?;

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

    mod test_avaibility {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn test_available() {
            let xml = r#"
<Quay>
    <AccessibilityAssessment>
        <MobilityImpairedAccess>true</MobilityImpairedAccess>
    </AccessibilityAssessment>
</Quay>"#;

            let quay: Element = xml.parse().unwrap();
            let avaibility = avaibility(&quay).unwrap();

            assert_eq!(Availability::Available, avaibility);
        }

        #[test]
        fn test_not_available() {
            let xml = r#"
<Quay>
    <AccessibilityAssessment>
        <MobilityImpairedAccess>false</MobilityImpairedAccess>
    </AccessibilityAssessment>
</Quay>"#;

            let quay: Element = xml.parse().unwrap();
            let avaibility = avaibility(&quay).unwrap();

            assert_eq!(Availability::NotAvailable, avaibility);
        }

        #[test]
        fn test_information_not_available() {
            let xml = r#"
<Quay>
    <AccessibilityAssessment>
        <MobilityImpairedAccess>whatever</MobilityImpairedAccess>
    </AccessibilityAssessment>
</Quay>"#;

            let quay: Element = xml.parse().unwrap();
            let avaibility = avaibility(&quay).unwrap();

            assert_eq!(Availability::InformationNotAvailable, avaibility);
        }

        #[test]
        #[should_panic]
        fn test_fail() {
            let xml = r#"
<Quay>
    <AccessibilityAssessment>
        NoMobilityImpairedAccessNode
    </AccessibilityAssessment>
</Quay>"#;

            let quay: Element = xml.parse().unwrap();
            avaibility(&quay).unwrap();
        }
    }
}
