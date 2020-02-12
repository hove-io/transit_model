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
    objects::Calendar,
    Model, Result,
};
use minidom::Element;

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
        let _uic_operating_periods_elements = self
            .model
            .calendars
            .values()
            .map(|calendar| self.export_uic_operating_period(calendar))
            .collect::<Result<Vec<Element>>>()?;
        let elements = day_types_elements;
        // elements.extend(day_type_assignments_elements);
        // elements.extend(uic_operating_periods_elements);
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

    fn export_uic_operating_period(&self, _calendar: &'a Calendar) -> Result<Element> {
        let uic_operating_period =
            Element::builder(ObjectType::UicOperatingPeriod.to_string()).build();
        Ok(uic_operating_period)
    }
}
