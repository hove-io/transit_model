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
    collection::CollectionWithId,
    minidom_utils::TryOnlyChild,
    model::{Collections, Model},
    objects::*,
    transxchange::naptan,
    AddPrefix, Result,
};
use chrono::{
    naive::{MAX_DATE, MIN_DATE},
    Duration,
};
use failure::format_err;
use log::info;
use minidom::Element;
use std::{fs::File, io::Read, path::Path};
use walkdir::WalkDir;
use zip::ZipArchive;

const EUROPE_LONDON_TIMEZONE: &str = "Europe/London";

fn get_service_validity_period(transxchange: &Element) -> Result<ValidityPeriod> {
    let operating_period = transxchange
        .try_only_child("Services")?
        .try_only_child("Service")?
        .try_only_child("OperatingPeriod")?;
    let start_date: Date = operating_period
        .try_only_child("StartDate")?
        .text()
        .parse()?;
    let end_date: Date = operating_period
        .try_only_child("EndDate")
        .map(Element::text)
        .map(|end_date_text| end_date_text.parse())
        .unwrap_or_else(|_| Ok(start_date + Duration::days(180)))?;
    Ok(ValidityPeriod {
        start_date,
        end_date,
    })
}

fn update_validity_period(dataset: &mut Dataset, service_validity_period: &ValidityPeriod) {
    dataset.start_date = if service_validity_period.start_date < dataset.start_date {
        service_validity_period.start_date
    } else {
        dataset.start_date
    };
    dataset.end_date = if service_validity_period.end_date > dataset.end_date {
        service_validity_period.end_date
    } else {
        dataset.end_date
    };
}

// The datasets already have some validity period. This function tries to
// extend them with a service validity period from the TransXChange file:
// - if service start date is before the dataset start date, then update the
//   dataset start date with service start date
// - if service end date is after the dataset end date, then update the
//   dataset end date with service end date
//
// Examples:
// Past                                                             Future
// |--------------------------------------------------------------------->
//
//             ^--------- dataset validity ---------^
//                 ^---- service validity ----^
//             ^------ final dataset validity ------^
//
//             ^--------- dataset validity ---------^
//      ^---- service validity ----^
//      ^--------- final dataset validity ----------^
//
//             ^--------- dataset validity ---------^
//          ^-------------- service validity --------------^
//          ^----------- final dataset validity -----------^
fn update_validity_period_from_transxchange(
    datasets: &mut CollectionWithId<Dataset>,
    transxchange: &Element,
) -> Result<CollectionWithId<Dataset>> {
    let service_validity_period = get_service_validity_period(transxchange)?;
    let mut datasets = datasets.take();
    for dataset in &mut datasets {
        update_validity_period(dataset, &service_validity_period);
    }
    CollectionWithId::new(datasets)
}

fn get_operator<'a>(transxchange: &'a Element, operator_ref: &str) -> Result<&'a Element> {
    let is_operator_ref = |operator: &&Element| {
        operator
            .attr("id")
            .filter(|id| *id == operator_ref)
            .is_some()
    };
    transxchange
        .try_only_child("Operators")?
        .children()
        .find(is_operator_ref)
        .ok_or_else(|| {
            format_err!(
                "Failed to find the operator for reference '{}'",
                operator_ref
            )
        })
}

fn load_network(transxchange: &Element) -> Result<Network> {
    let operator_ref = transxchange
        .try_only_child("Services")?
        .try_only_child("Service")?
        .try_only_child("RegisteredOperatorRef")?
        .text();
    let operator = get_operator(transxchange, &operator_ref)?;
    let id = operator.try_only_child("OperatorCode")?.text();
    let name = operator
        .try_only_child("TradingName")
        .or_else(|_| operator.try_only_child("OperatorShortName"))?
        .text()
        .trim()
        .to_string();
    let name = if name.is_empty() {
        String::from("Undefined")
    } else {
        name
    };
    let network = Network {
        id,
        name,
        timezone: Some(String::from(EUROPE_LONDON_TIMEZONE)),
        ..Default::default()
    };
    Ok(network)
}

fn load_companies(transxchange: &Element) -> Result<CollectionWithId<Company>> {
    let mut companies = CollectionWithId::default();
    for operator in transxchange.try_only_child("Operators")?.children() {
        let id = operator.try_only_child("OperatorCode")?.text();
        let name = operator
            .try_only_child("OperatorShortName")?
            .text()
            .trim()
            .to_string();
        let company = Company {
            id,
            name,
            ..Default::default()
        };
        companies.push(company)?;
    }
    Ok(companies)
}

