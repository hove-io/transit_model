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
    objects::{CommercialMode, Company, Line, Network, PhysicalMode},
    Result,
};
use failure::{bail, format_err, ResultExt};
use lazy_static::lazy_static;
use log::{info, warn};
use minidom::Element;
use std::{collections::BTreeSet, collections::HashMap, fs::File, io::Read};
use transit_model_collection::{CollectionWithId, Id};

#[derive(Debug, Default)]
struct LineNetexIDF {
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

type MapLineNetwork = HashMap<String, String>;

#[derive(Debug)]
struct ModeNetexIDF {
    // Tuple (mode_id, mode_name)
    physical_mode: (&'static str, &'static str),
    commercial_mode: (&'static str, &'static str),
}

lazy_static! {
    static ref MODES: HashMap<&'static str, ModeNetexIDF> = {
        let mut m = HashMap::new();
        m.insert(
            "air",
            ModeNetexIDF {
                physical_mode: ("Air", "Avion"),
                commercial_mode: ("Air", "Avion"),
            },
        );
        m.insert(
            "bus",
            ModeNetexIDF {
                physical_mode: ("Bus", "Bus"),
                commercial_mode: ("Bus", "Bus"),
            },
        );
        m.insert(
            "coach",
            ModeNetexIDF {
                physical_mode: ("Coach", "Autocar"),
                commercial_mode: ("Coach", "Autocar"),
            },
        );
        m.insert(
            "ferry",
            ModeNetexIDF {
                physical_mode: ("Ferry", "Ferry"),
                commercial_mode: ("Ferry", "Ferry"),
            },
        );
        m.insert(
            "metro",
            ModeNetexIDF {
                physical_mode: ("Metro", "Métro"),
                commercial_mode: ("Metro", "Métro"),
            },
        );
        m.insert(
            "rail",
            ModeNetexIDF {
                physical_mode: ("LocalTrain", "Train régional / TER"),
                commercial_mode: ("LocalTrain", "Train régional / TER"),
            },
        );
        m.insert(
            "trolleyBus",
            ModeNetexIDF {
                physical_mode: ("Tramway", "Tramway"),
                commercial_mode: ("TrolleyBus", "TrolleyBus"),
            },
        );
        m.insert(
            "tram",
            ModeNetexIDF {
                physical_mode: ("Tramway", "Tramway"),
                commercial_mode: ("Tramway", "Tramway"),
            },
        );
        m.insert(
            "water",
            ModeNetexIDF {
                physical_mode: ("Boat", "Navette maritime / fluviale"),
                commercial_mode: ("Boat", "Navette maritime / fluviale"),
            },
        );
        m.insert(
            "cableway",
            ModeNetexIDF {
                physical_mode: ("Tramway", "Tramway"),
                commercial_mode: ("CableWay", "CableWay"),
            },
        );
        m.insert(
            "funicular",
            ModeNetexIDF {
                physical_mode: ("Funicular", "Funiculaire"),
                commercial_mode: ("Funicular", "Funiculaire"),
            },
        );
        m.insert(
            "lift",
            ModeNetexIDF {
                physical_mode: ("Bus", "Bus"),
                commercial_mode: ("Bus", "Bus"),
            },
        );
        m.insert(
            "other",
            ModeNetexIDF {
                physical_mode: ("Bus", "Bus"),
                commercial_mode: ("Bus", "Bus"),
            },
        );
        m
    };
}

