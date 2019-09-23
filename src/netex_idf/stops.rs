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
    minidom_utils::{TryAttribute, TryOnlyChild},
    model::Collections,
    objects::{Codes, Coord, Properties, StopArea, StopPoint, StopType},
    Result,
};
use failure::{bail, format_err, ResultExt};
use log::{info, warn};
use minidom::Element;
use proj::Proj;
use std::{collections::HashMap, fs::File, io::Read};
use transit_model_collection::CollectionWithId;

fn load_stop_area(stop_place_elem: &Element, id: String) -> Result<StopArea> {
    // LDA or ZDL without ParentSiteRef
    let mut stop_area = StopArea {
        id,
        name: stop_place_elem
            .try_only_child("Name")?
            .text()
            .trim()
            .to_string(),
        visible: true,
        coord: Coord { lon: 0., lat: 0. },
        ..Default::default()
    };

    // add object properties
    let type_of_place_ref: String = stop_place_elem
        .try_only_child("placeTypes")?
        .try_only_child("TypeOfPlaceRef")?
        .try_attribute("ref")?;
    stop_area
        .properties_mut()
        .insert(("Netex_StopType".to_string(), type_of_place_ref));

    Ok(stop_area)
}

// A stop area is a LDA or a ZDE without ParentSiteRef
fn load_stop_areas<'a>(
    members: &[&'a Element],
    map_lda_zde: &mut HashMap<String, String>,
    map_quay_lda: &mut HashMap<String, String>,
) -> Result<CollectionWithId<StopArea>> {
    let mut stop_areas = CollectionWithId::default();
    for stop_place in members.iter().filter(|e| e.name() == "StopPlace") {
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

fn load_stop_points<'a>(
    members: &[&'a Element],
    stop_areas: &mut CollectionWithId<StopArea>,
    map_quay_lda: &mut HashMap<String, String>,
) -> Result<CollectionWithId<StopPoint>> {
    let mut stop_points = CollectionWithId::default();
    // add stop points
    let from = "EPSG:2154";
    let to = "+proj=longlat +datum=WGS84 +no_defs";
    let proj = Proj::new_known_crs(&from, &to, None)
        .ok_or_else(|| format_err!("Proj cannot build a converter from '{}' to '{}'", from, to))?;

    for quay in members.iter().filter(|e| e.name() == "Quay") {
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

        let stop_point = if let Some(stop_area_id) =
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

        stop_points.push(stop_point)?;
    }

    Ok(stop_points)
}

fn load_stops(elem: &Element) -> Result<(CollectionWithId<StopArea>, CollectionWithId<StopPoint>)> {
    let member_children: Vec<_> = elem
        .try_only_child("dataObjects")?
        .try_only_child("CompositeFrame")?
        .try_only_child("frames")?
        .children()
        .flat_map(|e| e.children())
        .filter(|e| e.name() == "members")
        .flat_map(|e| e.children())
        .collect();

    // for stop areas's object codes
    let mut map_lda_zde: HashMap<String, String> = HashMap::default();
    // relation between a stop point (quay) and its parent stop area (lda)
    let mut map_quay_lda: HashMap<String, String> = HashMap::default();

    let mut stop_areas = load_stop_areas(&member_children, &mut map_lda_zde, &mut map_quay_lda)?;
    add_stop_area_codes(&mut stop_areas, map_lda_zde);

    let stop_points = load_stop_points(&member_children, &mut stop_areas, &mut map_quay_lda)?;

    Ok((stop_areas, stop_points))
}

pub fn from_path(path: &std::path::Path, collections: &mut Collections) -> Result<()> {
    info!("reading {:?}", path);

    let mut file = File::open(&path).with_context(ctx_from_path!(path))?;
    let mut file_content = String::new();
    file.read_to_string(&mut file_content)?;
    let elem = file_content.parse::<Element>();

    let (stop_areas, stop_points) = elem
        .map_err(|e| format_err!("Failed to parse file '{:?}': {}", path, e))
        .and_then(|ref e| load_stops(e))?;

    collections.stop_areas.try_merge(stop_areas)?;
    collections.stop_points.try_merge(stop_points)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    #[should_panic]
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

    #[test]
    fn test_load_stop_areas() {
        let xml = r#"
<root>
	<dataObjects>
		<CompositeFrame>
			<frames>
				<GeneralFrame>
					<members>
						<StopPlace id="FR:78686:LDA:422420:STIF">
							<Name>Viroflay Gare Rive Droite</Name>
							<placeTypes>
								<TypeOfPlaceRef ref="LDA"/>
							</placeTypes>
							<StopPlaceType>railStation</StopPlaceType>
						</StopPlace>
						<StopPlace  id="FR:28140:LDA:74325:STIF">
							<Name>Prairie</Name>
							<placeTypes>
								<TypeOfPlaceRef ref="LDA"/>
							</placeTypes>
							<StopPlaceType>onstreetBus</StopPlaceType>
						</StopPlace>
                        <StopPlace id="FR:78423:ZDL:57857:STIF">>
                            <Name>Fort de Saint-Cyr</Name>
                            <placeTypes>
                                <TypeOfPlaceRef ref="ZDL"/>
                            </placeTypes>
                            <ParentSiteRef ref="FR:78686:LDA:422420:STIF" />
                            <quays>
                                <QuayRef ref="FR:94017:ZDE:50125579:STIF" />
                                <QuayRef ref="FR:92050:ZDE:50028126:STIF" />
                            </quays>
                        </StopPlace>
                        <StopPlace id="FR:0:ZDL:50057134:STIF">
                            <Name>CONVENTION</Name>
                            <placeTypes>
                                <TypeOfPlaceRef ref="ZDL"/>
                            </placeTypes>
                            <quays>
                                <QuayRef ref="FR:77035:ZDE:50021889:STIF" />
                            </quays>
                        </StopPlace>
                        <Quay id="FR:94017:ZDE:50125579:STIF">
                            <Name>RABELAIS</Name>
                            <Centroid>
                                <Location>
                                    <gml:pos srsName="EPSG:2154">666944.0 6856019.0</gml:pos>
                                </Location>
                            </Centroid>
                        </Quay>
                        <Quay id="FR:92050:ZDE:50028126:STIF">
                            <Name>FERNAND LEGER</Name>
                            <Centroid>
                                <Location>
                                    <gml:pos srsName="EPSG:2154">666944.0 6856019.0</gml:pos>
                                </Location>
                            </Centroid>
                        </Quay>
                        <Quay id="FR:77035:ZDE:50021889:STIF">
                            <Name>Launoy</Name>
                            <Centroid>
                                <Location>
                                    <gml:pos srsName="EPSG:2154">700290.0 6796273.0</gml:pos>
                                </Location>
                            </Centroid>
                        </Quay>
                        <Quay id="FR:78034:ZDE:50067439:STIF">
                            <Name>Centre Village</Name>
                            <Centroid>
                                <Location>
                                    <gml:pos srsName="EPSG:2154">700290.0 6796273.0</gml:pos>
                                </Location>
                            </Centroid>
                        </Quay>
					</members>
				</GeneralFrame>
            </frames>
		</CompositeFrame>
	</dataObjects>
</root>"#;
        let root: Element = xml.parse().unwrap();
        let (stop_areas, stop_points) = load_stops(&root).unwrap();
        assert_eq!(4, stop_areas.len());

        let names: Vec<_> = stop_areas.values().map(|sa| &sa.name).collect();
        assert_eq!(
            vec![
                "Viroflay Gare Rive Droite",
                "Prairie",
                "CONVENTION",
                "Centre Village"
            ],
            names
        );

        let object_properties: Vec<_> = stop_areas
            .values()
            .flat_map(|sa| &sa.object_properties)
            .map(|op| (op.0.as_ref(), op.1.as_ref()))
            .collect();

        assert_eq!(
            vec![
                ("Netex_StopType", "LDA"),
                ("Netex_StopType", "LDA"),
                ("Netex_StopType", "ZDL"),
            ],
            object_properties
        );

        let object_codes: Vec<(_, Vec<_>)> = stop_areas
            .values()
            .map(|sa| (sa.id.as_ref(), sa.codes.iter().cloned().collect()))
            .collect();

        assert_eq!(
            vec![
                (
                    "FR:78686:LDA:422420:STIF",
                    vec![(
                        "Netex_ZDL".to_string(),
                        "FR:78423:ZDL:57857:STIF".to_string()
                    )]
                ),
                ("FR:28140:LDA:74325:STIF", vec![]),
                ("FR:0:ZDL:50057134:STIF", vec![]),
                ("Navitia:FR:78034:ZDE:50067439:STIF", vec![])
            ],
            object_codes
        );

        let stop_points: Vec<_> = stop_points
            .values()
            .cloned()
            .map(|sp| (sp.name, sp.stop_area_id))
            .collect();
        assert_eq!(
            vec![
                (
                    "RABELAIS".to_string(),
                    "FR:78686:LDA:422420:STIF".to_string()
                ),
                (
                    "FERNAND LEGER".to_string(),
                    "FR:78686:LDA:422420:STIF".to_string()
                ),
                ("Launoy".to_string(), "FR:0:ZDL:50057134:STIF".to_string()),
                (
                    "Centre Village".to_string(),
                    "Navitia:FR:78034:ZDE:50067439:STIF".to_string()
                ),
            ],
            stop_points
        );
    }
}
