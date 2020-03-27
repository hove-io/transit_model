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

use super::{accessibility::*, attribute_with::AttributeWith, EUROPE_PARIS_TIMEZONE};
use crate::{
    minidom_utils::{TryAttribute, TryOnlyChild},
    model::Collections,
    netex_utils,
    netex_utils::{FrameType, Frames},
    objects::{Coord, Equipment, KeysValues, StopArea, StopPoint, StopType},
    Result,
};
use failure::{bail, format_err, ResultExt};
use log::{info, warn, Level as LogLevel};
use minidom::Element;
use proj::Proj;
use skip_error::skip_error_and_log;
use std::{collections::HashMap, fs::File, io::Read};
use typed_index_collection::CollectionWithId;

pub fn extract_quay_id(raw_id: &str) -> Result<&str> {
    raw_id
        .split(':')
        .nth(3)
        .ok_or_else(|| format_err!("Cannot extract Quay identifier from '{}'", raw_id))
}

// Need to extract third and fourth part of the original identifier
pub fn extract_monomodal_stop_place_id(raw_id: &str) -> Result<&str> {
    let error = || {
        format_err!(
            "Cannot extract Monomodal StopPlace identifier from '{}'",
            raw_id
        )
    };
    let indices: Vec<_> = raw_id.match_indices(':').collect();
    let left_bound = indices.get(1).ok_or_else(error)?.0 + 1;
    let right_bound = indices.get(3).ok_or_else(error)?.0;
    Ok(&raw_id[left_bound..right_bound])
}

pub fn extract_multimodal_stop_place_id(raw_id: &str) -> Result<&str> {
    raw_id
        .split(':')
        .nth(3)
        .ok_or_else(|| format_err!("Cannot extract third part of identifier '{}'", raw_id))
}

// load a stop area
// coordinates will be computed with centroid of stop points if the stop area
// has no coordinates
fn load_stop_area(stop_place_elem: &Element, proj: &Proj) -> Result<StopArea> {
    let raw_stop_place_id: String = stop_place_elem.try_attribute("id")?;
    let is_monomodal_stop_place = raw_stop_place_id.contains(":monomodalStopPlace:");
    let id: String = if is_monomodal_stop_place {
        stop_place_elem.try_attribute_with("id", extract_monomodal_stop_place_id)?
    } else {
        stop_place_elem.try_attribute_with("id", extract_multimodal_stop_place_id)?
    };
    let coord: Coord = load_coords(stop_place_elem)
        .and_then(|coords| proj.convert(coords).map_err(|e| format_err!("{}", e)))
        .map(Coord::from)
        .unwrap_or_else(|e| {
            warn!("unable to parse coordinates of stop place {}: {}", id, e);
            Coord::default()
        });
    let mut codes = KeysValues::default();
    codes.insert((String::from("source"), raw_stop_place_id));
    Ok(StopArea {
        id,
        name: stop_place_elem
            .try_only_child("Name")?
            .text()
            .trim()
            .to_string(),
        visible: true,
        coord,
        codes,
        ..Default::default()
    })
}

// A stop area is a multimodal stop place or a monomodal stoplace
// with ParentSiteRef referencing a nonexistent multimodal stop place
fn load_stop_areas<'a>(
    stop_places: Vec<&'a Element>,
    map_stopplace_stoparea: &mut HashMap<String, String>,
    proj: &Proj,
) -> Result<CollectionWithId<StopArea>> {
    let mut stop_areas = CollectionWithId::default();

    let has_parent_site_ref = |sp: &Element| sp.try_only_child("ParentSiteRef").is_ok();

    for stop_place in stop_places.iter().filter(|sp| !has_parent_site_ref(sp)) {
        stop_areas.push(load_stop_area(stop_place, proj)?)?;
    }

    for stop_place in stop_places.iter().filter(|sp| has_parent_site_ref(sp)) {
        let parent_site_ref: String = stop_place
            .try_only_child("ParentSiteRef")?
            .try_attribute_with("ref", extract_multimodal_stop_place_id)?;

        let stop_place_id = stop_place.try_attribute_with("id", extract_monomodal_stop_place_id)?;
        if stop_areas.get(&parent_site_ref).is_some() {
            map_stopplace_stoparea.insert(stop_place_id, parent_site_ref.clone());
        } else {
            stop_areas.push(load_stop_area(stop_place, proj)?)?;
            map_stopplace_stoparea.insert(stop_place_id.clone(), stop_place_id);
        }
    }
    Ok(stop_areas)
}