fn read_xml(transxchange: &Element, collections: &mut Collections) -> Result<()> {
    collections.datasets =
        update_validity_period_from_transxchange(&mut collections.datasets, transxchange)?;
    let network = load_network(transxchange)?;
    if collections.networks.get(&network.id).is_none() {
        collections.networks.push(network)?;
    }
    let companies = load_companies(transxchange)?;
    collections.companies.merge(companies);
    Ok(())
}

fn read_file<F>(file_path: &Path, mut file: F, collections: &mut Collections) -> Result<()>
where
    F: Read,
{
    match file_path.extension() {
        Some(ext) if ext == "xml" => {
            info!("reading TransXChange file {:?}", file_path);
            let mut file_content = String::new();
            file.read_to_string(&mut file_content)?;
            let root: Element = file_content.parse()?;
            read_xml(&root, collections)?;
        }
        _ => info!("skipping file {:?}", file_path),
    };
    Ok(())
}

fn read_from_zip<P>(transxchange_path: P, collections: &mut Collections) -> Result<()>
where
    P: AsRef<Path>,
{
    let zip_file = File::open(transxchange_path)?;
    let mut zip_archive = ZipArchive::new(zip_file)?;
    for index in 0..zip_archive.len() {
        let file = zip_archive.by_index(index)?;
        read_file(file.sanitized_name().as_path(), file, collections)?;
    }
    Ok(())
}

fn read_from_path<P>(transxchange_path: P, collections: &mut Collections) -> Result<()>
where
    P: AsRef<Path>,
{
    for entry in WalkDir::new(transxchange_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let file = File::open(entry.path())?;
        read_file(entry.path(), file, collections)?;
    }
    Ok(())
}

/// Read TransXChange format into a Navitia Transit Model
pub fn read<P>(
    transxchange_path: P,
    naptan_path: P,
    config_path: Option<P>,
    prefix: Option<String>,
) -> Result<Model>
where
    P: AsRef<Path>,
{
    fn init_dataset_validity_periods(
        mut datasets: CollectionWithId<Dataset>,
    ) -> Result<CollectionWithId<Dataset>> {
        let mut datasets = datasets.take();
        for dataset in &mut datasets {
            dataset.start_date = MAX_DATE;
            dataset.end_date = MIN_DATE;
        }
        CollectionWithId::new(datasets)
    }

    let mut collections = Collections::default();
    let (contributors, datasets, feed_infos) = crate::read_utils::read_config(config_path)?;
    collections.contributors = contributors;
    collections.datasets = init_dataset_validity_periods(datasets)?;
    collections.feed_infos = feed_infos;
    if naptan_path.as_ref().is_file() {
        naptan::read_from_zip(naptan_path, &mut collections)?;
    } else {
        naptan::read_from_path(naptan_path, &mut collections)?;
    };
    if transxchange_path.as_ref().is_file() {
        read_from_zip(transxchange_path, &mut collections)?;
    } else {
        read_from_path(transxchange_path, &mut collections)?;
    };

    if let Some(prefix) = prefix {
        collections.add_prefix_with_sep(prefix.as_str(), ":");
    }
    Model::new(collections)
}

#[cfg(test)]
mod tests {
    use super::*;

    mod get_service_validity_period {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn has_start_and_end() {
            let xml = r#"<root>
                <Services>
                    <Service>
                        <OperatingPeriod>
                            <StartDate>2019-01-01</StartDate>
                            <EndDate>2019-03-31</EndDate>
                        </OperatingPeriod>
                    </Service>
                </Services>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let ValidityPeriod {
                start_date,
                end_date,
            } = get_service_validity_period(&root).unwrap();
            assert_eq!(start_date, Date::from_ymd(2019, 1, 1));
            assert_eq!(end_date, Date::from_ymd(2019, 3, 31));
        }

        #[test]
        fn has_only_start() {
            let xml = r#"<root>
                <Services>
                    <Service>
                        <OperatingPeriod>
                            <StartDate>2019-01-01</StartDate>
                        </OperatingPeriod>
                    </Service>
                </Services>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let ValidityPeriod {
                start_date,
                end_date,
            } = get_service_validity_period(&root).unwrap();
            assert_eq!(start_date, Date::from_ymd(2019, 1, 1));
            assert_eq!(end_date, Date::from_ymd(2019, 6, 30));
        }

        #[test]
        #[should_panic]
        fn no_date() {
            let xml = r#"<root>
                <Services>
                    <Service>
                        <OperatingPeriod />
                    </Service>
                </Services>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            get_service_validity_period(&root).unwrap();
        }

        #[test]
        #[should_panic]
        fn invalid_start_date() {
            let xml = r#"<root>
                <Services>
                    <Service>
                        <OperatingPeriod>
                            <StartDate>2019-42-01</StartDate>
                        </OperatingPeriod>
                    </Service>
                </Services>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            get_service_validity_period(&root).unwrap();
        }

