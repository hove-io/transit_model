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

use crate::xml_builder::{Element, Node};
use crate::{
    netex_france::exporter::{Exporter, ObjectType},
    objects::{Calendar, Date},
    Model, Result,
};
use anyhow::bail;
use chrono::prelude::*;
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
            .attr("order", "1")
            .attr("version", "any")
            .append(self.generate_operating_period_ref(&calendar.id))
            .append(self.generate_day_type_ref(&calendar.id))
            .build()
    }

    fn export_uic_operating_period(&self, calendar: &'a Calendar) -> Result<Element> {
        if let (Some(from_date), Some(to_date)) = (
            calendar.dates.iter().next(),
            calendar.dates.iter().next_back(),
        ) {
            let from_date_element = Self::generate_date(
                "FromDate",
                *from_date,
                NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            );
            let to_date_element = Self::generate_date(
                "ToDate",
                *to_date,
                NaiveTime::from_hms_opt(23, 59, 59).unwrap(),
            );
            let valid_day_bits = Self::generate_valid_day_bits(&calendar.dates);
            let uic_operating_period = Element::builder(ObjectType::UicOperatingPeriod.to_string())
                .attr(
                    "id",
                    Exporter::generate_id(&calendar.id, ObjectType::UicOperatingPeriod),
                )
                .attr("version", "any")
                .append(from_date_element)
                .append(to_date_element)
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

    fn generate_date(element_name: &'a str, date: Date, time: NaiveTime) -> Element {
        let date_string = date.and_time(time).format("%Y-%m-%dT%H:%M:%S").to_string();
        Element::builder(element_name)
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
