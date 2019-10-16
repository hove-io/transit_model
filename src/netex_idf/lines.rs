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

use super::modes::MODES;
use super::EUROPE_PARIS_TIMEZONE;
use crate::{
    minidom_utils::{TryAttribute, TryOnlyChild},
    model::Collections,
    netex_utils,
    netex_utils::{FrameType, Frames},
    objects::{CommercialMode, Company, Line, Network, PhysicalMode},
    Result,
};
use failure::{bail, format_err, ResultExt};
use log::{info, warn};
use minidom::Element;
use std::{collections::BTreeSet, fs::File, io::Read};
use transit_model_collection::{CollectionWithId, Id};

#[derive(Debug, Default)]
pub struct LineNetexIDF {
    id: String,
    name: String,
    code: Option<String>,
    private_code: Option<String>,
    network_id: String,
    company_id: String,
    mode: String,
    wheelchair_accessible: bool,
}
impl_id!(LineNetexIDF);

fn load_netex_lines(
    frames: &Frames,
    networks: &CollectionWithId<Network>,
    companies: &CollectionWithId<Company>,
) -> Result<CollectionWithId<LineNetexIDF>> {
    let mut lines_netex_idf = CollectionWithId::default();
    for frame in frames.get(&FrameType::Service).unwrap_or(&vec![]) {
        if let Ok(lines_node) = frame.try_only_child("lines") {
            for line in lines_node.children().filter(|e| e.name() == "Line") {
                let id = line.try_attribute("id")?;
                let name = line.try_only_child("Name")?.text().parse()?;
                let code = line
                    .try_only_child_with_filter("PublicCode", |e| !e.text().is_empty())
                    .or_else(|_| line.try_only_child("ShortName"))
                    .map(Element::text)
                    .ok();
                let private_code = line.only_child("PrivateCode").map(Element::text);
                let network_id: String = line
                    .try_only_child("RepresentedByGroupRef")?
                    .try_attribute("ref")?;
                let network_id = if let Some(_network) = networks.get(&network_id) {
                    network_id
                } else {
                    warn!("Failed to find network {} for line {}", network_id, id);
                    continue;
                };
                let company_id: String =
                    line.try_only_child("OperatorRef")?.try_attribute("ref")?;
                let company_id = if let Some(_company) = companies.get(&company_id) {
                    company_id
                } else {
                    warn!("Failed to find company {} for line {}", company_id, id);
                    continue;
                };
                let mode: String = line.try_only_child("TransportMode")?.text().parse()?;
                MODES
                    .get(mode.as_str())
                    .ok_or_else(|| format_err!("Unknown mode {} found for line {}", mode, id))?;

                lines_netex_idf.push(LineNetexIDF {
                    id,
                    name,
                    code,
                    private_code,
                    network_id,
                    company_id,
                    mode,
                    wheelchair_accessible: false, // TODO
                })?;
            }
        }
    }
    Ok(lines_netex_idf)
}

fn make_lines(lines_netex_idf: &CollectionWithId<LineNetexIDF>) -> Result<CollectionWithId<Line>> {
    let mut lines = CollectionWithId::default();
    for ln in lines_netex_idf.values() {
        let commercial_mode_id = skip_fail!(MODES
            .get(ln.mode.as_str())
            .map(|m| { m.commercial_mode.0.to_string() })
            .ok_or_else(|| format_err!("{} not found", ln.mode)));
        lines.push(Line {
            id: ln.id.clone(),
            name: ln.name.clone(),
            code: ln.code.clone(),
            network_id: ln.network_id.clone(),
            commercial_mode_id,
            ..Default::default()
        })?;
    }
    Ok(lines)
}

fn make_networks_companies(
    frames: &Frames,
) -> Result<(CollectionWithId<Network>, CollectionWithId<Company>)> {
    let mut networks = CollectionWithId::default();
    let mut companies = CollectionWithId::default();
    for frame in frames.get(&FrameType::Service).unwrap_or(&vec![]) {
        for network in frame.children().filter(|e| e.name() == "Network") {
            let id = network.try_attribute("id")?;
            let name = network.try_only_child("Name")?.text().parse()?;
            let timezone = Some(String::from(EUROPE_PARIS_TIMEZONE));
            networks.push(Network {
                id,
                name,
                timezone,
                ..Default::default()
            })?;
        }
    }
    for frame in frames.get(&FrameType::Resource).unwrap_or(&vec![]) {
        if let Ok(organisations) = frame.try_only_child("organisations") {
            for operator in organisations.children().filter(|e| e.name() == "Operator") {
                let id = operator.try_attribute("id")?;
                let name = operator.try_only_child("Name")?.text().parse()?;
                companies.push(Company {
                    id,
                    name,
                    ..Default::default()
                })?;
            }
        }
    }
    Ok((networks, companies))
}

