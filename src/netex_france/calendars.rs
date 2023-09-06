// Copyright (C) 2017 Hove and/or its affiliates.
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
use anyhow::bail;
use chrono::prelude::*;
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
            .collect::<Vec<Element>>();
        let day_type_assignments_elements = self
            .model
            .calendars
            .values()
            .map(|calendar| self.export_day_type_assignement(calendar))
            .collect::<Vec<Element>>();
        let uic_operating_periods_elements = self
            .model
            .calendars
            .values()
            .map(|calendar| self.export_uic_operating_period(calendar))
            .collect::<Result<Vec<Element>>>()?;
        let mut elements = day_types_elements;
        elements.extend(day_type_assignments_elements);
        elements.extend(uic_operating_periods_elements);
        Ok(elements)
    }
}

// Internal methods
impl<'a> CalendarExporter<'a> {
    fn export_day_type(&self, calendar: &'a Calendar) -> Element {
        Element::builder(ObjectType::DayType.to_string())
            .attr(
                "id",
                Exporter::generate_id(&calendar.id, ObjectType::DayType),
            )
            .attr("version", "any")
            .build()
    }

    fn export_day_type_assignement(&self, calendar: &'a Calendar) -> Element {
        Element::builder(ObjectType::DayTypeAssignment.to_string())
            .attr(
                "id",
                Exporter::generate_id(&calendar.id, ObjectType::DayTypeAssignment),
            )
            .attr("version", "any")
            .attr("order", "0")
            .append(self.generate_operating_period_ref(&calendar.id))
            .append(self.generate_day_type_ref(&calendar.id))
            .build()
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
        let date_string =
            DateTime::<Utc>::from_naive_utc_and_offset(date.and_hms_opt(0, 0, 0).unwrap(), Utc)
                .to_rfc3339();
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

    fn generate_operating_period_ref(&self, id: &'a str) -> Element {
        Element::builder("OperatingPeriodRef")
            .attr(
                "ref",
                Exporter::generate_id(id, ObjectType::UicOperatingPeriod),
            )
            .build()
    }

    fn generate_day_type_ref(&self, id: &'a str) -> Element {
        Element::builder("DayTypeRef")
            .attr("ref", Exporter::generate_id(id, ObjectType::DayType))
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod valid_day_bits {
        use super::*;
        use pretty_assertions::assert_eq;

        fn get_valid_day_bits(element: Element) -> String {
            element
                .nodes()
                .next()
                .unwrap()
                .as_text()
                .unwrap()
                .to_string()
        }

        #[test]
        fn empty_validity_pattern() {
            let valid_day_bits_element =
                CalendarExporter::generate_valid_day_bits(&BTreeSet::new());
            assert_eq!("", get_valid_day_bits(valid_day_bits_element));
        }

        #[test]
        fn only_one_date() {
            let dates = vec![NaiveDate::from_ymd_opt(2020, 1, 1).unwrap()]
                .into_iter()
                .collect();
            let valid_day_bits_element = CalendarExporter::generate_valid_day_bits(&dates);
            assert_eq!("1", get_valid_day_bits(valid_day_bits_element));
        }

        #[test]
        fn successive_dates() {
            let dates = vec![
                NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(2020, 1, 2).unwrap(),
            ]
            .into_iter()
            .collect();
            let valid_day_bits_element = CalendarExporter::generate_valid_day_bits(&dates);
            assert_eq!("11", get_valid_day_bits(valid_day_bits_element));
        }

        #[test]
        fn not_successive_dates() {
            let dates = vec![
                NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(2020, 1, 3).unwrap(),
            ]
            .into_iter()
            .collect();
            let valid_day_bits_element = CalendarExporter::generate_valid_day_bits(&dates);
            assert_eq!("101", get_valid_day_bits(valid_day_bits_element));
        }
    }
}