fn load_coords(elem: &Element) -> Result<(f64, f64)> {
    let coords = elem
        .try_only_child("Centroid")?
        .try_only_child("Location")?
        .try_only_child("pos")?
        .text()
        .trim()
        .split_whitespace()
        .map(|n| n.parse())
        .collect::<std::result::Result<Vec<f64>, _>>();
    if let Ok(coords) = coords {
        if coords.len() == 2 {
            return Ok((coords[0], coords[1]));
        }
    }
    bail!("longitude and latitude not found")
}

fn stop_point_fare_zone_id(quay: &Element) -> Option<String> {
    quay.only_child("tariffZones")
        .and_then(|tariff_zones| tariff_zones.children().next())
        .and_then(|tariff_zone| tariff_zone.attribute::<String>("ref"))
        .and_then(|tzr| {
            tzr.split(':')
                .nth(2)
                .and_then(|zone| zone.parse::<u32>().ok())
        })
        .map(|v| v.to_string())
}

fn stop_point_parent_id(
    quay: &Element,
    map_refquay_stoparea: &HashMap<String, &String>,
    stop_areas: &CollectionWithId<StopArea>,
) -> Result<Option<String>> {
    Ok(quay
        .attribute_with::<_, _, String>("derivedFromObjectRef", extract_quay_id)
        .and_then(|refquay_id| map_refquay_stoparea.get(&refquay_id))
        .and_then(|stop_area_id| stop_areas.get(&stop_area_id))
        .map(|stop_area| stop_area.id.clone()))
}

pub fn get_or_create_equipment<'a>(
    quay: &Element,
    equipments: &'a mut HashMap<Accessibility, Equipment>,
    id_incr: &mut u8,
) -> Option<&'a Equipment> {
    let accessibility_node = quay.only_child("AccessibilityAssessment")?;
    let accessibility = accessibility(accessibility_node)?;

    let equipment = equipments.entry(accessibility.clone()).or_insert_with(|| {
        let Accessibility {
            wheelchair: wheelchair_boarding,
            visual_announcement,
            audible_announcement,
        } = accessibility;
        *id_incr += 1;
        Equipment {
            id: id_incr.to_string(),
            wheelchair_boarding,
            visual_announcement,
            audible_announcement,
            ..Default::default()
        }
    });
    Some(equipment)
}

