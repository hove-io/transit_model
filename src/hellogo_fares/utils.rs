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

use crate::{netex_utils, objects::Date, Result};
use chrono::NaiveDate;
use failure::{bail, format_err, Error};
use minidom::Element;
use minidom_ext::OnlyChildElementExt;
use rust_decimal::Decimal;
use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

#[derive(Debug, Eq, Hash, PartialEq)]
pub enum FareFrameType {
    UnitPrice,
    DistanceMatrix,
    DirectPriceMatrix,
}

impl Display for FareFrameType {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            FareFrameType::UnitPrice => write!(f, "UnitPrice"),
            FareFrameType::DistanceMatrix => write!(f, "DistanceMatrix"),
            FareFrameType::DirectPriceMatrix => write!(f, "DirectPriceMatrix"),
        }
    }
}

impl FromStr for FareFrameType {
    type Err = Error;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "UnitPrice" => Ok(FareFrameType::UnitPrice),
            "DistanceMatrix" => Ok(FareFrameType::DistanceMatrix),
            "DirectPriceMatrix" => Ok(FareFrameType::DirectPriceMatrix),
            _ => bail!("Failed to convert '{}' into a FareFrameType", s),
        }
    }
}

pub fn get_fare_frame_type(frame: &Element) -> Result<FareFrameType> {
    let fare_structure = frame
        .try_only_child("fareStructures")
        .map_err(|e| format_err!("{}", e))?
        .try_only_child("FareStructure")
        .map_err(|e| format_err!("{}", e))?;
    netex_utils::get_value_in_keylist(fare_structure, "FareStructureType")
}

pub fn get_amount_units_factor(element: &Element) -> Result<Decimal> {
    let amount = element
        .try_only_child("Amount")
        .map_err(|e| format_err!("{}", e))?
        .text();
    let amount: Decimal = amount
        .parse()
        .map_err(|_| format_err!("Failed to convert '{}' into a 'Decimal'", amount))?;
    let units = element
        .try_only_child("Units")
        .map_err(|e| format_err!("{}", e))?
        .text();
    let units: Decimal = units
        .parse()
        .map_err(|_| format_err!("Failed to convert '{}' into a 'Decimal'", units))?;
    Ok(amount * units)
}

pub fn get_unit_price(unit_price_frame: &Element) -> Result<Decimal> {
    let geographic_interval_price = unit_price_frame
        .try_only_child("fareStructures")
        .map_err(|e| format_err!("{}", e))?
        .try_only_child("FareStructure")
        .map_err(|e| format_err!("{}", e))?
        .try_only_child("geographicalIntervals")
        .map_err(|e| format_err!("{}", e))?
        .try_only_child("GeographicalInterval")
        .map_err(|e| format_err!("{}", e))?
        .try_only_child("prices")
        .map_err(|e| format_err!("{}", e))?
        .try_only_child("GeographicalIntervalPrice")
        .map_err(|e| format_err!("{}", e))?;
    Ok(get_amount_units_factor(geographic_interval_price)?)
}

const DATE_TIME_FORMAT: &str = "%+";
pub fn get_validity(resource_frame: &Element) -> Result<(Date, Date)> {
    fn extract_date(element: &Element, date_element_name: &str) -> Result<Date> {
        let date_str = element
            .try_only_child(date_element_name)
            .map_err(|e| format_err!("{}", e))?
            .text();
        let date = NaiveDate::parse_from_str(date_str.as_str(), DATE_TIME_FORMAT)
            .map_err(|_| format_err!("Failed to convert '{}' into a 'Date'", date_str))?;
        Ok(date)
    }

    if resource_frame.name() != "ResourceFrame" {
        bail!(
            "Failed to get validity from a '{}', it should be a 'ResourceFrame'",
            resource_frame.name()
        );
    }
    let version = resource_frame
        .try_only_child("versions")
        .map_err(|e| format_err!("{}", e))?
        .try_only_child("Version")
        .map_err(|e| format_err!("{}", e))?;
    let validity_start_date = extract_date(version, "StartDate")?;
    let validity_end_date = extract_date(version, "EndDate")?;
    Ok((validity_start_date, validity_end_date))
}

