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
    objects::{Company, Network},
    Result,
};
use failure::{format_err, ResultExt};
use log::info;
use minidom::Element;
use std::{fs::File, io::Read};
use transit_model_collection::CollectionWithId;

fn load_networks_companies(
    elem: &Element,
) -> Result<(CollectionWithId<Network>, CollectionWithId<Company>)> {
    let mut networks = CollectionWithId::default();
    let mut companies = CollectionWithId::default();
    for frame in elem
        .try_only_child("dataObjects")?
        .try_only_child("CompositeFrame")?
        .try_only_child("frames")?
        .children()
    {
        if let Ok(network) = frame.try_only_child("Network") {
            let id = network.try_attribute("id")?;
            let name = network.try_only_child("Name")?.text().parse()?;
            let timezone = Some(String::from(EUROPE_PARIS_TIMEZONE));
            let network = Network {
                id,
                name,
                timezone,
                ..Default::default()
            };
            networks.push(network)?;
        }
        if let Ok(company) = frame
            .try_only_child("organisations")
            .and_then(|org| org.try_only_child("Operator"))
        {
            let id = company.try_attribute("id")?;
            let name = company.try_only_child("Name")?.text().parse()?;
            let company = Company {
                id,
                name,
                ..Default::default()
            };
            companies.push(company)?;
        }
    }
    Ok((networks, companies))
}

pub fn from_path(path: &std::path::Path, collections: &mut Collections) -> Result<()> {
    info!("reading {:?}", path);

    let mut file = File::open(&path).with_context(ctx_from_path!(path))?;
    let mut file_content = String::new();
    file.read_to_string(&mut file_content)?;
    let elem = file_content.parse::<Element>();

    let (networks, companies) = elem
        .map_err(|e| format_err!("Failed to parse file '{:?}': {}", path, e))
        .and_then(|ref e| load_networks_companies(e))?;

    collections.networks.try_merge(networks)?;
    collections.companies.try_merge(companies)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_networks_companies() {
        let xml = r#"
<root>
   <dataObjects>
      <CompositeFrame id="FR100:CompositeFrame:NETEX_IDF-20181108T153214Z:LOC" version="1.8" dataSourceRef="FR100-OFFRE_AUTO">
         <TypeOfFrameRef ref="FR100:TypeOfFrame:NETEX_IDF:"/>
         <frames>
            <ServiceFrame version="any" id="STIF:CODIFLIGNE:ServiceFrame:119">
               <Network version="any" changed="2009-12-02T00:00:00Z" id="STIF:CODIFLIGNE:PTNetwork:119">
                  <Name>VEOLIA RAMBOUILLET</Name>
                  <members>
                     <LineRef ref="STIF:CODIFLIGNE:Line:C00163"/>
                     <LineRef ref="STIF:CODIFLIGNE:Line:C00164"/>
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
                     <Name>03</Name>
                     <ShortName>03</ShortName>
                     <TransportMode>bus</TransportMode>
                     <PrivateCode>013013003</PrivateCode>
                     <OperatorRef version="any" ref="STIF:CODIFLIGNE:Operator:013"/>
                     <TypeOfLineRef version="any" ref="null"/>
                     <Presentation>
                        <infoLinks/>
                     </Presentation>
                  </Line>
                  <Line version="any" created="2014-07-16T00:00:00+00:00" changed="2014-07-16T00:00:00+00:00" status="active" id="STIF:CODIFLIGNE:Line:C00163">
                     <keyList>
                        <KeyValue>
                           <Key>Accessibility</Key>
                           <Value>0</Value>
                        </KeyValue>
                     </keyList>
                     <Name>01</Name>
                     <ShortName>01</ShortName>
                     <TransportMode>bus</TransportMode>
                     <PrivateCode>013013001</PrivateCode>
                     <OperatorRef version="any" ref="STIF:CODIFLIGNE:Operator:013"/>
                     <TypeOfLineRef version="any" ref="null"/>
                     <Presentation>
                        <infoLinks/>
                     </Presentation>
                  </Line>
               </lines>
            </ServiceFrame>
            <ResourceFrame version="any" id="STIF:CODIFLIGNE:ResourceFrame:1">
               <typesOfValue>
                  <TypeOfLine version="any" id="STIF:CODIFLIGNE:seasonal"/>
               </typesOfValue>
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
        let (networks, companies) = load_networks_companies(&root).unwrap();
        let networks_names: Vec<_> = networks.values().map(|n| &n.name).collect();
        assert_eq!(vec!["VEOLIA RAMBOUILLET"], networks_names);
        let companies_names: Vec<_> = companies.values().map(|c| &c.name).collect();
        assert_eq!(vec!["TRANSDEV IDF RAMBOUILLET"], companies_names);
    }
}