fn load_stop_points<'a>(
    quays: Vec<&'a Element>,
    stop_areas: &mut CollectionWithId<StopArea>,
    map_stopplace_stoparea: &HashMap<String, String>,
    proj: &Proj,
) -> Result<(CollectionWithId<StopPoint>, CollectionWithId<Equipment>)> {
    let mut stop_points = CollectionWithId::default();
    let mut equipments: HashMap<Accessibility, Equipment> = HashMap::new();
    let mut id_incr = 0u8;

    let is_referential_quay = |quay: &Element| {
        quay.try_attribute::<String>("dataSourceRef")
            .map(|ds_ref| ds_ref == "FR1-ARRET_AUTO")
            .unwrap_or(false)
    };

    let map_refquay_stoparea: HashMap<_, _> = quays
        .iter()
        .filter(|quay| is_referential_quay(*quay))
        .flat_map(|quay| {
            let referential_quay_id: String = quay.attribute_with("id", extract_quay_id)?;
            let stop_place_id: String = quay
                .only_child("ParentZoneRef")?
                .attribute_with("ref", extract_monomodal_stop_place_id)?;
            let stop_area_id = map_stopplace_stoparea.get(&stop_place_id)?;
            Some((referential_quay_id, stop_area_id))
        })
        .collect();

    for quay in quays.iter().filter(|q| !is_referential_quay(*q)) {
        let raw_quay_id: String = quay.try_attribute("id")?;
        let id: String = quay.try_attribute_with("id", extract_quay_id)?;
        let coords = skip_error_and_log!(
            load_coords(quay).map_err(|e| format_err!(
                "unable to parse coordinates of quay {}: {}",
                id,
                e
            )),
            LogLevel::Warn
        );
        let mut codes = KeysValues::default();
        codes.insert((String::from("source"), raw_quay_id));

        let mut stop_point = StopPoint {
            id,
            name: quay.try_only_child("Name")?.text().trim().to_string(),
            visible: true,
            coord: proj.convert(coords).map(Coord::from)?,
            stop_area_id: "default_id".to_string(),
            timezone: Some(EUROPE_PARIS_TIMEZONE.to_string()),
            stop_type: StopType::Point,
            fare_zone_id: stop_point_fare_zone_id(quay),
            codes,
            ..Default::default()
        };

        let mut stop_point = if let Some(stop_area_id) =
            stop_point_parent_id(quay, &map_refquay_stoparea, &stop_areas)?
        {
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

        if let Some(associated_equipment) =
            get_or_create_equipment(quay, &mut equipments, &mut id_incr)
        {
            stop_point.equipment_id = Some(associated_equipment.id.clone());
        }

        stop_points.push(stop_point)?;
    }

    let mut equipments: Vec<_> = equipments.into_iter().map(|(_, e)| e).collect();
    equipments.sort_unstable_by(|eq1, eq2| eq1.id.cmp(&eq2.id));

    Ok((stop_points, CollectionWithId::new(equipments)?))
}

fn load_stops(
    frames: &Frames,
) -> Result<(
    CollectionWithId<StopArea>,
    CollectionWithId<StopPoint>,
    CollectionWithId<Equipment>,
)> {
    let member_children = || {
        frames
            .get(&FrameType::General)
            .into_iter()
            .flatten()
            .flat_map(|e| e.children())
            .filter(|e| e.name() == "members")
            .flat_map(|e| e.children())
    };

    let from = "EPSG:2154";
    let to = "+proj=longlat +datum=WGS84 +no_defs";
    let proj = Proj::new_known_crs(&from, &to, None)
        .ok_or_else(|| format_err!("Proj cannot build a converter from '{}' to '{}'", from, to))?;

    let mut map_stopplace_stoparea = HashMap::default();

    let mut stop_areas = load_stop_areas(
        member_children()
            .filter(|e| e.name() == "StopPlace")
            .collect(),
        &mut map_stopplace_stoparea,
        &proj,
    )?;

    let (stop_points, equipments) = load_stop_points(
        member_children().filter(|e| e.name() == "Quay").collect(),
        &mut stop_areas,
        &map_stopplace_stoparea,
        &proj,
    )?;

    Ok((stop_areas, stop_points, equipments))
}

pub fn from_path(path: &std::path::Path, collections: &mut Collections) -> Result<()> {
    info!("Reading {:?}", path);

    let mut file = File::open(&path).with_context(|_| format!("Error reading {:?}", path))?;
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

    mod fare_zone_id {
        use super::*;

        #[test]
        fn test_no_tarif_zones() {
            let xml = "<Quay></Quay>";
            let quay: Element = xml.parse().unwrap();
            assert_eq!(None, stop_point_fare_zone_id(&quay));
        }

        #[test]
        fn test_zone_not_found() {
            let xml = r#"
    <Quay>
        <tariffZones>
            <TariffZoneRef ref="FR1:unvalid"/>
        </tariffZones>
    </Quay>"#;
            let quay: Element = xml.parse().unwrap();
            assert_eq!(None, stop_point_fare_zone_id(&quay));
        }

        #[test]
        fn test_zone_not_integer() {
            let xml = r#"
    <Quay>
        <tariffZones>
            <TariffZoneRef ref="FR1:TariffZone:not_integer:LOC"/>
        </tariffZones>
    </Quay>"#;
            let quay: Element = xml.parse().unwrap();
            assert_eq!(None, stop_point_fare_zone_id(&quay));
        }

        #[test]
        fn test_one_good_zone() {
            let xml = r#"
    <Quay>
        <tariffZones>
            <TariffZoneRef ref="FR1:TariffZone:2:LOC"/>
        </tariffZones>
    </Quay>"#;
            let quay: Element = xml.parse().unwrap();
            assert_eq!(Some("2".to_string()), stop_point_fare_zone_id(&quay));
        }

        #[test]
        fn test_first_zone() {
            let xml = r#"
    <Quay>
        <tariffZones>
            <TariffZoneRef ref="FR1:TariffZone:9:LOC"/>
            <TariffZoneRef ref="FR1:TariffZone:1:LOC"/>
        </tariffZones>
    </Quay>"#;
            let quay: Element = xml.parse().unwrap();
            assert_eq!(Some("9".to_string()), stop_point_fare_zone_id(&quay));
        }
    }
}
