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

use crate::hellogo_fares::TryOnlyChild;
use crate::objects::Date;
use crate::Result;
use chrono::NaiveDate;
use failure::{bail, format_err, Error};
use minidom::Element;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Debug, Eq, Hash, PartialEq)]
pub enum FrameType {
    Resource,
    Service,
    UnitPrice,
    DistanceMatrix,
    DirectPriceMatrix,
}

impl Display for FrameType {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            FrameType::Resource => write!(f, "Resource"),
            FrameType::Service => write!(f, "Service"),
            FrameType::UnitPrice => write!(f, "UnitPrice"),
            FrameType::DistanceMatrix => write!(f, "DistanceMatrix"),
            FrameType::DirectPriceMatrix => write!(f, "DirectPriceMatrix"),
        }
    }
}

impl FromStr for FrameType {
    type Err = Error;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "UnitPrice" => Ok(FrameType::UnitPrice),
            "DistanceMatrix" => Ok(FrameType::DistanceMatrix),
            "DirectPriceMatrix" => Ok(FrameType::DirectPriceMatrix),
            _ => bail!("Failed to convert '{}' into a FrameType", s),
        }
    }
}

pub fn get_value_in_keylist<F>(element: &Element, key: &str) -> Result<F>
where
    F: FromStr,
{
    let values = element
        .try_only_child("KeyList")?
        .children()
        .filter(|key_value| match key_value.try_only_child("Key") {
            Ok(k) => k.text() == key,
            _ => false,
        })
        .map(|key_value| key_value.try_only_child("Value"))
        .collect::<Result<Vec<_>>>()?;
    if values.len() != 1 {
        bail!(
            "Failed to find a unique key '{}' in '{}'",
            key,
            element.name()
        )
    }
    values[0]
        .text()
        .parse()
        .map_err(|_| format_err!("Failed to get the value out of 'KeyList' for key '{}'", key))
}

pub fn get_amount_units_factor(element: &Element) -> Result<Decimal> {
    let amount = element.try_only_child("Amount")?.text();
    let amount: Decimal = amount
        .parse()
        .map_err(|_| format_err!("Failed to convert '{}' into a 'Decimal'", amount))?;
    let units = element.try_only_child("Units")?.text();
    let units: Decimal = units
        .parse()
        .map_err(|_| format_err!("Failed to convert '{}' into a 'Decimal'", units))?;
    Ok(amount * units)
}

pub fn get_unit_price(unit_price_frame: &Element) -> Result<Decimal> {
    let geographic_interval_price = unit_price_frame
        .try_only_child("fareStructures")?
        .try_only_child("FareStructure")?
        .try_only_child("geographicalIntervals")?
        .try_only_child("GeographicalInterval")?
        .try_only_child("prices")?
        .try_only_child("GeographicalIntervalPrice")?;
    Ok(get_amount_units_factor(geographic_interval_price)?)
}

const DATE_TIME_FORMAT: &str = "%+";
pub fn get_validity(resource_frame: &Element) -> Result<(Date, Date)> {
    fn extract_date(element: &Element, date_element_name: &str) -> Result<Date> {
        let date_str = element.try_only_child(date_element_name)?.text();
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
        .try_only_child("versions")?
        .try_only_child("Version")?;
    let validity_start_date = extract_date(version, "StartDate")?;
    let validity_end_date = extract_date(version, "EndDate")?;
    Ok((validity_start_date, validity_end_date))
}

pub fn get_currency(fare_frame: &Element) -> Result<String> {
    let currency = fare_frame
        .try_only_child("FrameDefaults")?
        .try_only_child("DefaultCurrency")?
        .text();
    if iso4217::alpha3(currency.as_str()).is_none() {
        bail!("Failed to validate '{}' as a currency", currency)
    }
    Ok(currency)
}

pub fn get_frame_type(frame: &Element) -> Result<FrameType> {
    if frame.name() == "ServiceFrame" {
        return Ok(FrameType::Service);
    }
    if frame.name() == "ResourceFrame" {
        return Ok(FrameType::Resource);
    }
    let fare_structure = frame
        .try_only_child("fareStructures")?
        .try_only_child("FareStructure")?;
    let frame_type: FrameType = get_value_in_keylist(fare_structure, "FareStructureType")?;
    Ok(frame_type)
}

pub fn get_fare_frames<'a>(root: &'a Element) -> Result<HashMap<FrameType, Vec<&'a Element>>> {
    root.try_only_child("dataObjects")?
        .try_only_child("CompositeFrame")?
        .try_only_child("frames")?
        .children()
        .try_fold(HashMap::new(), |mut map, frame| {
            let frame_type = get_frame_type(frame)?;
            map.entry(frame_type).or_insert_with(Vec::new).push(frame);
            Ok(map)
        })
}

