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
    objects::{CommercialMode, Company, Line, Network},
    Result,
};
use failure::{bail, ResultExt};
use log::{info, warn};
use minidom::Element;
use std::{collections::HashMap, fs::File, io::Read};
use transit_model_collection::{CollectionWithId, Id};

#[derive(Debug, Default)]
struct LineNetexIDF {
    id: String,
    name: String,
    code: Option<String>,
    private_code: Option<String>,
    network_id: String,
    company_id: String,
    commercial_mode_id: String,
    physical_mode_id: String,
    wheelchair_accessible: bool,
}
impl_id!(LineNetexIDF);

type MapLineNetwork = HashMap<String, String>;

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
                lines_netex_idf.push(LineNetexIDF {
                    id,
                    name,
                    code,
                    private_code,
                    network_id,
                    company_id,
                    commercial_mode_id: String::from("Bus"), // TODO
                    physical_mode_id: String::from("Bus"),   // TODO
                    wheelchair_accessible: false,            // TODO
                })?;
            }
        }
    }
    Ok(lines_netex_idf)
}

fn make_lines(lines_netex_idf: &CollectionWithId<LineNetexIDF>) -> Result<CollectionWithId<Line>> {
    let mut lines = CollectionWithId::default();
    for ln in lines_netex_idf.values() {
        lines.push(Line {
            id: ln.id.clone(),
            name: ln.name.clone(),
            code: ln.code.clone(),
            network_id: ln.network_id.clone(),
            commercial_mode_id: ln.commercial_mode_id.clone(),
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

pub fn from_path(path: &std::path::Path, collections: &mut Collections) -> Result<()> {
    info!("Reading {:?}", path);

    let mut file = File::open(&path).with_context(ctx_from_path!(path))?;
    let mut file_content = String::new();
    file.read_to_string(&mut file_content)?;

    if let Ok(elem) = file_content.parse::<Element>() {
        let (networks, companies, map_line_network) = make_networks_companies(&elem)?;
        let lines_netex_idf = load_netex_lines(&elem, &map_line_network, &companies)?;
        let lines = make_lines(&lines_netex_idf)?;
        collections.networks.try_merge(networks)?;
        collections.companies.try_merge(companies)?;
        // TODO - to remove
        collections.commercial_modes.push(CommercialMode {
            id: String::from("Bus"),
            name: String::from("Bus"),
        })?;
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
            vec![
                "VEOLIA RAMBOUILLET",
                "VEOLIA RAMBOUILLET 2",
                "VEOLIA RAMBOUILLET 3"
            ],
            networks_names
        );
        let companies_names: Vec<_> = companies.values().map(|c| &c.name).collect();
        assert_eq!(
            vec![
                "TRANSDEV IDF RAMBOUILLET",
                "TRANSDEV IDF RAMBOUILLET 2",
                "TRANSDEV IDF RAMBOUILLET 3"
            ],
            companies_names
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
        assert_eq!(vec!["Line 01", "Line 04",], lines_names);
    }
}