        #[test]
        #[should_panic]
        fn invalid_end_date() {
            let xml = r#"<root>
                <Services>
                    <Service>
                        <OperatingPeriod>
                            <StartDate>2019-01-01</StartDate>
                            <EndDate>NotADate</EndDate>
                        </OperatingPeriod>
                    </Service>
                </Services>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            get_service_validity_period(&root).unwrap();
        }
    }

    mod update_validity_period {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn no_existing_validity_period() {
            let start_date = Date::from_ymd(2019, 1, 1);
            let end_date = Date::from_ymd(2019, 6, 30);
            let mut dataset = Dataset {
                id: String::from("dataset_id"),
                contributor_id: String::from("contributor_id"),
                start_date: MAX_DATE,
                end_date: MIN_DATE,
                ..Default::default()
            };
            let service_validity_period = ValidityPeriod {
                start_date,
                end_date,
            };
            update_validity_period(&mut dataset, &service_validity_period);
            assert_eq!(dataset.start_date, start_date);
            assert_eq!(dataset.end_date, end_date);
        }

        #[test]
        fn with_extended_validity_period() {
            let start_date = Date::from_ymd(2019, 1, 1);
            let end_date = Date::from_ymd(2019, 6, 30);
            let mut dataset = Dataset {
                id: String::from("dataset_id"),
                contributor_id: String::from("contributor_id"),
                start_date: Date::from_ymd(2019, 3, 1),
                end_date: Date::from_ymd(2019, 4, 30),
                ..Default::default()
            };
            let service_validity_period = ValidityPeriod {
                start_date,
                end_date,
            };
            update_validity_period(&mut dataset, &service_validity_period);
            assert_eq!(dataset.start_date, start_date);
            assert_eq!(dataset.end_date, end_date);
        }

        #[test]
        fn with_included_validity_period() {
            let start_date = Date::from_ymd(2019, 1, 1);
            let end_date = Date::from_ymd(2019, 6, 30);
            let mut dataset = Dataset {
                id: String::from("dataset_id"),
                contributor_id: String::from("contributor_id"),
                start_date,
                end_date,
                ..Default::default()
            };
            let service_validity_period = ValidityPeriod {
                start_date: Date::from_ymd(2019, 3, 1),
                end_date: Date::from_ymd(2019, 4, 30),
            };
            update_validity_period(&mut dataset, &service_validity_period);
            assert_eq!(dataset.start_date, start_date);
            assert_eq!(dataset.end_date, end_date);
        }
    }

    mod update_validity_period_from_transxchange {
        use super::*;

        #[test]
        fn has_start_and_end() {
            let xml = r#"<root>
                <Services>
                    <Service>
                        <OperatingPeriod>
                            <StartDate>2019-03-01</StartDate>
                            <EndDate>2019-04-30</EndDate>
                        </OperatingPeriod>
                    </Service>
                </Services>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let ds1 = Dataset {
                id: String::from("dataset_1"),
                contributor_id: String::from("contributor_id"),
                start_date: Date::from_ymd(2019, 1, 1),
                end_date: Date::from_ymd(2019, 6, 30),
                ..Default::default()
            };
            let ds2 = Dataset {
                id: String::from("dataset_2"),
                contributor_id: String::from("contributor_id"),
                start_date: Date::from_ymd(2019, 3, 31),
                end_date: Date::from_ymd(2019, 4, 1),
                ..Default::default()
            };
            let mut datasets = CollectionWithId::new(vec![ds1, ds2]).unwrap();
            let datasets = update_validity_period_from_transxchange(&mut datasets, &root).unwrap();
            let mut datasets_iter = datasets.values();
            let dataset = datasets_iter.next().unwrap();
            assert_eq!(dataset.start_date, Date::from_ymd(2019, 1, 1));
            assert_eq!(dataset.end_date, Date::from_ymd(2019, 6, 30));
            let dataset = datasets_iter.next().unwrap();
            assert_eq!(dataset.start_date, Date::from_ymd(2019, 3, 1));
            assert_eq!(dataset.end_date, Date::from_ymd(2019, 4, 30));
        }
    }

    mod get_operator {
        use super::*;

        #[test]
        fn has_operator() {
            let xml = r#"<root>
                <Operators>
                    <Operator id="op1">
                        <OperatorCode>SOME_CODE</OperatorCode>
                        <TradingName>Some name</TradingName>
                    </Operator>
                    <Operator id="op2">
                        <OperatorCode>OTHER_CODE</OperatorCode>
                        <TradingName>Other name</TradingName>
                    </Operator>
                </Operators>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let operator = get_operator(&root, &String::from("op1")).unwrap();
            let id = operator.try_only_child("OperatorCode").unwrap().text();
            assert_eq!(id, "SOME_CODE");
            let name = operator.try_only_child("TradingName").unwrap().text();
            assert_eq!(name, "Some name");
        }

