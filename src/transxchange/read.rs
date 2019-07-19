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
    minidom_utils::TryOnlyChild,
    model::{Collections, Model},
    objects::*,
    transxchange::naptan,
    Result,
};
use log::info;
use minidom::Element;
use std::{fs::File, io::Read, path::Path};
use zip::ZipArchive;

const EUROPE_LONDON_TIMEZONE: &str = "Europe/London";

fn get_operator(root: &Element) -> Result<&Element> {
    root.try_only_child("Operators")?.try_only_child("Operator")
}

fn load_network(operator: &Element) -> Result<Network> {
    let id = operator.try_only_child("OperatorCode")?.text();
    let name = operator
        .try_only_child("TradingName")
        .or_else(|_| operator.try_only_child("OperatorShortName"))?
        .text();
    let network = Network {
        id,
        name,
        timezone: Some(String::from(EUROPE_LONDON_TIMEZONE)),
        ..Default::default()
    };
    Ok(network)
}

fn load_company(operator: &Element) -> Result<Company> {
    let id = operator.try_only_child("OperatorCode")?.text();
    let name = operator.try_only_child("OperatorShortName")?.text();
    let company = Company {
        id,
        name,
        ..Default::default()
    };
    Ok(company)
}

fn read_transxchange(transxchange: &Element, collections: &mut Collections) -> Result<()> {
    let operator = get_operator(transxchange)?;
    let network = load_network(operator)?;
    collections.networks.push(network)?;
    let company = load_company(operator)?;
    collections.companies.push(company)?;
    unimplemented!()
}

fn read_transxchange_archive<P>(transxchange_path: P, collections: &mut Collections) -> Result<()>
where
    P: AsRef<Path>,
{
    let zip_file = File::open(transxchange_path)?;
    let mut zip_archive = ZipArchive::new(zip_file)?;
    for index in 0..zip_archive.len() {
        let mut zip_file = zip_archive.by_index(index)?;
        match zip_file.sanitized_name().extension() {
            Some(ext) if ext == "xml" => {
                info!("reading TransXChange file {:?}", zip_file.sanitized_name());
                let mut file_content = String::new();
                zip_file.read_to_string(&mut file_content)?;
                let root: Element = file_content.parse()?;
                read_transxchange(&root, collections)?;
            }
            _ => {
                info!("skipping file in zip: {:?}", zip_file.sanitized_name());
            }
        }
    }
    Ok(())
}

/// Read TransXChange format into a Navitia Transit Model
pub fn read<P>(transxchange_path: P, naptan_path: P) -> Result<Model>
where
    P: AsRef<Path>,
{
    let mut collections = Collections::default();
    naptan::read_naptan(naptan_path, &mut collections)?;
    read_transxchange_archive(transxchange_path, &mut collections)?;
    Model::new(collections)
}

#[cfg(test)]
mod tests {
    use super::*;

    mod get_operator {
        use super::*;

        #[test]
        fn has_operator() {
            let xml = r#"<root>
                <Operators>
                    <Operator />
                </Operators>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let operator = get_operator(&root).unwrap();
            assert_eq!(operator.name(), "Operator");
        }

        #[test]
        #[should_panic(expected = "Failed to find a child \\'Operators\\' in element \\'root\\'")]
        fn no_operators() {
            let xml = r#"<root />"#;
            let root: Element = xml.parse().unwrap();
            get_operator(&root).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'Operator\\' in element \\'Operators\\'"
        )]
        fn no_operator() {
            let xml = r#"<root>
                <Operators />
            </root>"#;
            let root: Element = xml.parse().unwrap();
            get_operator(&root).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a unique child \\'Operator\\' in element \\'Operators\\'"
        )]
        fn multiple_operator() {
            let xml = r#"<root>
                <Operators>
                    <Operator />
                    <Operator />
                </Operators>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            get_operator(&root).unwrap();
        }
    }

    mod load_network {
        use super::*;

        #[test]
        fn has_network() {
            let xml = r#"<root>
                <OperatorCode>SOME_CODE</OperatorCode>
                <TradingName>Some name</TradingName>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let network = load_network(&root).unwrap();
            assert_eq!(network.id, String::from("SOME_CODE"));
            assert_eq!(network.name, String::from("Some name"));
        }

        #[test]
        fn no_trading_name() {
            let xml = r#"<root>
                <OperatorCode>SOME_CODE</OperatorCode>
                <OperatorShortName>Some name</OperatorShortName>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let network = load_network(&root).unwrap();
            assert_eq!(network.id, String::from("SOME_CODE"));
            assert_eq!(network.name, String::from("Some name"));
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'OperatorCode\\' in element \\'root\\'"
        )]
        fn no_id() {
            let xml = r#"<root>
                <TradingName>Some name</TradingName>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            load_network(&root).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'OperatorShortName\\' in element \\'root\\'"
        )]
        fn no_name() {
            let xml = r#"<root>
                <OperatorCode>SOME_CODE</OperatorCode>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            load_network(&root).unwrap();
        }
    }

    mod load_company {
        use super::*;

        #[test]
        fn has_company() {
            let xml = r#"<root>
                <OperatorCode>SOME_CODE</OperatorCode>
                <OperatorShortName>Some name</OperatorShortName>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let company = load_company(&root).unwrap();
            assert_eq!(company.id, String::from("SOME_CODE"));
            assert_eq!(company.name, String::from("Some name"));
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'OperatorCode\\' in element \\'root\\'"
        )]
        fn no_id() {
            let xml = r#"<root>
                <OperatorShortName>Some name</OperatorShortName>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            load_company(&root).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'OperatorShortName\\' in element \\'root\\'"
        )]
        fn no_name() {
            let xml = r#"<root>
                <OperatorCode>SOME_CODE</OperatorCode>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            load_company(&root).unwrap();
        }
    }
}