fn make_physical_and_commercial_modes(
    lines_netex_idf: &CollectionWithId<LineNetexIDF>,
) -> Result<(
    CollectionWithId<PhysicalMode>,
    CollectionWithId<CommercialMode>,
)> {
    let mut physical_modes = CollectionWithId::default();
    let mut commercial_modes = CollectionWithId::default();
    let modes: BTreeSet<_> = lines_netex_idf.values().map(|l| &l.mode).collect();
    for m in modes {
        let (physical_mode_id, physical_mode_name, commercial_mode_id, commercial_mode_name) =
            skip_fail!(MODES
                .get(m.as_str())
                .map(|m| {
                    (
                        m.physical_mode.0,
                        m.physical_mode.1,
                        m.commercial_mode.0,
                        m.commercial_mode.1,
                    )
                })
                .ok_or_else(|| format_err!("{} not found", m)));
        physical_modes.push(PhysicalMode {
            id: physical_mode_id.to_string(),
            name: physical_mode_name.to_string(),
            ..Default::default()
        })?;
        commercial_modes.push(CommercialMode {
            id: commercial_mode_id.to_string(),
            name: commercial_mode_name.to_string(),
        })?;
    }
    Ok((physical_modes, commercial_modes))
}

pub fn from_path(
    path: &std::path::Path,
    collections: &mut Collections,
) -> Result<CollectionWithId<LineNetexIDF>> {
    info!("Reading {:?}", path);
    let mut file = File::open(&path).with_context(ctx_from_path!(path))?;
    let mut file_content = String::new();
    file.read_to_string(&mut file_content)?;

    let lines_netex_idf = if let Ok(root) = file_content.parse::<Element>() {
        let frames = netex_utils::parse_frames_by_type(
            root.try_only_child("dataObjects")?
                .try_only_child("CompositeFrame")?
                .try_only_child("frames")?,
        )?;
        let (networks, companies) = make_networks_companies(&frames)?;
        let lines_netex_idf = load_netex_lines(&frames, &networks, &companies)?;
        let lines = make_lines(&lines_netex_idf)?;
        let (physical_modes, commercial_modes) =
            make_physical_and_commercial_modes(&lines_netex_idf)?;
        collections.networks.try_merge(networks)?;
        collections.companies.try_merge(companies)?;
        collections.physical_modes.try_merge(physical_modes)?;
        collections.commercial_modes.try_merge(commercial_modes)?;
        collections.lines.try_merge(lines)?;
        lines_netex_idf
    } else {
        bail!("Failed to parse file {:?}", path);
    };
    Ok(lines_netex_idf)
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use pretty_assertions::assert_eq;

//     #[test]
//     #[should_panic(expected = "Unknown mode UNKNOWN found for line STIF:CODIFLIGNE:Line:C00163")]
//     fn test_load_netex_lines_unknown_mode() {
//         let xml = r#"
//             <ServiceFrame version="any" id="STIF:CODIFLIGNE:ServiceFrame:119">
//                <Network version="any" changed="2009-12-02T00:00:00Z" id="STIF:CODIFLIGNE:PTNetwork:119">
//                   <Name>VEOLIA RAMBOUILLET</Name>
//                   <members>
//                      <LineRef ref="STIF:CODIFLIGNE:Line:C00163"/>
//                   </members>
//                </Network>
//             </ServiceFrame>"#;
//         let service_frame_networks: Element = xml.parse().unwrap();
//         let xml = r#"
//             <ServiceFrame version="any" id="STIF:CODIFLIGNE:ServiceFrame:lineid">
//                <lines>
//                   <Line version="any" created="2014-07-16T00:00:00+00:00" changed="2014-07-16T00:00:00+00:00" status="active" id="STIF:CODIFLIGNE:Line:C00163">
//                      <Name>Line 01</Name>
//                      <ShortName>01</ShortName>
//                      <TransportMode>UNKNOWN</TransportMode>
//                      <PrivateCode>013013001</PrivateCode>
//                      <OperatorRef version="any" ref="STIF:CODIFLIGNE:Operator:013"/>
//                   </Line>
//                </lines>
//             </ServiceFrame>"#;
//         let service_frame_lines: Element = xml.parse().unwrap();
//         let xml = r#"
//             <ResourceFrame version="any" id="STIF:CODIFLIGNE:ResourceFrame:1">
//                <organisations>
//                   <Operator version="any" id="STIF:CODIFLIGNE:Operator:013">
//                      <Name>TRANSDEV IDF RAMBOUILLET</Name>
//                   </Operator>
//                </organisations>
//             </ResourceFrame>"#;
//         let resource_frame_organisations: Element = xml.parse().unwrap();
//         let mut frames = HashMap::new();
//         frames.insert(
//             FrameType::Service,
//             vec![&service_frame_networks, &service_frame_lines],
//         );
//         frames.insert(FrameType::Resource, vec![&resource_frame_organisations]);
//         let (_networks, companies, map_line_network) = make_networks_companies(&frames).unwrap();
//         load_netex_lines(&frames, &map_line_network, &companies).unwrap();
//     }
// }