        #[test]
        #[should_panic(expected = "Failed to find the operator for reference \\'op3\\'")]
        fn no_operator() {
            let xml = r#"<root>
                <Operators>
                    <Operator id="op1">
                        <OperatorCode>SOME_CODE</OperatorCode>
                        <TradingName>Some name</TradingName>
                    </Operator>
                    <Operator id="op2">
                        <OperatorCode>OTHER_CODE</OperatorCode>
                        <TradingName>Other name</TradingName>
                    </Operator>
                </Operators>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            get_operator(&root, &String::from("op3")).unwrap();
        }
    }

    mod load_network {
        use super::*;

        #[test]
        fn has_network() {
            let xml = r#"<root>
                <Services>
                    <Service>
                        <RegisteredOperatorRef>op1</RegisteredOperatorRef>
                    </Service>
                </Services>
                <Operators>
                    <Operator id="op1">
                        <OperatorCode>SOME_CODE</OperatorCode>
                        <TradingName>Some name</TradingName>
                    </Operator>
                    <Operator id="op2">
                        <OperatorCode>OTHER_CODE</OperatorCode>
                        <TradingName>Other name</TradingName>
                    </Operator>
                </Operators>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let network = load_network(&root).unwrap();
            assert_eq!(network.name, String::from("Some name"));
        }

        #[test]
        fn no_trading_name() {
            let xml = r#"<root>
                <Services>
                    <Service>
                        <RegisteredOperatorRef>op1</RegisteredOperatorRef>
                    </Service>
                </Services>
                <Operators>
                    <Operator id="op1">
                        <OperatorCode>SOME_CODE</OperatorCode>
                        <OperatorShortName>Some name</OperatorShortName>
                    </Operator>
                </Operators>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let network = load_network(&root).unwrap();
            assert_eq!(network.name, String::from("Some name"));
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'RegisteredOperatorRef\\' in element \\'Service\\'"
        )]
        fn no_operator_ref() {
            let xml = r#"<root>
                <Services>
                    <Service />
                </Services>
                <Operators>
                    <Operator>
                        <TradingName>Some name</TradingName>
                    </Operator>
                </Operators>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            load_network(&root).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'OperatorCode\\' in element \\'Operator\\'"
        )]
        fn no_id() {
            let xml = r#"<root>
                <Services>
                    <Service>
                        <RegisteredOperatorRef>op1</RegisteredOperatorRef>
                    </Service>
                </Services>
                <Operators>
                    <Operator id="op1">
                        <TradingName>Some name</TradingName>
                    </Operator>
                </Operators>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            load_network(&root).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'OperatorShortName\\' in element \\'Operator\\'"
        )]
        fn no_name() {
            let xml = r#"<root>
                <Services>
                    <Service>
                        <RegisteredOperatorRef>op1</RegisteredOperatorRef>
                    </Service>
                </Services>
                <Operators>
                    <Operator id="op1">
                        <OperatorCode>SOME_CODE</OperatorCode>
                    </Operator>
                </Operators>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            load_network(&root).unwrap();
        }
    }

    mod load_companies {
        use super::*;

        #[test]
        fn has_company() {
            let xml = r#"<root>
                <Operators>
                    <Operator>
                        <OperatorCode>SOME_CODE</OperatorCode>
                        <OperatorShortName>Some name</OperatorShortName>
                    </Operator>
                    <Operator>
                        <OperatorCode>OTHER_CODE</OperatorCode>
                        <OperatorShortName>Other name</OperatorShortName>
                    </Operator>
                </Operators>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let companies = load_companies(&root).unwrap();
            let company = companies.get("SOME_CODE").unwrap();
            assert_eq!(company.name, String::from("Some name"));
            let company = companies.get("OTHER_CODE").unwrap();
            assert_eq!(company.name, String::from("Other name"));
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'OperatorCode\\' in element \\'Operator\\'"
        )]
        fn no_id() {
            let xml = r#"<root>
                <Operators>
                    <Operator>
                        <OperatorShortName>Some name</OperatorShortName>
                    </Operator>
                </Operators>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            load_companies(&root).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'OperatorShortName\\' in element \\'Operator\\'"
        )]
        fn no_name() {
            let xml = r#"<root>
                <Operators>
                    <Operator>
                        <OperatorCode>SOME_CODE</OperatorCode>
                    </Operator>
                </Operators>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            load_companies(&root).unwrap();
        }
    }
}