pub fn get_only_frame<'a>(
    frames: &'a HashMap<FrameType, Vec<&'a Element>>,
    frame_type: FrameType,
) -> Result<&'a Element> {
    let frame = frames
        .get(&frame_type)
        .ok_or_else(|| format_err!("Failed to find a '{}' frame in the Netex file", frame_type))?;
    if frame.len() == 1 {
        Ok(frame[0])
    } else {
        bail!(
            "Failed to find a unique '{}' frame in the Netex file",
            frame_type
        )
    }
}

pub fn get_distance_matrix_elements<'a>(fare_frame: &'a Element) -> Result<Vec<&'a Element>> {
    let distance_matrix_elements = fare_frame
        .try_only_child("fareStructures")?
        .try_only_child("FareStructure")?
        .try_only_child("distanceMatrixElements")?
        .children()
        .collect();
    Ok(distance_matrix_elements)
}

#[cfg(test)]
mod tests {
    mod frame_type {
        use crate::hellogo_fares::utils::FrameType;
        use pretty_assertions::assert_eq;

        #[test]
        fn parse_valid() {
            let frame_type: FrameType = "UnitPrice".parse().unwrap();
            assert_eq!(frame_type, FrameType::UnitPrice)
        }

        #[test]
        #[should_panic(expected = "Failed to convert \\'NotAFrameType\\' into a FrameType")]
        fn parse_invalid() {
            "NotAFrameType".parse::<FrameType>().unwrap();
        }
    }

    mod value_in_keylist {
        use super::super::get_value_in_keylist;
        use minidom::Element;
        use pretty_assertions::assert_eq;

        #[test]
        fn has_value() {
            let xml = r#"<root>
                    <KeyList>
                        <KeyValue>
                            <Key>key</Key>
                            <Value>42</Value>
                        </KeyValue>
                    </KeyList>
                </root>"#;
            let root: Element = xml.parse().unwrap();
            let value: u32 = get_value_in_keylist(&root, "key").unwrap();
            assert_eq!(value, 42);
        }

        #[test]
        #[should_panic(expected = "Failed to find a child \\'KeyList\\' in element \\'root\\'")]
        fn no_keylist_found() {
            let xml = r#"<root />"#;
            let root: Element = xml.parse().unwrap();
            get_value_in_keylist::<u32>(&root, "key").unwrap();
        }

        #[test]
        #[should_panic(expected = "Failed to find a unique key \\'key\\' in \\'root\\'")]
        fn no_key_found() {
            let xml = r#"<root>
                    <KeyList />
                </root>"#;
            let root: Element = xml.parse().unwrap();
            get_value_in_keylist::<u32>(&root, "key").unwrap();
        }

        #[test]
        #[should_panic(expected = "Failed to find a child \\'Value\\' in element \\'KeyValue\\'")]
        fn no_value_found() {
            let xml = r#"<root>
                    <KeyList>
                        <KeyValue>
                            <Key>key</Key>
                        </KeyValue>
                    </KeyList>
                </root>"#;
            let root: Element = xml.parse().unwrap();
            get_value_in_keylist::<u32>(&root, "key").unwrap();
        }
    }

    mod amount_unit_factor {
        use super::super::get_amount_units_factor;
        use minidom::Element;
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
            assert_eq!(factor, dec!(21.0));
        }

        #[test]
        #[should_panic(expected = "Failed to find a child \\'Amount\\' in element \\'root\\'")]
        fn no_amount() {
            let xml = r#"<root>
                    <Units>0.5</Units>
                </root>"#;
            let root: Element = xml.parse().unwrap();
            get_amount_units_factor(&root).unwrap();
        }

