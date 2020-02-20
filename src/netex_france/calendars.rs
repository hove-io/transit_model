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

use crate::{
    netex_france::exporter::{Exporter, ObjectType},
    objects::{Calendar, Date},
    Model, Result,
};
use chrono::prelude::*;
use failure::bail;
use minidom::{Element, Node};
use std::collections::BTreeSet;

pub struct CalendarExporter<'a> {
    model: &'a Model,
}

// Publicly exposed methods
impl<'a> CalendarExporter<'a> {
    pub fn new(model: &'a Model) -> Self {
        CalendarExporter { model }
    }
    pub fn export(&self) -> Result<Vec<Element>> {
        let day_types_elements = self
            .model
            .calendars
            .values()
            .map(|calendar| self.export_day_type(calendar))
            .collect::<Result<Vec<Element>>>()?;
        let _day_type_assignments_elements = self
            .model
            .calendars
            .values()
            .map(|calendar| self.export_day_type_assignement(calendar))
            .collect::<Result<Vec<Element>>>()?;
        let uic_operating_periods_elements = self
            .model
            .calendars
            .values()
            .map(|calendar| self.export_uic_operating_period(calendar))
            .collect::<Result<Vec<Element>>>()?;
        let mut elements = day_types_elements;
        // TODO: Uncomment once implemented
        // elements.extend(day_type_assignments_elements);
        elements.extend(uic_operating_periods_elements);
        Ok(elements)
    }
}

// Internal methods
impl<'a> CalendarExporter<'a> {
    fn export_day_type(&self, calendar: &'a Calendar) -> Result<Element> {
        let element_builder = Element::builder(ObjectType::DayType.to_string())
            .attr(
                "id",
                Exporter::generate_id(&calendar.id, ObjectType::DayType),
            )
            .attr("version", "any");
        Ok(element_builder.build())
    }

    fn export_day_type_assignement(&self, _calendar: &'a Calendar) -> Result<Element> {
        let day_type_assignment =
            Element::builder(ObjectType::DayTypeAssignment.to_string()).build();
        Ok(day_type_assignment)
    }

    fn export_uic_operating_period(&self, calendar: &'a Calendar) -> Result<Element> {
        if let Some(from_date) = calendar.dates.iter().next() {
            let from_date = Self::generate_from_date(*from_date);
            let valid_day_bits = Self::generate_valid_day_bits(&calendar.dates);
            let uic_operating_period = Element::builder(ObjectType::UicOperatingPeriod.to_string())
                .attr(
                    "id",
                    Exporter::generate_id(&calendar.id, ObjectType::UicOperatingPeriod),
                )
                .attr("version", "any")
                .append(from_date)
                .append(valid_day_bits)
                .build();
            Ok(uic_operating_period)
        } else {
            bail!(
                "Calendar '{}' cannot be exported because it contains no date",
                calendar.id
            )
        }
    }

    fn generate_from_date(date: Date) -> Element {
        let date_string = DateTime::<Utc>::from_utc(date.and_hms(0, 0, 0), Utc).to_rfc3339();
        Element::builder("FromDate")
            .append(Node::Text(date_string))
            .build()
    }

    fn generate_valid_day_bits(dates: &'a BTreeSet<Date>) -> Element {
        let valid_day_bits_string = if dates.is_empty() {
            String::new()
        } else {
            dates
                .iter()
                .zip(dates.iter().skip(1))
                .map(|(date_1, date_2)| *date_2 - *date_1)
                .map(|duration| duration.num_days())
                .fold(String::from("1"), |mut valid_day_bits, days_diff| {
                    for _ in 1..days_diff {
                        valid_day_bits += "0"
                    }
                    valid_day_bits += "1";
                    valid_day_bits
                })
        };
        Element::builder("ValidDayBits")
            .append(Node::Text(valid_day_bits_string))
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod valid_day_bits {
        use super::*;
        use crate::minidom_utils::ElementWriter;
        use pretty_assertions::assert_eq;

        fn get_valid_day_bits(element: Element) -> String {
            let writer = ElementWriter::new(element, false);
            let mut buffer = Vec::<u8>::new();
            writer.write(&mut buffer).unwrap();
            String::from_utf8(buffer)
                .unwrap()
                .replace(
                    r#"<?xml version="1.0" encoding="UTF-8"?><ValidDayBits>"#,
                    "",
                )
                .replace(r#"</ValidDayBits>"#, "")
                .to_owned()
        }

        #[test]
        fn empty_validity_pattern() {
            let valid_day_bits_element =
                CalendarExporter::generate_valid_day_bits(&BTreeSet::new());
            assert_eq!("", get_valid_day_bits(valid_day_bits_element));
        }

        #[test]
        fn only_one_date() {
            let dates = vec![NaiveDate::from_ymd(2020, 1, 1)].into_iter().collect();
            let valid_day_bits_element = CalendarExporter::generate_valid_day_bits(&dates);
            assert_eq!("1", get_valid_day_bits(valid_day_bits_element));
        }

        #[test]
        fn successive_dates() {
            let dates = vec![
                NaiveDate::from_ymd(2020, 1, 1),
                NaiveDate::from_ymd(2020, 1, 2),
            ]
            .into_iter()
            .collect();
            let valid_day_bits_element = CalendarExporter::generate_valid_day_bits(&dates);
            assert_eq!("11", get_valid_day_bits(valid_day_bits_element));
        }

        #[test]
        fn not_successive_dates() {
            let dates = vec![
                NaiveDate::from_ymd(2020, 1, 1),
                NaiveDate::from_ymd(2020, 1, 3),
            ]
            .into_iter()
            .collect();
            let valid_day_bits_element = CalendarExporter::generate_valid_day_bits(&dates);
            assert_eq!("101", get_valid_day_bits(valid_day_bits_element));
        }
    }
}