pub fn get_currency(fare_frame: &Element) -> Result<String> {
    let currency = fare_frame
        .try_only_child("FrameDefaults")
        .map_err(|e| format_err!("{}", e))?
        .try_only_child("DefaultCurrency")
        .map_err(|e| format_err!("{}", e))?
        .text();
    if iso4217::alpha3(currency.as_str()).is_none() {
        bail!("Failed to validate '{}' as a currency", currency)
    }
    Ok(currency)
}

pub fn get_distance_matrix_elements<'a>(fare_frame: &'a Element) -> Result<Vec<&'a Element>> {
    let distance_matrix_elements = fare_frame
        .try_only_child("fareStructures")
        .map_err(|e| format_err!("{}", e))?
        .try_only_child("FareStructure")
        .map_err(|e| format_err!("{}", e))?
        .try_only_child("distanceMatrixElements")
        .map_err(|e| format_err!("{}", e))?
        .children()
        .collect();
    Ok(distance_matrix_elements)
}

#[cfg(test)]
mod tests {
    use super::*;

    mod fare_frame_type {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn parse_fare_frame_type() {
            let frame_type: FareFrameType = "UnitPrice".parse().unwrap();
            assert_eq!(FareFrameType::UnitPrice, frame_type)
        }

        #[test]
        #[should_panic(expected = "Failed to convert \\'NotAFareFrameType\\' into a FareFrameType")]
        fn parse_invalid_fare_frame_type() {
            "NotAFareFrameType".parse::<FareFrameType>().unwrap();
        }
    }

    mod get_fare_frame_type {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn parse_fare_frame_type() {
            let xml = r#"<root>
                    <fareStructures>
                        <FareStructure>
                            <KeyList>
                                <KeyValue>
                                    <Key>FareStructureType</Key>
                                    <Value>DistanceMatrix</Value>
                                </KeyValue>
                            </KeyList>
                        </FareStructure>
                    </fareStructures>
                </root>"#;
            let root: Element = xml.parse().unwrap();
            let fare_frame_type = get_fare_frame_type(&root).unwrap();
            assert_eq!(fare_frame_type, FareFrameType::DistanceMatrix);
        }