fn load_netex_lines(
    elem: &Element,
    map_line_network: &MapLineNetwork,
    companies: &CollectionWithId<Company>,
) -> Result<CollectionWithId<LineNetexIDF>> {
    let mut lines_netex_idf = CollectionWithId::default();
    for frame in elem
        .try_only_child("dataObjects")?
        .try_only_child("CompositeFrame")?
        .try_only_child("frames")?
        .children()
    {
        if let Ok(lines_node) = frame.try_only_child("lines") {
            for line in lines_node.children().filter(|e| e.name() == "Line") {
                let id = line.try_attribute("id")?;
                let name = line.try_only_child("Name")?.text().parse()?;
                let code = line.try_only_child("ShortName").map(Element::text).ok();
                let private_code = line.try_only_child("PrivateCode").map(Element::text).ok();
                let network_id = if let Some(network_id) = map_line_network.get(&id) {
                    network_id.to_string()
                } else {
                    warn!("Failed to find network for line {}", id);
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
                    .get::<str>(&mode)
                    .ok_or_else(|| format_err!("Unknown mode '{}' found for line {}", mode, id))?;

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
        let commercial_mode_id = if let Some(commercial_mode_id) =
            MODES.get::<str>(&ln.mode).map(|m| {
                let (cid, _) = m.commercial_mode;
                cid.to_string()
            }) {
            commercial_mode_id
        } else {
            warn!("commercial_mode_id not found for {}", ln.mode);
            continue;
        };
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
    elem: &Element,
) -> Result<(
    CollectionWithId<Network>,
    CollectionWithId<Company>,
    MapLineNetwork,
)> {
    let mut networks = CollectionWithId::default();
    let mut companies = CollectionWithId::default();
    let mut map_line_network: MapLineNetwork = HashMap::new();
    for frame in elem
        .try_only_child("dataObjects")?
        .try_only_child("CompositeFrame")?
        .try_only_child("frames")?
        .children()
    {
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

            let lines_ref = network
                .try_only_child("members")?
                .children()
                .filter(|e| e.name() == "LineRef");
            for line_ref in lines_ref {
                map_line_network
                    .insert(line_ref.try_attribute("ref")?, network.try_attribute("id")?);
            }
        }
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
    Ok((networks, companies, map_line_network))
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
            if let Some((pi, pn, ci, cn)) = MODES.get::<str>(&m).map(|m| {
                let (pi, pn) = m.physical_mode;
                let (ci, cn) = m.commercial_mode;
                (pi, pn, ci, cn)
            }) {
                (pi, pn, ci, cn)
            } else {
                warn!("{} not found", m);
                continue;
            };
        physical_modes.push(PhysicalMode {
            id: physical_mode_id.to_string(),
            name: physical_mode_name.to_string(),
            ..Default::default()
        })?;
        commercial_modes.push(CommercialMode {
            id: commercial_mode_id.to_string(),
            name: commercial_mode_name.to_string(),
            ..Default::default()
        })?;
    }
    Ok((physical_modes, commercial_modes))
}

pub fn from_path(path: &std::path::Path, collections: &mut Collections) -> Result<()> {
    info!("Reading {:?}", path);

    let mut file = File::open(&path).with_context(ctx_from_path!(path))?;
    let mut file_content = String::new();
    file.read_to_string(&mut file_content)?;

    if let Ok(elem) = file_content.parse::<Element>() {
        let (networks, companies, map_line_network) = make_networks_companies(&elem)?;
        let lines_netex_idf = load_netex_lines(&elem, &map_line_network, &companies)?;
        let lines = make_lines(&lines_netex_idf)?;
        let (physical_modes, commercial_modes) =
            make_physical_and_commercial_modes(&lines_netex_idf)?;
        collections.networks.try_merge(networks)?;
        collections.companies.try_merge(companies)?;
        collections.physical_modes.try_merge(physical_modes)?;
        collections.commercial_modes.try_merge(commercial_modes)?;
        collections.lines.try_merge(lines)?;
    } else {
        bail!("Failed to parse file {:?}", path);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_networks_companies() {
        // Test several networks in the same frame, or in several frames
        // Same thing for operators (= companies)
        let xml = r#"
<root>
   <dataObjects>
      <CompositeFrame id="FR100:CompositeFrame:NETEX_IDF-20181108T153214Z:LOC" version="1.8" dataSourceRef="FR100-OFFRE_AUTO">
         <frames>
            <ServiceFrame version="any" id="STIF:CODIFLIGNE:ServiceFrame:119">
               <Network version="any" changed="2009-12-02T00:00:00Z" id="STIF:CODIFLIGNE:PTNetwork:119">
                  <Name>VEOLIA RAMBOUILLET</Name>
                  <members>
                     <LineRef ref="STIF:CODIFLIGNE:Line:C00163"/>
                     <LineRef ref="STIF:CODIFLIGNE:Line:C00164"/>
                  </members>
               </Network>
               <Network version="any" changed="2009-12-02T00:00:00Z" id="STIF:CODIFLIGNE:PTNetwork:120">
                  <Name>VEOLIA RAMBOUILLET 2</Name>
                  <members>
                     <LineRef ref="STIF:CODIFLIGNE:Line:C00165"/>
                  </members>
               </Network>
            </ServiceFrame>
            <ServiceFrame version="any" id="STIF:CODIFLIGNE:ServiceFrame:121">
               <Network version="any" changed="2009-12-02T00:00:00Z" id="STIF:CODIFLIGNE:PTNetwork:121">
                  <Name>VEOLIA RAMBOUILLET 3</Name>
                  <members>
                     <LineRef ref="STIF:CODIFLIGNE:Line:C00166"/>
                  </members>
               </Network>
            </ServiceFrame>
            <ResourceFrame version="any" id="STIF:CODIFLIGNE:ResourceFrame:1">
               <organisations>
                  <Operator version="any" id="STIF:CODIFLIGNE:Operator:013">
                     <Name>TRANSDEV IDF RAMBOUILLET</Name>
                  </Operator>
                  <Operator version="any" id="STIF:CODIFLIGNE:Operator:014">
                     <Name>TRANSDEV IDF RAMBOUILLET 2</Name>
                  </Operator>
               </organisations>
            </ResourceFrame>
            <ResourceFrame version="any" id="STIF:CODIFLIGNE:ResourceFrame:2">
               <organisations>
                  <Operator version="any" id="STIF:CODIFLIGNE:Operator:015">
                     <Name>TRANSDEV IDF RAMBOUILLET 3</Name>
                  </Operator>
               </organisations>
            </ResourceFrame>
         </frames>
      </CompositeFrame>
   </dataObjects>
</root>"#;
        let root: Element = xml.parse().unwrap();
        let (networks, companies, _) = make_networks_companies(&root).unwrap();
        let networks_names: Vec<_> = networks.values().map(|n| &n.name).collect();
        assert_eq!(
            networks_names,
            vec![
                "VEOLIA RAMBOUILLET",
                "VEOLIA RAMBOUILLET 2",
                "VEOLIA RAMBOUILLET 3"
            ]
        );
        let companies_names: Vec<_> = companies.values().map(|c| &c.name).collect();
        assert_eq!(
            companies_names,
            vec![
                "TRANSDEV IDF RAMBOUILLET",
                "TRANSDEV IDF RAMBOUILLET 2",
                "TRANSDEV IDF RAMBOUILLET 3"
            ]
        );
    }

    #[test]
    fn test_make_lines() {
        let xml = r#"
<root>
   <dataObjects>
      <CompositeFrame id="FR100:CompositeFrame:NETEX_IDF-20181108T153214Z:LOC" version="1.8" dataSourceRef="FR100-OFFRE_AUTO">
         <frames>
            <ServiceFrame version="any" id="STIF:CODIFLIGNE:ServiceFrame:119">
               <Network version="any" changed="2009-12-02T00:00:00Z" id="STIF:CODIFLIGNE:PTNetwork:119">
                  <Name>VEOLIA RAMBOUILLET</Name>
                  <members>
                     <LineRef ref="STIF:CODIFLIGNE:Line:C00163"/>
                  </members>
               </Network>
               <Network version="any" changed="2009-12-02T00:00:00Z" id="STIF:CODIFLIGNE:PTNetwork:120">
                  <Name>VEOLIA RAMBOUILLET 2</Name>
                  <members>
                     <LineRef ref="STIF:CODIFLIGNE:Line:C00165"/>
                  </members>
               </Network>
            </ServiceFrame>
            <ServiceFrame version="any" id="STIF:CODIFLIGNE:ServiceFrame:121">
               <Network version="any" changed="2009-12-02T00:00:00Z" id="STIF:CODIFLIGNE:PTNetwork:121">
                  <Name>VEOLIA RAMBOUILLET 3</Name>
                  <members>
                     <LineRef ref="STIF:CODIFLIGNE:Line:C00166"/>
                  </members>
               </Network>
            </ServiceFrame>
            <ServiceFrame version="any" id="STIF:CODIFLIGNE:ServiceFrame:lineid">
               <lines>
                  <Line version="any" created="2014-07-16T00:00:00+00:00" changed="2014-07-16T00:00:00+00:00" status="active" id="STIF:CODIFLIGNE:Line:C00164">
                     <keyList>
                        <KeyValue>
                           <Key>Accessibility</Key>
                           <Value>0</Value>
                        </KeyValue>
                     </keyList>
                     <Name>Line 03</Name>
                     <ShortName>03</ShortName>
                     <TransportMode>bus</TransportMode>
                     <PrivateCode>013013003</PrivateCode>
                     <OperatorRef version="any" ref="STIF:CODIFLIGNE:Operator:013"/>
                  </Line>
                  <Line version="any" created="2014-07-16T00:00:00+00:00" changed="2014-07-16T00:00:00+00:00" status="active" id="STIF:CODIFLIGNE:Line:C00163">
                     <keyList>
                        <KeyValue>
                           <Key>Accessibility</Key>
                           <Value>0</Value>
                        </KeyValue>
                     </keyList>
                     <Name>Line 01</Name>
                     <ShortName>01</ShortName>
                     <TransportMode>bus</TransportMode>
                     <PrivateCode>013013001</PrivateCode>
                     <OperatorRef version="any" ref="STIF:CODIFLIGNE:Operator:013"/>
                  </Line>
                  <Line version="any" created="2014-07-16T00:00:00+00:00" changed="2014-07-16T00:00:00+00:00" status="active" id="STIF:CODIFLIGNE:Line:C00165">
                     <keyList>
                        <KeyValue>
                           <Key>Accessibility</Key>
                           <Value>0</Value>
                        </KeyValue>
                     </keyList>
                     <Name>Line 04</Name>
                     <ShortName>04</ShortName>
                     <TransportMode>bus</TransportMode>
                     <PrivateCode>013013004</PrivateCode>
                     <OperatorRef version="any" ref="STIF:CODIFLIGNE:Operator:013"/>
                  </Line>
                  <Line version="any" created="2014-07-16T00:00:00+00:00" changed="2014-07-16T00:00:00+00:00" status="active" id="STIF:CODIFLIGNE:Line:C00166">
                     <keyList>
                        <KeyValue>
                           <Key>Accessibility</Key>
                           <Value>0</Value>
                        </KeyValue>
                     </keyList>
                     <Name>Line 05</Name>
                     <ShortName>05</ShortName>
                     <TransportMode>bus</TransportMode>
                     <PrivateCode>013013005</PrivateCode>
                     <OperatorRef version="any" ref="STIF:CODIFLIGNE:Operator:0133"/>
                  </Line>
               </lines>
            </ServiceFrame>
            <ResourceFrame version="any" id="STIF:CODIFLIGNE:ResourceFrame:1">
               <organisations>
                  <Operator version="any" id="STIF:CODIFLIGNE:Operator:013">
                     <Name>TRANSDEV IDF RAMBOUILLET</Name>
                  </Operator>
               </organisations>
            </ResourceFrame>
         </frames>
      </CompositeFrame>
   </dataObjects>
</root>"#;
        let root: Element = xml.parse().unwrap();
        let (_networks, companies, map_line_network) = make_networks_companies(&root).unwrap();
        let lines_netex_idf = load_netex_lines(&root, &map_line_network, &companies).unwrap();
        let lines = make_lines(&lines_netex_idf).unwrap();
        let lines_names: Vec<_> = lines.values().map(|l| &l.name).collect();
        // Test explanation
        // Line 03 - Orphan line; not referenced by any network -> line skipped
        // Line 05 - Unknown company -> line skipped
        assert_eq!(lines_names, vec!["Line 01", "Line 04"]);
    }
}
