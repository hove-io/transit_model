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
    pub id: String,
    pub name: String,
    pub code: Option<String>,
    pub private_code: Option<String>,
    pub network_id: String,
    pub company_id: String,
    pub mode: String,
    pub wheelchair_accessible: bool,
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
                if !networks.contains_id(&network_id) {
                    warn!("Failed to find network {} for line {}", network_id, id);
                    continue;
                }
                let company_id: String =
                    line.try_only_child("OperatorRef")?.try_attribute("ref")?;
                if !companies.contains_id(&company_id) {
                    warn!("Failed to find company {} for line {}", company_id, id);
                    continue;
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    #[should_panic(expected = "Unknown mode UNKNOWN found for line FR1:Line:C00001")]
    fn test_load_netex_lines_unknown_mode() {
        let xml = r#"
            <ServiceFrame>
               <lines>
                  <Line id="FR1:Line:C00001">
                     <Name>Line 01</Name>
                     <ShortName>01</ShortName>
                     <TransportMode>UNKNOWN</TransportMode>
                     <RepresentedByGroupRef ref="FR1:Network:1:LOC"/>
                     <OperatorRef ref="FR1:Operator:1:LOC"/>
                  </Line>
               </lines>
            </ServiceFrame>"#;
        let mut frames = HashMap::new();
        let service_frame_lines: Element = xml.parse().unwrap();
        frames.insert(FrameType::Service, vec![&service_frame_lines]);

        let networks = CollectionWithId::new(vec![Network {
            id: String::from("FR1:Network:1:LOC"),
            name: String::from("Network1"),
            ..Default::default()
        }])
        .unwrap();
        let companies = CollectionWithId::new(vec![Company {
            id: String::from("FR1:Operator:1:LOC"),
            name: String::from("Operator1"),
            ..Default::default()
        }])
        .unwrap();

        load_netex_lines(&frames, &networks, &companies).unwrap();
    }
}