        #[test]
        #[should_panic(
            expected = "No children with name \\'fareStructures\\' in Element \\'root\\'"
        )]
        fn missing_fare_structures() {
            let xml = r#"<root />"#;
            let root: Element = xml.parse().unwrap();
            get_fare_frame_type(&root).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "No children with name \\'FareStructure\\' in Element \\'fareStructures\\'"
        )]
        fn missing_fare_structure() {
            let xml = r#"<root>
                    <fareStructures />
                </root>"#;
            let root: Element = xml.parse().unwrap();
            get_fare_frame_type(&root).unwrap();
        }
    }

    mod amount_unit_factor {
        use super::*;
        use pretty_assertions::assert_eq;
        use rust_decimal_macros::dec;

        #[test]
        fn has_amount_units() {
            let xml = r#"<root>
                    <Amount>42</Amount>
                    <Units>0.5</Units>
                </root>"#;
            let root: Element = xml.parse().unwrap();
            let factor = get_amount_units_factor(&root).unwrap();
            assert_eq!(dec!(21.0), factor);
        }

        #[test]
        #[should_panic(expected = "No children with name \\'Amount\\' in Element \\'root\\'")]
        fn no_amount() {
            let xml = r#"<root>
                    <Units>0.5</Units>
                </root>"#;
            let root: Element = xml.parse().unwrap();
            get_amount_units_factor(&root).unwrap();
        }

        #[test]
        #[should_panic(expected = "No children with name \\'Units\\' in Element \\'root\\'")]
        fn no_units() {
            let xml = r#"<root>
                    <Amount>42</Amount>
                </root>"#;
            let root: Element = xml.parse().unwrap();
            get_amount_units_factor(&root).unwrap();
        }
    }

    mod unit_price {
        use super::*;
        use pretty_assertions::assert_eq;
        use rust_decimal_macros::dec;

        #[test]
        fn extract_unit_price() {
            let xml = r#"<FareFrame>
                    <fareStructures>
                        <FareStructure>
                            <geographicalIntervals>
                                <GeographicalInterval>
                                    <prices>
                                        <GeographicalIntervalPrice>
                                            <Amount>1.100</Amount>
                                            <Units>0.01</Units>
                                        </GeographicalIntervalPrice>
                                    </prices>
                                </GeographicalInterval>
                            </geographicalIntervals>
                        </FareStructure>
                    </fareStructures>
                </FareFrame>"#;
            let unit_price_frame: Element = xml.parse().unwrap();
            let unit_price = get_unit_price(&unit_price_frame).unwrap();
            assert_eq!(dec!(0.011), unit_price);
        }

        #[test]
        #[should_panic(
            expected = "No children with name \\'fareStructures\\' in Element \\'FareFrame\\'"
        )]
        fn no_fare_structures() {
            let xml = r#"<FareFrame />"#;
            let unit_price_frame: Element = xml.parse().unwrap();
            get_unit_price(&unit_price_frame).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "No children with name \\'FareStructure\\' in Element \\'fareStructures\\'"
        )]
        fn no_fare_structure() {
            let xml = r#"<FareFrame>
                    <fareStructures />
                </FareFrame>"#;
            let unit_price_frame: Element = xml.parse().unwrap();
            get_unit_price(&unit_price_frame).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Multiple children with name \\'FareStructure\\' in Element \\'fareStructures\\'"
        )]
        fn multiple_fare_structure() {
            let xml = r#"<FareFrame>
                    <fareStructures>
                        <FareStructure />
                        <FareStructure />
                    </fareStructures>
                </FareFrame>"#;
            let unit_price_frame: Element = xml.parse().unwrap();
            get_unit_price(&unit_price_frame).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "No children with name \\'geographicalIntervals\\' in Element \\'FareStructure\\'"
        )]
        fn no_geographical_intervals() {
            let xml = r#"<FareFrame>
                    <fareStructures>
                        <FareStructure />
                    </fareStructures>
                </FareFrame>"#;
            let unit_price_frame: Element = xml.parse().unwrap();
            get_unit_price(&unit_price_frame).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "No children with name \\'GeographicalInterval\\' in Element \\'geographicalIntervals\\'"
        )]
        fn no_geographical_interval() {
            let xml = r#"<FareFrame>
                    <fareStructures>
                        <FareStructure>
                            <geographicalIntervals />
                        </FareStructure>
                    </fareStructures>
                </FareFrame>"#;
            let unit_price_frame: Element = xml.parse().unwrap();
            get_unit_price(&unit_price_frame).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Multiple children with name \\'GeographicalInterval\\' in Element \\'geographicalIntervals\\'"
        )]
        fn multiple_geographical_interval() {
            let xml = r#"<FareFrame>
                    <fareStructures>
                        <FareStructure>
                            <geographicalIntervals>
                                <GeographicalInterval />
                                <GeographicalInterval />
                            </geographicalIntervals>
                        </FareStructure>
                    </fareStructures>
                </FareFrame>"#;
            let unit_price_frame: Element = xml.parse().unwrap();
            get_unit_price(&unit_price_frame).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "No children with name \\'prices\\' in Element \\'GeographicalInterval\\'"
        )]
        fn no_prices() {
            let xml = r#"<FareFrame>
                    <fareStructures>
                        <FareStructure>
                            <geographicalIntervals>
                                <GeographicalInterval />
                            </geographicalIntervals>
                        </FareStructure>
                    </fareStructures>
                </FareFrame>"#;
            let unit_price_frame: Element = xml.parse().unwrap();
            get_unit_price(&unit_price_frame).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "No children with name \\'GeographicalIntervalPrice\\' in Element \\'prices\\'"
        )]
        fn no_geographical_interval_price() {
            let xml = r#"<FareFrame>
                    <fareStructures>
                        <FareStructure>
                            <geographicalIntervals>
                                <GeographicalInterval>
                                    <prices />
                                </GeographicalInterval>
                            </geographicalIntervals>
                        </FareStructure>
                    </fareStructures>
                </FareFrame>"#;
            let unit_price_frame: Element = xml.parse().unwrap();
            get_unit_price(&unit_price_frame).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Multiple children with name \\'GeographicalIntervalPrice\\' in Element \\'prices\\'"
        )]
        fn multiple_geographical_interval_price() {
            let xml = r#"<FareFrame>
                    <fareStructures>
                        <FareStructure>
                            <geographicalIntervals>
                                <GeographicalInterval>
                                    <prices>
                                        <GeographicalIntervalPrice />
                                        <GeographicalIntervalPrice />
                                    </prices>
                                </GeographicalInterval>
                            </geographicalIntervals>
                        </FareStructure>
                    </fareStructures>
                </FareFrame>"#;
            let unit_price_frame: Element = xml.parse().unwrap();
            get_unit_price(&unit_price_frame).unwrap();
        }
    }

    mod validity {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn extract_validity() {
            let xml = r#"<ResourceFrame>
                    <versions>
                        <Version>
                            <StartDate>2019-01-01T00:00:00.0Z</StartDate>
                            <EndDate>2019-12-31T00:00:00.0Z</EndDate>
                        </Version>
                    </versions>
                </ResourceFrame>"#;
            let resource_frame: Element = xml.parse().unwrap();
            let (start, end) = get_validity(&resource_frame).unwrap();
            assert_eq!(NaiveDate::from_ymd(2019, 1, 1), start);
            assert_eq!(NaiveDate::from_ymd(2019, 12, 31), end);
        }

        #[test]
        #[should_panic(
            expected = "Failed to get validity from a \\'ServiceFrame\\', it should be a \\'ResourceFrame\\'"
        )]
        fn incorrect_element() {
            let xml = r#"<ServiceFrame />"#;
            let resource_frame: Element = xml.parse().unwrap();
            get_validity(&resource_frame).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "No children with name \\'versions\\' in Element \\'ResourceFrame\\'"
        )]
        fn no_versions() {
            let xml = r#"<ResourceFrame />"#;
            let resource_frame: Element = xml.parse().unwrap();
            get_validity(&resource_frame).unwrap();
        }

        #[test]
        #[should_panic(expected = "No children with name \\'Version\\' in Element \\'versions\\'")]
        fn no_version() {
            let xml = r#"<ResourceFrame>
                    <versions />
                </ResourceFrame>"#;
            let resource_frame: Element = xml.parse().unwrap();
            get_validity(&resource_frame).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Multiple children with name \\'Version\\' in Element \\'versions\\'"
        )]
        fn multiple_version() {
            let xml = r#"<ResourceFrame>
                    <versions>
                        <Version />
                        <Version />
                    </versions>
                </ResourceFrame>"#;
            let resource_frame: Element = xml.parse().unwrap();
            get_validity(&resource_frame).unwrap();
        }

        #[test]
        #[should_panic(expected = "No children with name \\'EndDate\\' in Element \\'Version\\'")]
        fn no_end() {
            let xml = r#"<ResourceFrame>
                    <versions>
                        <Version>
                            <StartDate>2019-01-01T00:00:00.0Z</StartDate>
                        </Version>
                    </versions>
                </ResourceFrame>"#;
            let resource_frame: Element = xml.parse().unwrap();
            get_validity(&resource_frame).unwrap();
        }

        #[test]
        #[should_panic(expected = "No children with name \\'StartDate\\' in Element \\'Version\\'")]
        fn no_start() {
            let xml = r#"<ResourceFrame>
                    <versions>
                        <Version>
                            <EndDate>2019-12-31T00:00:00.0Z</EndDate>
                        </Version>
                    </versions>
                </ResourceFrame>"#;
            let resource_frame: Element = xml.parse().unwrap();
            get_validity(&resource_frame).unwrap();
        }

        #[test]
        #[should_panic(expected = "Failed to convert \\'Not A Valid Date\\' into a \\'Date\\'")]
        fn invalid_date() {
            let xml = r#"<ResourceFrame>
                    <versions>
                        <Version>
                            <StartDate>Not A Valid Date</StartDate>
                            <EndDate>2019-12-31T00:00:00.0Z</EndDate>
                        </Version>
                    </versions>
                </ResourceFrame>"#;
            let resource_frame: Element = xml.parse().unwrap();
            get_validity(&resource_frame).unwrap();
        }
    }

    mod currency {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn extract_currency() {
            let xml = r#"<FareFrame>
                    <FrameDefaults>
                        <DefaultCurrency>EUR</DefaultCurrency>
                    </FrameDefaults>
                </FareFrame>"#;
            let fare_frame: Element = xml.parse().unwrap();
            let currency = get_currency(&fare_frame).unwrap();
            assert_eq!("EUR", currency);
        }

        #[test]
        #[should_panic(
            expected = "No children with name \\'FrameDefaults\\' in Element \\'FareFrame\\'"
        )]
        fn no_frame_defaults() {
            let xml = r#"<FareFrame />"#;
            let fare_frame: Element = xml.parse().unwrap();
            get_currency(&fare_frame).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "No children with name \\'DefaultCurrency\\' in Element \\'FrameDefaults\\'"
        )]
        fn no_default_currency() {
            let xml = r#"<FareFrame>
                    <FrameDefaults />
                </FareFrame>"#;
            let fare_frame: Element = xml.parse().unwrap();
            get_currency(&fare_frame).unwrap();
        }

        #[test]
        #[should_panic(expected = "Failed to validate \\'Not A Valid Currency\\' as a currency")]
        fn invalid_currency() {
            let xml = r#"<FareFrame>
                    <FrameDefaults>
                        <DefaultCurrency>Not A Valid Currency</DefaultCurrency>
                    </FrameDefaults>
                </FareFrame>"#;
            let fare_frame: Element = xml.parse().unwrap();
            get_currency(&fare_frame).unwrap();
        }
    }

    mod distance_matrix_elements {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn has_elements() {
            let xml = r#"<root>
                    <fareStructures>
                        <FareStructure>
                            <distanceMatrixElements>
                                <child />
                                <child />
                            </distanceMatrixElements>
                        </FareStructure>
                    </fareStructures>
                </root>"#;
            let root: Element = xml.parse().unwrap();
            let distance_matrix_elements = get_distance_matrix_elements(&root).unwrap();
            assert_eq!(2, distance_matrix_elements.len());
        }

        #[test]
        fn has_no_element() {
            let xml = r#"<root>
                    <fareStructures>
                        <FareStructure>
                            <distanceMatrixElements />
                        </FareStructure>
                    </fareStructures>
                </root>"#;
            let root: Element = xml.parse().unwrap();
            let distance_matrix_elements = get_distance_matrix_elements(&root).unwrap();
            assert_eq!(0, distance_matrix_elements.len());
        }

        #[test]
        #[should_panic(
            expected = "No children with name \\'fareStructures\\' in Element \\'root\\'"
        )]
        fn no_fare_structures() {
            let xml = r#"<root>"#;
            let root: Element = xml.parse().unwrap();
            get_distance_matrix_elements(&root).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "No children with name \\'FareStructure\\' in Element \\'fareStructures\\'"
        )]
        fn no_fare_structure() {
            let xml = r#"<root>
                    <fareStructures>
                    </fareStructures>
                </root>"#;
            let root: Element = xml.parse().unwrap();
            get_distance_matrix_elements(&root).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Multiple children with name \\'FareStructure\\' in Element \\'fareStructures\\'"
        )]
        fn multiple_fare_structure() {
            let xml = r#"<root>
                    <fareStructures>
                        <FareStructure />
                        <FareStructure />
                    </fareStructures>
                </root>"#;
            let root: Element = xml.parse().unwrap();
            get_distance_matrix_elements(&root).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "No children with name \\'distanceMatrixElements\\' in Element \\'FareStructure\\'"
        )]
        fn no_distance_matrix_elements() {
            let xml = r#"<root>
                    <fareStructures>
                        <FareStructure />
                    </fareStructures>
                </root>"#;
            let root: Element = xml.parse().unwrap();
            get_distance_matrix_elements(&root).unwrap();
        }
    }
}
