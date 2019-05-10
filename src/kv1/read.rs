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

use crate::common_format::CalendarDate;
use crate::model::Collections;
use crate::objects::{Calendar, CommercialMode, Date, ExceptionType, PhysicalMode};
use crate::read_utils::FileHandler;
use crate::Result;
use chrono::NaiveDate;
use csv;
use failure::ResultExt;
use lazy_static::lazy_static;
use log::info;
use serde_derive::Deserialize;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::result::Result as StdResult;

/// Deserialize kv1 string date (Y-m-d) to NaiveDate
fn de_from_date_string<'de, D>(deserializer: D) -> StdResult<Date, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let s = String::deserialize(deserializer)?;

    NaiveDate::parse_from_str(&s, "%Y-%m-%d").map_err(serde::de::Error::custom)
}

#[derive(Deserialize, Debug)]
struct OPerDay {
    #[serde(rename = "[OrganizationalUnitCode]")]
    org_unit_code: String,
    #[serde(rename = "[ScheduleCode]")]
    schedule_code: String,
    #[serde(rename = "[ScheduleTypeCode]")]
    schedule_type_code: String,
    #[serde(rename = "[ValidDate]", deserialize_with = "de_from_date_string")]
    valid_date: Date,
}

lazy_static! {
    static ref MODES: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("BUS", "Bus");
        m.insert("TRAIN", "Train");
        m.insert("METRO", "Metro");
        m.insert("TRAM", "Tramway");
        m.insert("BOAT", "Ferry");
        m
    };
}

/// Generates calendars
pub fn read_operday<H>(file_handler: &mut H, collections: &mut Collections) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    let file = "OPERDAYXXX.TMI";
    let (reader, path) = file_handler.get_file(file)?;
    info!("Reading {}", file);

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(b'|')
        .trim(csv::Trim::All)
        .from_reader(reader);

    for opd in rdr.deserialize() {
        let opd: OPerDay = opd.with_context(ctx_from_path!(path))?;

        let calendar_date: CalendarDate = CalendarDate {
            service_id: format!(
                "{}:{}:{}",
                opd.org_unit_code, opd.schedule_code, opd.schedule_type_code
            ),
            date: opd.valid_date,
            exception_type: ExceptionType::Add,
        };

        let is_inserted = collections
            .calendars
            .get_mut(&calendar_date.service_id)
            .map(|mut calendar| {
                calendar.dates.insert(calendar_date.date);
            });

        is_inserted.unwrap_or_else(|| {
            let mut dates = BTreeSet::new();
            dates.insert(calendar_date.date);
            collections
                .calendars
                .push(Calendar {
                    id: calendar_date.service_id,
                    dates,
                })
                .unwrap();
        });
    }
    Ok(())
}

/// Generates physical and commercial modes
pub fn make_physical_and_commercial_modes(collections: &mut Collections) {
    for m in MODES.values() {
        collections
            .physical_modes
            .push(PhysicalMode {
                id: m.to_string(),
                name: m.to_string(),
                co2_emission: None,
            })
            .unwrap();
        collections
            .commercial_modes
            .push(CommercialMode {
                id: m.to_string(),
                name: m.to_string(),
            })
            .unwrap();
    }
}

/// Generates stop_points
pub fn read_usrstop_point<H>(file_handler: &mut H, collections: &mut Collections) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    info!("Reading USRSTOPXXX.TMI and POINTXXXXX.TMI");

    // read POINTXXXXX.TMI
    // Generate HashMap<PointCode, (LocationX_EW, LocationY_NS)>

    // Read USRSTOPXXX.TMI and use the HashMap above to get the stop position
    // use proj crate to convert coordinates EPSG:28992 to EPSG:4326

    // collections.stop_points = CollectionWithId::new(stop_points)?;

    Ok(())
}

/// Generates stop_areas
pub fn read_usrstar<H>(file_handler: &mut H, collections: &mut Collections) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    info!("Reading USRSTARXXX.TMI");
    // filter collections.stop_points by sp.parent == UserStopAreaCode to calculate barycenter
    // collections.stop_areas = CollectionWithId::new(stop_areas)?;

    Ok(())
}

/// Generates networks, companies, stop_times, vehicle_journeys, routes and lines
pub fn read_jopa_pujopass_line<H>(file_handler: &mut H, collections: &mut Collections) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    info!("Reading JOPAXXXXXX.TMI, PUJOPASSXX.TMI and LINEXXXXXX.TMI");

    // collections.networks = CollectionWithId::new(networks)?;
    // collections.companies = CollectionWithId::new(companies)?;

    // Check that UserStopCode exists in collections.stop_points?
    // collections.stop_times = CollectionWithId::new(stop_times)?;

    // physical_mode_id = TransportationType in LINEXXXX.TMI where JOPAXXXXXX.(LinePlanningNumber, Direction) == LINEXXXXXX.(LinePlanningNumber, Direction)
    // needs collections.calendars
    // collections.vehicle_journeys = CollectionWithId::new(vehicle_journeys)?;

    // needs vehicles_journeys -> stop_times -> stop_points + stop_areas
    // collections.routes = CollectionWithId::new(routes)?;

    // need routes + stop_areas
    // collections.lines = CollectionWithId::new(lines)?;

    Ok(())
}

/// Generates comments on trips
pub fn read_notice_ntcassgn<H>(file_handler: &mut H, collections: &mut Collections) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    info!("Reading NOTICEXXXX.TMI and NTCASSGNMX.TMI");

    // collections.comments = CollectionWithId::new(comments)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Collections;
    use crate::read_utils::PathFileHandler;
    use crate::test_utils::*;

    #[test]
    #[should_panic]
    fn read_operday_with_invalid_date() {
        let operday_content =
            "[OrganizationalUnitCode]|[ScheduleCode]|[ScheduleTypeCode]|[ValidDate]\n
                2029|1|1|20190428";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "OPERDAYXXX.TMI", operday_content);
            let mut collections = Collections::default();
            super::read_operday(&mut handler, &mut collections).unwrap();
        });
    }

    #[test]
    fn make_physical_and_commercial_modes() {
        let mut collections = Collections::default();
        super::make_physical_and_commercial_modes(&mut collections);

        let expected = [
            ("Bus", "Bus"),
            ("Ferry", "Ferry"),
            ("Metro", "Metro"),
            ("Train", "Train"),
            ("Tramway", "Tramway"),
        ];

        let mut pms: Vec<(&str, &str)> = collections
            .physical_modes
            .values()
            .map(|pm| (pm.id.as_ref(), pm.name.as_ref()))
            .collect();
        pms.sort_unstable_by(|a, b| a.cmp(&b));

        let mut cms: Vec<(&str, &str)> = collections
            .commercial_modes
            .values()
            .map(|cm| (cm.id.as_ref(), cm.name.as_ref()))
            .collect();
        cms.sort_unstable_by(|a, b| a.cmp(&b));

        assert_eq!(pms, expected);
        assert_eq!(cms, expected);
    }
}