        #[test]
        #[should_panic(expected = "Failed to find a child \\'Units\\' in element \\'root\\'")]
        fn no_units() {
            let xml = r#"<root>
                    <Amount>42</Amount>
                </root>"#;
            let root: Element = xml.parse().unwrap();
            get_amount_units_factor(&root).unwrap();
        }
    }

    mod unit_price {
        use super::super::get_unit_price;
        use minidom::Element;
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
            assert_eq!(unit_price, dec!(0.011));
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'fareStructures\\' in element \\'FareFrame\\'"
        )]
        fn no_fare_structures() {
            let xml = r#"<FareFrame />"#;
            let unit_price_frame: Element = xml.parse().unwrap();
            get_unit_price(&unit_price_frame).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'FareStructure\\' in element \\'fareStructures\\'"
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
            expected = "Failed to find a unique child \\'FareStructure\\' in element \\'fareStructures\\'"
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
            expected = "Failed to find a child \\'geographicalIntervals\\' in element \\'FareStructure\\'"
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
            expected = "Failed to find a child \\'GeographicalInterval\\' in element \\'geographicalIntervals\\'"
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
            expected = "Failed to find a unique child \\'GeographicalInterval\\' in element \\'geographicalIntervals\\'"
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
            expected = "Failed to find a child \\'prices\\' in element \\'GeographicalInterval\\'"
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
            expected = "Failed to find a child \\'GeographicalIntervalPrice\\' in element \\'prices\\'"
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
            expected = "Failed to find a unique child \\'GeographicalIntervalPrice\\' in element \\'prices\\'"
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
        use super::super::get_validity;
        use chrono::NaiveDate;
        use minidom::Element;
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
            assert_eq!(start, NaiveDate::from_ymd(2019, 01, 01));
            assert_eq!(end, NaiveDate::from_ymd(2019, 12, 31));
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
            expected = "Failed to find a child \\'versions\\' in element \\'ResourceFrame\\'"
        )]
        fn no_versions() {
            let xml = r#"<ResourceFrame />"#;
            let resource_frame: Element = xml.parse().unwrap();
            get_validity(&resource_frame).unwrap();
        }

        #[test]
        #[should_panic(expected = "Failed to find a child \\'Version\\' in element \\'versions\\'")]
        fn no_version() {
            let xml = r#"<ResourceFrame>
                    <versions />
                </ResourceFrame>"#;
            let resource_frame: Element = xml.parse().unwrap();
            get_validity(&resource_frame).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a unique child \\'Version\\' in element \\'versions\\'"
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
        #[should_panic(expected = "Failed to find a child \\'EndDate\\' in element \\'Version\\'")]
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
        #[should_panic(
            expected = "Failed to find a child \\'StartDate\\' in element \\'Version\\'"
        )]
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
        use super::super::get_currency;
        use minidom::Element;
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
            assert_eq!(currency, "EUR");
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'FrameDefaults\\' in element \\'FareFrame\\'"
        )]
        fn no_frame_defaults() {
            let xml = r#"<FareFrame />"#;
            let fare_frame: Element = xml.parse().unwrap();
            get_currency(&fare_frame).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'DefaultCurrency\\' in element \\'FrameDefaults\\'"
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

    mod get_frame_type {
        use super::super::get_frame_type;
        use crate::hellogo_fares::utils::FrameType;
        use minidom::Element;
        use pretty_assertions::assert_eq;

        #[test]
        fn is_frame_type() {
            let xml = r#"<ServiceFrame />"#;
            let root: Element = xml.parse().unwrap();
            let frame_type = get_frame_type(&root).unwrap();
            assert_eq!(frame_type, FrameType::Service);
        }

        #[test]
        fn has_frame_type() {
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
            let frame_type = get_frame_type(&root).unwrap();
            assert_eq!(frame_type, FrameType::DistanceMatrix);
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'fareStructures\\' in element \\'root\\'"
        )]
        fn fare_structures_not_found() {
            let xml = r#"<root />"#;
            let root: Element = xml.parse().unwrap();
            get_frame_type(&root).unwrap();
        }
    }

    mod frames {
        use super::super::get_fare_frames;
        use crate::hellogo_fares::utils::FrameType;
        use minidom::Element;
        use pretty_assertions::assert_eq;

        #[test]
        fn some_frame() {
            let xml = r#"<root>
                    <dataObjects>
                        <CompositeFrame>
                            <frames>
                                <FareFrame>
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
                                </FareFrame>
                                <FareFrame>
                                    <fareStructures>
                                        <FareStructure>
                                            <KeyList>
                                                <KeyValue>
                                                    <Key>FareStructureType</Key>
                                                    <Value>UnitPrice</Value>
                                                </KeyValue>
                                            </KeyList>
                                        </FareStructure>
                                    </fareStructures>
                                </FareFrame>
                                <FareFrame>
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
                                </FareFrame>
                            </frames>
                        </CompositeFrame>
                    </dataObjects>
                </root>"#;
            let root: Element = xml.parse().unwrap();
            let frames = get_fare_frames(&root).unwrap();
            assert_eq!(frames.keys().count(), 2);
            assert_eq!(frames.get(&FrameType::UnitPrice).unwrap().len(), 1);
            assert_eq!(frames.get(&FrameType::DistanceMatrix).unwrap().len(), 2);
        }

        #[test]
        #[should_panic(expected = "Failed to find a child \\'dataObjects\\' in element \\'root\\'")]
        fn no_data_objects() {
            let xml = r#"<root />"#;
            let root: Element = xml.parse().unwrap();
            get_fare_frames(&root).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'CompositeFrame\\' in element \\'dataObjects\\'"
        )]
        fn no_composite_frame() {
            let xml = r#"<root>
                    <dataObjects />
                </root>"#;
            let root: Element = xml.parse().unwrap();
            get_fare_frames(&root).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a unique child \\'CompositeFrame\\' in element \\'dataObjects\\'"
        )]
        fn multiple_composite_frames() {
            let xml = r#"<root>
                    <dataObjects>
                        <CompositeFrame />
                        <CompositeFrame />
                    </dataObjects>
                </root>"#;
            let root: Element = xml.parse().unwrap();
            get_fare_frames(&root).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'frames\\' in element \\'CompositeFrame\\'"
        )]
        fn no_frames() {
            let xml = r#"<root>
                    <dataObjects>
                        <CompositeFrame />
                    </dataObjects>
                </root>"#;
            let root: Element = xml.parse().unwrap();
            get_fare_frames(&root).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'KeyList\\' in element \\'FareStructure\\'"
        )]
        fn fold_error() {
            let xml = r#"<root>
                    <dataObjects>
                        <CompositeFrame>
                            <frames>
                                <FareFrame>
                                    <fareStructures>
                                        <FareStructure />
                                    </fareStructures>
                                </FareFrame>
                            </frames>
                        </CompositeFrame>
                    </dataObjects>
                </root>"#;
            let root: Element = xml.parse().unwrap();
            get_fare_frames(&root).unwrap();
        }
    }

    mod unit_price_frame {
        use super::super::get_only_frame;
        use crate::hellogo_fares::utils::FrameType;
        use minidom::Element;
        use pretty_assertions::assert_eq;
        use std::collections::HashMap;

        #[test]
        fn one_unit_price_frame() {
            let mut frames = HashMap::new();
            let unit_price_frame: Element = r#"<frame xmlns="test" />"#.parse().unwrap();
            frames.insert(FrameType::UnitPrice, vec![&unit_price_frame]);
            let unit_price_frame = get_only_frame(&frames, FrameType::UnitPrice).unwrap();
            assert_eq!(unit_price_frame.name(), "frame");
        }

        #[test]
        #[should_panic(expected = "Failed to find a \\'DistanceMatrix\\' frame in the Netex file")]
        fn no_unit_price_frame() {
            let frames = HashMap::new();
            get_only_frame(&frames, FrameType::DistanceMatrix).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a unique \\'UnitPrice\\' frame in the Netex file"
        )]
        fn multiple_unit_price_frame() {
            let mut frames = HashMap::new();
            let unit_price_frame: Element = r#"<frame xmlns="test" />"#.parse().unwrap();
            frames.insert(
                FrameType::UnitPrice,
                vec![&unit_price_frame, &unit_price_frame],
            );
            get_only_frame(&frames, FrameType::UnitPrice).unwrap();
        }
    }

    mod distance_matrix_elements {
        use super::super::get_distance_matrix_elements;
        use minidom::Element;
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
            assert_eq!(distance_matrix_elements.len(), 2);
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
            assert_eq!(distance_matrix_elements.len(), 0);
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'fareStructures\\' in element \\'root\\'"
        )]
        fn no_fare_structures() {
            let xml = r#"<root>"#;
            let root: Element = xml.parse().unwrap();
            get_distance_matrix_elements(&root).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a child \\'FareStructure\\' in element \\'fareStructures\\'"
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
            expected = "Failed to find a unique child \\'FareStructure\\' in element \\'fareStructures\\'"
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
            expected = "Failed to find a child \\'distanceMatrixElements\\' in element \\'FareStructure\\'"
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
