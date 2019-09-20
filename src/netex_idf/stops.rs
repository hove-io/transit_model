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
    objects::{Coord, Properties, StopArea},
    Result,
};
use failure::{format_err, ResultExt};
use log::info;
use minidom::Element;
use std::{fs::File, io::Read};
use transit_model_collection::CollectionWithId;

fn load_stop_areas(elem: &Element) -> Result<CollectionWithId<StopArea>> {
    let mut stop_areas = CollectionWithId::default();
    let members = elem
        .try_only_child("dataObjects")?
        .try_only_child("CompositeFrame")?
        .try_only_child("frames")?
        .children()
        .flat_map(|e| e.children())
        .filter(|e| e.name() == "members");
    for member in members {
        let stop_places = member.children().filter(|e| e.name() == "StopPlace");

        for stop_place in stop_places {
            if let Ok(_parent_site_ref) = stop_place.try_only_child("ParentSiteRef") {
                // mapping quays/QuayRef/@ref <-> ParentSiteRef/Name
                continue;
            }

            // add stop area
            let mut stop_area = StopArea {
                id: stop_place.try_attribute("id")?,
                name: stop_place.try_only_child("Name")?.text().trim().to_string(),
                visible: true,
                coord: Coord { lon: 0., lat: 0. },
                ..Default::default()
            };

            // add object properties
            let type_of_place_ref: String = stop_place
                .try_only_child("placeTypes")?
                .try_only_child("TypeOfPlaceRef")?
                .try_attribute("ref")?;
            stop_area
                .properties_mut()
                .insert(("Netex_StopType".to_string(), type_of_place_ref));

            stop_areas.push(stop_area)?;
        }
    }

    Ok(stop_areas)
}

pub fn from_path(path: &std::path::Path, collections: &mut Collections) -> Result<()> {
    info!("reading {:?}", path);

    let mut file = File::open(&path).with_context(ctx_from_path!(path))?;
    let mut file_content = String::new();
    file.read_to_string(&mut file_content)?;
    let elem = file_content.parse::<Element>();

    let stop_areas = elem
        .map_err(|e| format_err!("Failed to parse file '{:?}': {}", path, e))
        .and_then(|ref e| load_stop_areas(e))?;

    collections.stop_areas.try_merge(stop_areas)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
							<AccessibilityAssessment>
								<MobilityImpairedAccess>partial</MobilityImpairedAccess>
							</AccessibilityAssessment>
							<StopPlaceType>railStation</StopPlaceType>
						</StopPlace>
						<StopPlace  id="FR:28140:LDA:74325:STIF">
							<Name>Prairie</Name>
							<placeTypes>
								<TypeOfPlaceRef ref="LDA"/>
							</placeTypes>
							<AccessibilityAssessment>
								<MobilityImpairedAccess>unknown</MobilityImpairedAccess>
							</AccessibilityAssessment>
							<StopPlaceType>onstreetBus</StopPlaceType>
						</StopPlace>
                        <StopPlace id="FR:78423:ZDL:57857:STIF">>
                            <Name>Fort de Saint-Cyr</Name>
                            <placeTypes>
                                <TypeOfPlaceRef ref="ZDL"/>
                            </placeTypes>
                            <AccessibilityAssessment>
                                <MobilityImpairedAccess>partial</MobilityImpairedAccess>
                            </AccessibilityAssessment>
                            <ParentSiteRef ref="FR:78686:LDA:422420:STIF" />
                            <quays>
                                <QuayRef ref="FR:78423:ZDE:20880:STIF" />
                                <QuayRef ref="FR:78423:ZDE:20894:STIF" />
                            </quays>
                        </StopPlace>
                        <StopPlace id="FR:0:ZDL:50057134:STIF">
                            <Name>CONVENTION</Name>
                            <placeTypes>
                                <TypeOfPlaceRef ref="ZDL"/>
                            </placeTypes>
                            <AccessibilityAssessment>
                                <MobilityImpairedAccess>unknown</MobilityImpairedAccess>
                            </AccessibilityAssessment>
                            <quays>
                                <QuayRef ref="FR:93061:ZDE:50105305:STIF" />
                                <QuayRef ref="FR:93061:ZDE:50105266:STIF" />
                            </quays>
                        </StopPlace>
					</members>
				</GeneralFrame>
            </frames>
		</CompositeFrame>
	</dataObjects>
</root>"#;
        let root: Element = xml.parse().unwrap();
        let stop_areas = load_stop_areas(&root).unwrap();
        assert_eq!(3, stop_areas.len());

        let names: Vec<_> = stop_areas.values().map(|sa| &sa.name).collect();
        assert_eq!(
            vec!["Viroflay Gare Rive Droite", "Prairie", "CONVENTION"],
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
    }
}
