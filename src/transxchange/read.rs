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
    minidom_utils::TryOnlyChild,
    model::{Collections, Model},
    objects::*,
    transxchange::{bank_holidays, bank_holidays::BankHoliday, naptan},
    validity_period, AddPrefix, Result,
};
use chrono::{
    naive::{NaiveDate, MAX_DATE, MIN_DATE},
    Duration,
};
use failure::{bail, format_err};
use lazy_static::lazy_static;
use log::{info, warn};
use minidom::Element;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    convert::TryFrom,
    fs::File,
    io::Read,
    path::Path,
};
use transit_model_collection::CollectionWithId;
use walkdir::WalkDir;
use zip::ZipArchive;

// An XML tag that doesn't exist or a XML tag whose content is empty,
// should be considered the same
const UNDEFINED: &str = "";
const EUROPE_LONDON_TIMEZONE: &str = "Europe/London";
const DEFAULT_MODE: &str = "Bus";

lazy_static! {
    static ref MODES: std::collections::HashMap<&'static str, &'static str> = {
        let mut modes_map = std::collections::HashMap::new();
        modes_map.insert("air", "Air");
        modes_map.insert("bus", DEFAULT_MODE);
        modes_map.insert("coach", "Coach");
        modes_map.insert("ferry", "Ferry");
        modes_map.insert("underground", "Metro");
        modes_map.insert("metro", "Metro");
        modes_map.insert("rail", "Train");
        modes_map.insert("tram", "Tramway");
        modes_map.insert("trolleyBus", "Shuttle");
        modes_map
    };
}

fn get_by_reference<'a>(
    element: &'a Element,
    child_name: &str,
    reference: &str,
) -> Result<&'a Element> {
    element.try_only_child_with_filter(child_name, |e| {
        e.attr("id").filter(|id| *id == reference).is_some()
    })
}

fn get_service_validity_period(
    transxchange: &Element,
    max_end_date: NaiveDate,
) -> Result<ValidityPeriod> {
    let operating_period = transxchange
        .try_only_child("Services")?
        .try_only_child("Service")?
        .try_only_child("OperatingPeriod")?;
    let start_date: Date = operating_period
        .try_only_child("StartDate")?
        .text()
        .parse()?;
    let mut end_date: Date = if let Ok(end_date) = operating_period.try_only_child("EndDate") {
        end_date.text().parse()?
    } else {
        chrono::naive::MAX_DATE
    };
    if end_date > max_end_date {
        end_date = max_end_date;
    }
    Ok(ValidityPeriod {
        start_date,
        end_date,
    })
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
    max_end_date: NaiveDate,
) -> Result<CollectionWithId<Dataset>> {
    let service_validity_period = get_service_validity_period(transxchange, max_end_date)?;
    let mut datasets = datasets.take();
    for dataset in &mut datasets {
        validity_period::update_validity_period(dataset, &service_validity_period);
    }
    CollectionWithId::new(datasets)
}

fn load_network(transxchange: &Element) -> Result<Network> {
    let operator_ref = transxchange
        .try_only_child("Services")?
        .try_only_child("Service")?
        .try_only_child("RegisteredOperatorRef")?
        .text();
    let operator = get_by_reference(
        transxchange.try_only_child("Operators")?,
        "Operator",
        &operator_ref,
    )?;
    let id = operator.try_only_child("OperatorCode")?.text();
    let name = operator
        .try_only_child("TradingName")
        .or_else(|_| operator.try_only_child("OperatorShortName"))
        .map(Element::text)
        .unwrap_or_else(|_| String::from(UNDEFINED))
        .trim()
        .to_string();
    let timezone = Some(String::from(EUROPE_LONDON_TIMEZONE));
    let url = operator.only_child("WebSite").map(Element::text);
    let phone = operator
        .only_child("ContactTelephoneNumber")
        .map(Element::text);
    let network = Network {
        id,
        name,
        timezone,
        url,
        phone,
        ..Default::default()
    };
    Ok(network)
}

fn load_companies(transxchange: &Element) -> Result<CollectionWithId<Company>> {
    let mut companies = CollectionWithId::default();
    for operator in transxchange.try_only_child("Operators")?.children() {
        let id = operator.try_only_child("OperatorCode")?.text();
        let name = operator
            .try_only_child("OperatorShortName")
            .map(Element::text)
            .unwrap_or_else(|_| String::from(UNDEFINED))
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

fn load_commercial_physical_modes(
    transxchange: &Element,
) -> Result<(CommercialMode, PhysicalMode)> {
    let mode = match transxchange
        .try_only_child("Services")?
        .try_only_child("Service")?
        .try_only_child("Mode")
        .map(Element::text)
    {
        Ok(mode) => MODES.get(mode.as_str()).unwrap_or(&DEFAULT_MODE),
        Err(e) => {
            warn!("{} - Default mode '{}' assigned", e, DEFAULT_MODE);
            DEFAULT_MODE
        }
    };
    let commercial_mode = CommercialMode {
        id: mode.to_string(),
        name: mode.to_string(),
    };
    let physical_mode = PhysicalMode {
        id: mode.to_string(),
        name: mode.to_string(),
        ..Default::default()
    };
    Ok((commercial_mode, physical_mode))
}

fn load_lines(
    transxchange: &Element,
    network_id: &str,
    commercial_mode_id: &str,
) -> Result<CollectionWithId<Line>> {
    let service = transxchange
        .try_only_child("Services")?
        .try_only_child("Service")?;
    let service_id = service.try_only_child("ServiceCode")?.text();
    let mut lines = CollectionWithId::default();
    let name = if let Ok(description) = service.try_only_child("Description") {
        description.text().trim().to_string()
    } else {
        String::from(UNDEFINED)
    };
    let standard_service = service.try_only_child("StandardService")?;
    let forward_name = standard_service
        .try_only_child("Destination")?
        .text()
        .trim()
        .to_string();
    let backward_name = standard_service
        .try_only_child("Origin")?
        .text()
        .trim()
        .to_string();
    for line in service.try_only_child("Lines")?.children() {
        if let Some(line_id) = line.attr("id") {
            let id = format!("{}:{}", service_id, line_id);
            let code = Some(line.try_only_child("LineName")?.text().trim().to_string());
            let network_id = network_id.to_string();
            let commercial_mode_id = commercial_mode_id.to_string();
            let name = name.to_string();
            let forward_name = Some(forward_name.clone());
            let backward_name = Some(backward_name.clone());
            let line = Line {
                id,
                code,
                name,
                forward_name,
                backward_name,
                network_id,
                commercial_mode_id,
                ..Default::default()
            };
            let _ = lines.push(line);
        }
    }
    Ok(lines)
}

fn create_route(
    collections: &Collections,
    transxchange: &Element,
    vehicle_journey: &Element,
    lines: &CollectionWithId<Line>,
    stop_times: &[StopTime],
) -> Result<Route> {
    let service = transxchange
        .try_only_child("Services")?
        .try_only_child("Service")?;
    let standard_service = service.try_only_child("StandardService")?;
    let journey_pattern_ref = vehicle_journey.try_only_child("JourneyPatternRef")?.text();
    let direction_type =
        match get_by_reference(standard_service, "JourneyPattern", &journey_pattern_ref)?
            .try_only_child("Direction")?
            .text()
            .as_str()
        {
            "inboundAndOutbound" => "inbound",
            "circular" => "clockwise",
            direction => direction,
        }
        .to_string();
    let service_code = service.try_only_child("ServiceCode")?.text();
    let line_ref = vehicle_journey.try_only_child("LineRef")?.text();
    let line_id = format!("{}:{}", service_code, line_ref);
    if !lines.contains_id(&line_id) {
        bail!(
            "Failed to create route because line {} doesn't exist.",
            line_id
        );
    }
    let id = format!("{}:{}", line_id, direction_type);
    let (first_stop_time, last_stop_time) = stop_times
        .first()
        .and_then(|f| stop_times.last().map(|l| (f, l)))
        .ok_or_else(|| format_err!("Failed to find any StopTime to create the route {}", id))?;
    let first_stop_point = &collections.stop_points[first_stop_time.stop_point_idx];
    let last_stop_point = &collections.stop_points[last_stop_time.stop_point_idx];
    let first_stop_area = collections.stop_areas.get(&first_stop_point.stop_area_id);
    let first_stop_area_name = first_stop_area
        .map(|stop_area| stop_area.name.clone())
        .unwrap_or_else(|| UNDEFINED.to_string());
    let last_stop_area = collections.stop_areas.get(&last_stop_point.stop_area_id);
    let last_stop_area_name = last_stop_area
        .map(|stop_area| stop_area.name.clone())
        .unwrap_or_else(|| UNDEFINED.to_string());
    let name = format!("{} - {}", first_stop_area_name, last_stop_area_name);
    let direction_type = Some(direction_type);
    let destination_id = last_stop_area.map(|stop_area| stop_area.id.clone());
    Ok(Route {
        id,
        name,
        direction_type,
        line_id,
        destination_id,
        ..Default::default()
    })
}

fn generate_calendar_dates(
    operating_profile: &Element,
    bank_holidays: &HashMap<BankHoliday, Vec<Date>>,
    validity_period: &ValidityPeriod,
) -> Result<BTreeSet<Date>> {
    use crate::transxchange::operating_profile::OperatingProfile;
    let operating_profile = OperatingProfile::from(operating_profile);
    let mut bank_holidays = bank_holidays.clone();
    let new_year_days = bank_holidays::get_fixed_days(1, 1, validity_period);
    bank_holidays.insert(BankHoliday::NewYear, new_year_days);
    let january_second_days = bank_holidays::get_fixed_days(2, 1, validity_period);
    bank_holidays.insert(BankHoliday::JanuarySecond, january_second_days);
    let saint_andrews_days = bank_holidays::get_fixed_days(30, 11, validity_period);
    bank_holidays.insert(BankHoliday::SaintAndrews, saint_andrews_days);
    let christmas_days = bank_holidays::get_fixed_days(25, 12, validity_period);
    bank_holidays.insert(BankHoliday::Christmas, christmas_days);
    let boxing_days = bank_holidays::get_fixed_days(26, 12, validity_period);
    bank_holidays.insert(BankHoliday::BoxingDay, boxing_days);
    let dates: BTreeSet<Date> = operating_profile
        .iter_with_bank_holidays_between(&bank_holidays, validity_period)
        .collect();
    Ok(dates)
}

fn create_calendar_dates(
    transxchange: &Element,
    vehicle_journey: &Element,
    bank_holidays: &HashMap<BankHoliday, Vec<Date>>,
    max_end_date: NaiveDate,
) -> Result<BTreeSet<Date>> {
    let operating_profile = vehicle_journey
        .try_only_child("OperatingProfile")
        .or_else(|_| {
            transxchange
                .try_only_child("Services")?
                .try_only_child("Service")?
                .try_only_child("OperatingProfile")
        })?;
    let validity_period = get_service_validity_period(transxchange, max_end_date)?;
    generate_calendar_dates(&operating_profile, bank_holidays, &validity_period)
}

fn find_duplicate_calendar<'a>(
    collections: &'a Collections,
    calendars: &'a CollectionWithId<Calendar>,
    dates: &BTreeSet<NaiveDate>,
) -> Option<&'a Calendar> {
    for c in collections.calendars.values() {
        if c.dates == *dates {
            return Some(c);
        }
    }
    for c in calendars.values() {
        if c.dates == *dates {
            return Some(c);
        }
    }
    None
}

// Get Wait or Run time from ISO 8601 duration
fn parse_duration_in_seconds(duration_iso8601: &str) -> Result<Time> {
    let std_duration = time_parse::duration::parse_nom(duration_iso8601)?;
    let duration_seconds = Duration::from_std(std_duration)?.num_seconds();
    let time = Time::new(0, 0, u32::try_from(duration_seconds)?);
    Ok(time)
}

fn get_duration_from(element: &Element, name: &str) -> Time {
    element
        .try_only_child(name)
        .map(Element::text)
        .and_then(|s| parse_duration_in_seconds(&s))
        .unwrap_or_default()
}

fn get_pickup_and_dropoff_types(element: &Element, name: &str) -> (u8, u8) {
    element
        .try_only_child(name)
        .map(Element::text)
        .map(|a| match a.as_str() {
            "pickUp" => (0, 1),
            "setDown" => (1, 0),
            _ => (0, 0),
        })
        .unwrap_or((0, 0))
}

fn get_stop_point_activity<'a>(
    journey_pattern_timing_link: &'a Element,
    vehicle_journey_timing_links: &'a [&Element],
    stop_point: &'a Element,
    direction: &str,
) -> &'a Element {
    vehicle_journey_timing_links
        .iter()
        .find(|vjtl| {
            vjtl.only_child("JourneyPatternTimingLinkRef")
                .map(Element::text)
                == journey_pattern_timing_link
                    .attr("id")
                    .map(|s| s.to_string())
        })
        .and_then(|vjtl| vjtl.only_child(direction))
        .unwrap_or(stop_point)
}

fn calculate_stop_times(
    stop_points: &CollectionWithId<StopPoint>,
    journey_pattern_section: &Element,
    first_departure_time: Time,
    vehicle_journey_timing_links: &[&Element],
) -> Result<Vec<StopTime>> {
    let mut stop_times = vec![];
    let mut next_arrival_time = first_departure_time;
    let mut stop_point_previous_wait_to = Time::default();
    let mut sequence = 1; // use loop index instead of JourneyPatternTimingLinkId (not always continuous)

    for journey_pattern_timing_link in journey_pattern_section.children() {
        let stop_point = journey_pattern_timing_link.try_only_child("From")?;
        let stop_point_ref = stop_point.try_only_child("StopPointRef")?.text();
        let stop_point_idx = stop_points
            .get_idx(&stop_point_ref)
            .ok_or_else(|| format_err!("stop_id={:?} not found", stop_point_ref))?;
        let stop_point_wait_from = get_duration_from(&stop_point, "WaitTime");
        let run_time = get_duration_from(&journey_pattern_timing_link, "RunTime");
        let stop_point_activity = get_stop_point_activity(
            &journey_pattern_timing_link,
            &vehicle_journey_timing_links,
            &stop_point,
            "From",
        );
        let (pickup_type, drop_off_type) =
            get_pickup_and_dropoff_types(&stop_point_activity, "Activity");
        let arrival_time = next_arrival_time;
        let departure_time = arrival_time + stop_point_wait_from + stop_point_previous_wait_to;

        stop_times.push(StopTime {
            stop_point_idx,
            sequence,
            arrival_time,
            departure_time,
            boarding_duration: 0,
            alighting_duration: 0,
            pickup_type,
            drop_off_type,
            datetime_estimated: false,
            local_zone_id: None,
        });

        next_arrival_time = departure_time + run_time;
        stop_point_previous_wait_to = get_duration_from(
            journey_pattern_timing_link.try_only_child("To")?,
            "WaitTime",
        );
        sequence += 1;
    }
    let last_journey_pattern_timing_link = journey_pattern_section
        .children()
        .last()
        .ok_or_else(|| format_err!("Failed to find the last JourneyPatternSection"))?;
    let last_stop_point = last_journey_pattern_timing_link.try_only_child("To")?;
    let last_stop_point_ref = last_stop_point.try_only_child("StopPointRef")?.text();
    let last_stop_point_idx = stop_points
        .get_idx(&last_stop_point_ref)
        .ok_or_else(|| format_err!("stop_id={} not found", last_stop_point_ref))?;
    let last_stop_point_activity = get_stop_point_activity(
        &last_journey_pattern_timing_link,
        &vehicle_journey_timing_links,
        &last_stop_point,
        "To",
    );
    let (pickup_type, drop_off_type) =
        get_pickup_and_dropoff_types(&last_stop_point_activity, "Activity");

    stop_times.push(StopTime {
        stop_point_idx: last_stop_point_idx,
        sequence,
        arrival_time: next_arrival_time,
        departure_time: next_arrival_time,
        boarding_duration: 0,
        alighting_duration: 0,
        pickup_type,
        drop_off_type,
        datetime_estimated: false,
        local_zone_id: None,
    });
    Ok(stop_times)
}

fn load_routes_vehicle_journeys_calendars(
    collections: &Collections,
    transxchange: &Element,
    bank_holidays: &HashMap<BankHoliday, Vec<Date>>,
    lines: &CollectionWithId<Line>,
    dataset_id: &str,
    physical_mode_id: &str,
    max_end_date: NaiveDate,
) -> Result<(
    CollectionWithId<Route>,
    CollectionWithId<VehicleJourney>,
    CollectionWithId<Calendar>,
)> {
    fn get_journey_pattern<'a>(
        transxchange: &'a Element,
        vehicle_journey: &Element,
    ) -> Result<&'a Element> {
        let journey_pattern_ref = vehicle_journey.try_only_child("JourneyPatternRef")?.text();
        get_by_reference(
            transxchange
                .try_only_child("Services")?
                .try_only_child("Service")?
                .try_only_child("StandardService")?,
            "JourneyPattern",
            &journey_pattern_ref,
        )
    }
    fn get_journey_pattern_section<'a>(
        transxchange: &'a Element,
        journey_pattern: &Element,
    ) -> Result<&'a Element> {
        let journey_pattern_section_ref = journey_pattern
            .try_only_child("JourneyPatternSectionRefs")?
            .text();
        get_by_reference(
            transxchange.try_only_child("JourneyPatternSections")?,
            "JourneyPatternSection",
            &journey_pattern_section_ref,
        )
    }
    let mut routes = CollectionWithId::default();
    let mut vehicle_journeys = CollectionWithId::default();
    let mut calendars = CollectionWithId::default();

    for vehicle_journey in transxchange.try_only_child("VehicleJourneys")?.children() {
        let service_ref = vehicle_journey.try_only_child("ServiceRef")?.text();
        let line_ref = vehicle_journey.try_only_child("LineRef")?.text();
        let vehicle_journey_code = vehicle_journey.try_only_child("VehicleJourneyCode")?.text();
        let id = {
            let mut seq = 1;
            let mut vj_id;
            loop {
                vj_id = format!(
                    "{}:{}:{}:{}",
                    service_ref, line_ref, vehicle_journey_code, seq
                );
                if !collections.vehicle_journeys.contains_id(&vj_id) {
                    break;
                }
                seq += 1;
            }
            vj_id
        };
        let dates =
            create_calendar_dates(transxchange, vehicle_journey, bank_holidays, max_end_date)?;
        if dates.is_empty() {
            warn!("No calendar date, skipping Vehicle Journey {}", id);
            continue;
        }
        let dup_calendar = find_duplicate_calendar(collections, &calendars, &dates);
        let calendar = Calendar {
            id: format!("CD:{}", id),
            dates,
        };
        let service_id = dup_calendar
            .map(|c| c.id.clone())
            .unwrap_or_else(|| calendar.id.clone());
        let journey_pattern = skip_fail!(get_journey_pattern(transxchange, vehicle_journey));
        let journey_pattern_section =
            skip_fail!(get_journey_pattern_section(transxchange, journey_pattern));
        let departure_time: Time = skip_fail!(vehicle_journey
            .try_only_child("DepartureTime")?
            .text()
            .parse());
        let vehicle_journey_timing_links = vehicle_journey
            .children()
            .filter(|child| child.name() == "VehicleJourneyTimingLink")
            .collect::<Vec<_>>();
        let stop_times = skip_fail!(calculate_stop_times(
            &collections.stop_points,
            &journey_pattern_section,
            departure_time,
            &vehicle_journey_timing_links
        )
        .map_err(|e| format_err!("{} / vehiclejourney {} skipped", e, id)));

        let operator_ref = vehicle_journey
            .try_only_child("OperatorRef")
            .or_else(|_| {
                transxchange
                    .try_only_child("Services")?
                    .try_only_child("Service")?
                    .try_only_child("RegisteredOperatorRef")
            })?
            .text();
        let operator = get_by_reference(
            transxchange.try_only_child("Operators")?,
            "Operator",
            &operator_ref,
        )?;
        let company_id = operator.try_only_child("OperatorCode")?.text();
        let route = create_route(
            collections,
            transxchange,
            vehicle_journey,
            lines,
            &stop_times,
        )?;
        let route_id = route.id.clone();
        let headsign = journey_pattern
            .only_child("DestinationDisplay")
            .map(Element::text)
            .map(|head_sign| head_sign.trim().to_string());

        // Insert only at the last moment and if no duplicate calendar exist
        if dup_calendar.is_none() {
            calendars.push(calendar)?;
        }
        // Ignore duplicate insert (it means the route has already been created)
        let _ = routes.push(route);
        vehicle_journeys.push(VehicleJourney {
            id,
            stop_times,
            route_id,
            physical_mode_id: physical_mode_id.to_string(),
            dataset_id: dataset_id.to_string(),
            service_id,
            company_id,
            headsign,
            ..Default::default()
        })?;
    }
    Ok((routes, vehicle_journeys, calendars))
}

fn read_xml(
    transxchange: &Element,
    collections: &mut Collections,
    bank_holidays: &HashMap<BankHoliday, Vec<Date>>,
    dataset_id: &str,
    max_end_date: NaiveDate,
) -> Result<()> {
    let network = load_network(transxchange)?;
    let companies = load_companies(transxchange)?;
    let (commercial_mode, physical_mode) = load_commercial_physical_modes(transxchange)?;
    let lines = load_lines(transxchange, &network.id, &commercial_mode.id)?;
    let (routes, vehicle_journeys, calendars) = load_routes_vehicle_journeys_calendars(
        collections,
        transxchange,
        bank_holidays,
        &lines,
        dataset_id,
        &physical_mode.id,
        max_end_date,
    )?;

    // Insert in collections
    collections.datasets = update_validity_period_from_transxchange(
        &mut collections.datasets,
        transxchange,
        max_end_date,
    )?;
    collections
        .networks
        .merge_with(std::iter::once(network), |network, conflict| {
            if network.name == UNDEFINED {
                network.name = conflict.name.clone();
            }
        });
    collections
        .companies
        .merge_with(companies, |company, conflict| {
            if company.name == UNDEFINED {
                company.name = conflict.name.clone();
            }
        });
    // Ignore if `push` returns an error for duplicates
    let _ = collections.commercial_modes.push(commercial_mode);
    let _ = collections.physical_modes.push(physical_mode);
    collections.lines.merge(lines);
    collections.routes.merge(routes);
    collections.vehicle_journeys.try_merge(vehicle_journeys)?;
    collections.calendars.try_merge(calendars)?;
    Ok(())
}

fn read_file<F>(
    file_path: &Path,
    mut file: F,
    bank_holidays: &HashMap<BankHoliday, Vec<Date>>,
    collections: &mut Collections,
    dataset_id: &str,
    max_end_date: NaiveDate,
) -> Result<()>
where
    F: Read,
{
    match file_path.extension() {
        Some(ext) if ext == "xml" => {
            info!("reading TransXChange file {:?}", file_path);
            let mut file_content = String::new();
            file.read_to_string(&mut file_content)?;
            match file_content.parse::<Element>() {
                Ok(element) => read_xml(
                    &element,
                    collections,
                    bank_holidays,
                    dataset_id,
                    max_end_date,
                )?,
                Err(e) => {
                    warn!("Failed to parse file '{:?}' as DOM: {}", file_path, e);
                }
            };
        }
        _ => info!("skipping file {:?}", file_path),
    };
    Ok(())
}

fn read_from_zip<P>(
    transxchange_path: P,
    bank_holidays: &HashMap<BankHoliday, Vec<Date>>,
    collections: &mut Collections,
    dataset_id: &str,
    max_end_date: NaiveDate,
) -> Result<()>
where
    P: AsRef<Path>,
{
    let zip_file = File::open(transxchange_path)?;
    let mut zip_archive = ZipArchive::new(zip_file)?;
    // The filenames should be sorted before processing as per the specification
    // Path is used as the key so the entries will be ordered by filename
    let entries: BTreeMap<std::path::PathBuf, usize> = (0..zip_archive.len())
        .filter_map(|index| {
            zip_archive
                .by_index(index)
                .map(|file| (file.sanitized_name(), index))
                .ok()
        })
        .collect();
    for index in entries.values() {
        let file = zip_archive.by_index(*index)?;
        read_file(
            file.sanitized_name().as_path(),
            file,
            bank_holidays,
            collections,
            dataset_id,
            max_end_date,
        )?;
    }
    Ok(())
}

fn read_from_path<P>(
    transxchange_path: P,
    bank_holidays: &HashMap<BankHoliday, Vec<Date>>,
    collections: &mut Collections,
    dataset_id: &str,
    max_end_date: NaiveDate,
) -> Result<()>
where
    P: AsRef<Path>,
{
    let mut paths: Vec<_> = WalkDir::new(transxchange_path)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .map(walkdir::DirEntry::into_path)
        .collect();
    // The filenames should be sorted before processing as per the specification
    paths.sort();
    for path in paths {
        let file = File::open(&path)?;
        read_file(
            &path,
            file,
            bank_holidays,
            collections,
            dataset_id,
            max_end_date,
        )?;
    }
    Ok(())
}

/// Read TransXChange format into a Navitia Transit Model
pub fn read<P>(
    transxchange_path: P,
    naptan_path: P,
    bank_holidays_path: Option<P>,
    config_path: Option<P>,
    prefix: Option<String>,
    max_end_date: NaiveDate,
) -> Result<Model>
where
    P: AsRef<Path>,
{
    fn init_dataset_validity_period(dataset: &mut Dataset) {
        dataset.start_date = MAX_DATE;
        dataset.end_date = MIN_DATE;
    }

    let mut collections = Collections::default();
    let (contributor, mut dataset, feed_infos) = crate::read_utils::read_config(config_path)?;
    collections.contributors = CollectionWithId::from(contributor);
    init_dataset_validity_period(&mut dataset);
    let dataset_id = dataset.id.clone();
    collections.datasets = CollectionWithId::from(dataset);
    collections.feed_infos = feed_infos;
    if naptan_path.as_ref().is_file() {
        naptan::read_from_zip(naptan_path, &mut collections)?;
    } else {
        naptan::read_from_path(naptan_path, &mut collections)?;
    };
    let bank_holidays = if let Some(bank_holidays_path) = bank_holidays_path {
        bank_holidays::get_bank_holiday(bank_holidays_path)?
    } else {
        Default::default()
    };
    if transxchange_path.as_ref().is_file() {
        read_from_zip(
            transxchange_path,
            &bank_holidays,
            &mut collections,
            &dataset_id,
            max_end_date,
        )?;
    } else if transxchange_path.as_ref().is_dir() {
        read_from_path(
            transxchange_path,
            &bank_holidays,
            &mut collections,
            &dataset_id,
            max_end_date,
        )?;
    } else {
        bail!("Invalid input data: must be an existing directory or a ZIP archive");
    };

    if let Some(prefix) = prefix {
        collections.add_prefix_with_sep(prefix.as_str(), ":");
    }

    collections.calendar_deduplication();
    collections.sanitize()?;
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
            let max_end_date = Date::from_ymd(2021, 1, 1);
            let ValidityPeriod {
                start_date,
                end_date,
            } = get_service_validity_period(&root, max_end_date).unwrap();
            assert_eq!(Date::from_ymd(2019, 1, 1), start_date);
            assert_eq!(Date::from_ymd(2019, 3, 31), end_date);
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
            let max_end_date = Date::from_ymd(2021, 1, 1);
            let ValidityPeriod {
                start_date,
                end_date,
            } = get_service_validity_period(&root, max_end_date).unwrap();
            assert_eq!(Date::from_ymd(2019, 1, 1), start_date);
            assert_eq!(max_end_date, end_date);
        }

        #[test]
        fn has_far_end() {
            let xml = r#"<root>
                <Services>
                    <Service>
                        <OperatingPeriod>
                            <StartDate>2000-01-01</StartDate>
                            <EndDate>9999-03-31</EndDate>
                        </OperatingPeriod>
                    </Service>
                </Services>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let max_end_date = Date::from_ymd(2021, 1, 1);
            let ValidityPeriod {
                start_date,
                end_date,
            } = get_service_validity_period(&root, max_end_date).unwrap();
            assert_eq!(Date::from_ymd(2000, 1, 1), start_date);
            assert_eq!(max_end_date, end_date);
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
            let max_end_date = Date::from_ymd(2021, 1, 1);
            get_service_validity_period(&root, max_end_date).unwrap();
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
            let max_end_date = Date::from_ymd(2021, 1, 1);
            get_service_validity_period(&root, max_end_date).unwrap();
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
            let max_end_date = Date::from_ymd(2021, 1, 1);
            get_service_validity_period(&root, max_end_date).unwrap();
        }
    }

    mod update_validity_period_from_transxchange {
        use super::*;
        use pretty_assertions::assert_eq;

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
            let max_end_date = Date::from_ymd(2021, 1, 1);
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
            let datasets =
                update_validity_period_from_transxchange(&mut datasets, &root, max_end_date)
                    .unwrap();
            let mut datasets_iter = datasets.values();
            let dataset = datasets_iter.next().unwrap();
            assert_eq!(Date::from_ymd(2019, 1, 1), dataset.start_date);
            assert_eq!(Date::from_ymd(2019, 6, 30), dataset.end_date);
            let dataset = datasets_iter.next().unwrap();
            assert_eq!(Date::from_ymd(2019, 3, 1), dataset.start_date);
            assert_eq!(Date::from_ymd(2019, 4, 30), dataset.end_date);
        }
    }

    mod get_by_reference {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn has_operator() {
            let xml = r#"<root>
                    <Operator id="op1">
                        <OperatorCode>SOME_CODE</OperatorCode>
                        <TradingName>Some name</TradingName>
                    </Operator>
                    <Operator id="op2">
                        <OperatorCode>OTHER_CODE</OperatorCode>
                        <TradingName>Other name</TradingName>
                    </Operator>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let operator = get_by_reference(&root, "Operator", "op1").unwrap();
            let id = operator.try_only_child("OperatorCode").unwrap().text();
            assert_eq!("SOME_CODE", id);
            let name = operator.try_only_child("TradingName").unwrap().text();
            assert_eq!("Some name", name);
        }

        #[test]
        #[should_panic(expected = "Failed to find a child \\'Operator\\' in element \\'root\\'")]
        fn no_operator() {
            let xml = r#"<root>
                <Operator id="op1" />
                <Operator id="op2" />
            </root>"#;
            let root: Element = xml.parse().unwrap();
            get_by_reference(&root, "Operator", "op3").unwrap();
        }
    }

    mod load_network {
        use super::*;
        use pretty_assertions::assert_eq;

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
                        <WebSite>www.example.com</WebSite>
                        <ContactTelephoneNumber>123-456-7890</ContactTelephoneNumber>
                    </Operator>
                    <Operator id="op2">
                        <OperatorCode>OTHER_CODE</OperatorCode>
                        <TradingName>Other name</TradingName>
                    </Operator>
                </Operators>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let network = load_network(&root).unwrap();
            assert_eq!(String::from("Some name"), network.name);
            assert_eq!(Some(String::from("www.example.com")), network.url);
            assert_eq!(Some(String::from("123-456-7890")), network.phone);
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
            assert_eq!(String::from("Some name"), network.name);
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
            let network = load_network(&root).unwrap();
            assert_eq!(UNDEFINED.to_string(), network.name)
        }
    }

    mod load_companies {
        use super::*;
        use pretty_assertions::assert_eq;

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
            assert_eq!(String::from("Some name"), company.name);
            let company = companies.get("OTHER_CODE").unwrap();
            assert_eq!(String::from("Other name"), company.name);
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
        fn no_name() {
            let xml = r#"<root>
                <Operators>
                    <Operator>
                        <OperatorCode>SOME_CODE</OperatorCode>
                    </Operator>
                </Operators>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let companies = load_companies(&root).unwrap();
            let company = companies.get("SOME_CODE").unwrap();
            assert_eq!(UNDEFINED.to_string(), company.name)
        }
    }

    mod load_commercial_physical_modes {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn has_commercial_physical_modes() {
            let xml = r#"<root>
                <Services>
                    <Service>
                        <Mode>bus</Mode>
                    </Service>
                </Services>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let (commercial_mode, physical_mode) = load_commercial_physical_modes(&root).unwrap();

            assert_eq!(String::from("Bus"), commercial_mode.id);
            assert_eq!(String::from("Bus"), commercial_mode.name);

            assert_eq!(String::from("Bus"), physical_mode.id);
            assert_eq!(String::from("Bus"), physical_mode.name);
        }

        #[test]
        fn default_mode() {
            let xml = r#"<root>
                <Services>
                    <Service>
                        <Mode>unicorn</Mode>
                    </Service>
                </Services>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let (commercial_mode, physical_mode) = load_commercial_physical_modes(&root).unwrap();

            assert_eq!(String::from("Bus"), commercial_mode.id);
            assert_eq!(String::from("Bus"), commercial_mode.name);

            assert_eq!(String::from("Bus"), physical_mode.id);
            assert_eq!(String::from("Bus"), physical_mode.name);
        }
    }

    mod load_lines {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn has_line() {
            let xml = r#"<root>
                <Operators>
                    <Operator id="O1">
                        <OperatorCode>SSWL</OperatorCode>
                    </Operator>
                </Operators>
                <Services>
                    <Service>
                        <ServiceCode>SCBO001</ServiceCode>
                        <Lines>
                            <Line id="SL1">
                                <LineName>1</LineName>
                            </Line>
                            <Line id="SL2">
                                <LineName>2</LineName>
                            </Line>
                        </Lines>
                        <Description>Cwmbran - Cwmbran via Thornhill</Description>
                        <StandardService>
                            <Origin>Cwmbran South</Origin>
                            <Destination>Cwmbran North</Destination>
                        </StandardService>
                        <RegisteredOperatorRef>O1</RegisteredOperatorRef>
                        <Mode>bus</Mode>
                    </Service>
                </Services>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let lines = load_lines(&root, "SSWL", "Bus").unwrap();
            let line = lines.get("SCBO001:SL1").unwrap();
            assert_eq!(Some(String::from("1")), line.code);
            assert_eq!(String::from("Cwmbran - Cwmbran via Thornhill"), line.name);
            assert_eq!(Some(String::from("Cwmbran North")), line.forward_name);
            // TODO: Fill up the forward direction
            assert_eq!(None, line.forward_direction);
            assert_eq!(Some(String::from("Cwmbran South")), line.backward_name);
            // TODO: Fill up the backward direction
            assert_eq!(None, line.backward_direction);
            assert_eq!(String::from("SSWL"), line.network_id);
            assert_eq!(String::from("Bus"), line.commercial_mode_id);

            let line = lines.get("SCBO001:SL2").unwrap();
            assert_eq!(Some(String::from("2")), line.code);
            assert_eq!(String::from("Cwmbran - Cwmbran via Thornhill"), line.name);
            assert_eq!(Some(String::from("Cwmbran North")), line.forward_name);
            // TODO: Fill up the forward direction
            assert_eq!(None, line.forward_direction);
            assert_eq!(Some(String::from("Cwmbran South")), line.backward_name);
            // TODO: Fill up the backward direction
            assert_eq!(None, line.backward_direction);
            assert_eq!(String::from("SSWL"), line.network_id);
            assert_eq!(String::from("Bus"), line.commercial_mode_id);
        }

        #[test]
        fn has_line_without_name() {
            let xml = r#"<root>
                <Operators>
                    <Operator id="O1">
                        <OperatorCode>SSWL</OperatorCode>
                    </Operator>
                </Operators>
                <Services>
                    <Service>
                        <ServiceCode>SCBO001</ServiceCode>
                        <Lines>
                            <Line id="SL1">
                                <LineName>1</LineName>
                            </Line>
                        </Lines>
                        <StandardService>
                            <Origin>Cwmbran South</Origin>
                            <Destination>Cwmbran North</Destination>
                        </StandardService>
                        <RegisteredOperatorRef>O1</RegisteredOperatorRef>
                        <Mode>bus</Mode>
                    </Service>
                </Services>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let lines = load_lines(&root, "SSWL", "Bus").unwrap();
            let line = lines.get("SCBO001:SL1").unwrap();
            assert_eq!(Some(String::from("1")), line.code);
            assert_eq!(String::from(UNDEFINED), line.name);
            assert_eq!(Some(String::from("Cwmbran North")), line.forward_name);
            // TODO: Fill up the forward direction
            assert_eq!(None, line.forward_direction);
            assert_eq!(Some(String::from("Cwmbran South")), line.backward_name);
            // TODO: Fill up the backward direction
            assert_eq!(None, line.backward_direction);
            assert_eq!(String::from("SSWL"), line.network_id);
            assert_eq!(String::from("Bus"), line.commercial_mode_id);
        }
    }

    mod parse_duration_in_seconds {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn parse_duration() {
            let time = parse_duration_in_seconds("PT1H30M5S").unwrap();
            assert_eq!(Time::new(1, 30, 5), time);
        }

        #[test]
        #[should_panic]
        fn invalid_duration() {
            parse_duration_in_seconds("NotAValidISO8601Duration").unwrap();
        }
    }

    mod get_duration_from {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn get_duration() {
            let xml = r#"<root>
                <duration>PT30S</duration>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let time = get_duration_from(&root, "duration");
            assert_eq!(Time::new(0, 0, 30), time);
        }

        #[test]
        fn no_child() {
            let xml = r#"<root />"#;
            let root: Element = xml.parse().unwrap();
            let time = get_duration_from(&root, "duration");
            assert_eq!(Time::new(0, 0, 0), time);
        }
    }

    mod get_pickup_and_dropoff_types {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn get_pickup() {
            let xml = r#"<root>
                <activity>pickUp</activity>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let (pickup_type, drop_off_type) = get_pickup_and_dropoff_types(&root, "activity");
            assert_eq!(0, pickup_type);
            assert_eq!(1, drop_off_type);
        }

        #[test]
        fn get_setdown() {
            let xml = r#"<root>
                <activity>setDown</activity>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let (pickup_type, drop_off_type) = get_pickup_and_dropoff_types(&root, "activity");
            assert_eq!(1, pickup_type);
            assert_eq!(0, drop_off_type);
        }

        #[test]
        fn get_pickupandsetdown() {
            let xml = r#"<root>
                <activity>pickUpAndSetDown</activity>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let (pickup_type, drop_off_type) = get_pickup_and_dropoff_types(&root, "activity");
            assert_eq!(0, pickup_type);
            assert_eq!(0, drop_off_type);
        }

        #[test]
        fn no_child() {
            let xml = r#"<root />"#;
            let root: Element = xml.parse().unwrap();
            let (pickup_type, drop_off_type) = get_pickup_and_dropoff_types(&root, "activity");
            assert_eq!(0, pickup_type);
            assert_eq!(0, drop_off_type);
        }
    }

    mod calculate_stop_times {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn generate_stop_times() {
            let stop_points = CollectionWithId::new(vec![
                StopPoint {
                    id: String::from("sp:1"),
                    ..Default::default()
                },
                StopPoint {
                    id: String::from("sp:2"),
                    ..Default::default()
                },
                StopPoint {
                    id: String::from("sp:3"),
                    ..Default::default()
                },
            ])
            .unwrap();
            let xml = r#"<root>
                <child>
                    <From>
                        <StopPointRef>sp:1</StopPointRef>
                        <WaitTime>PT60S</WaitTime>
                    </From>
                    <To>
                        <StopPointRef>sp:2</StopPointRef>
                    </To>
                    <RunTime>PT10M</RunTime>
                </child>
                <child>
                    <From>
                        <StopPointRef>sp:2</StopPointRef>
                        <WaitTime>PT1M30S</WaitTime>
                    </From>
                    <To>
                        <StopPointRef>sp:3</StopPointRef>
                        <WaitTime>PT2M</WaitTime>
                    </To>
                    <RunTime>PT5M</RunTime>
                </child>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let stop_times =
                calculate_stop_times(&stop_points, &root, Time::new(0, 0, 0), &Vec::new()).unwrap();
            let stop_time = &stop_times[0];
            assert_eq!(
                stop_points.get_idx("sp:1").unwrap(),
                stop_time.stop_point_idx
            );
            assert_eq!(1, stop_time.sequence);
            assert_eq!(Time::new(0, 0, 0), stop_time.arrival_time);
            assert_eq!(Time::new(0, 1, 0), stop_time.departure_time);

            let stop_time = &stop_times[1];
            assert_eq!(
                stop_points.get_idx("sp:2").unwrap(),
                stop_time.stop_point_idx
            );
            assert_eq!(2, stop_time.sequence);
            assert_eq!(Time::new(0, 11, 0), stop_time.arrival_time);
            assert_eq!(Time::new(0, 12, 30), stop_time.departure_time);

            let stop_time = &stop_times[2];
            assert_eq!(
                stop_points.get_idx("sp:3").unwrap(),
                stop_time.stop_point_idx
            );
            assert_eq!(3, stop_time.sequence);
            assert_eq!(Time::new(0, 17, 30), stop_time.arrival_time);
            assert_eq!(Time::new(0, 17, 30), stop_time.departure_time);
        }

        #[test]
        #[should_panic(expected = "stop_id=\\\"sp:1\\\" not found")]
        fn stop_point_not_found() {
            let stop_points = CollectionWithId::new(vec![]).unwrap();
            let xml = r#"<root>
                <child>
                    <From>
                        <StopPointRef>sp:1</StopPointRef>
                        <WaitTime>PT60S</WaitTime>
                    </From>
                    <To>
                        <StopPointRef>sp:2</StopPointRef>
                    </To>
                    <RunTime>PT10M</RunTime>
                </child>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            calculate_stop_times(&stop_points, &root, Time::new(0, 0, 0), &Vec::new()).unwrap();
        }

        #[test]
        #[should_panic(expected = "Failed to find the last JourneyPatternSection")]
        fn no_section() {
            let stop_points = CollectionWithId::new(vec![]).unwrap();
            let xml = r#"<root />"#;
            let root: Element = xml.parse().unwrap();
            calculate_stop_times(&stop_points, &root, Time::new(0, 0, 0), &Vec::new()).unwrap();
        }
    }

    mod generate_calendar_dates {
        use super::*;

        #[test]
        fn all_work_week() {
            let xml = r#"<root>
                <RegularDayType>
                    <DaysOfWeek>
                        <MondayToFriday />
                    </DaysOfWeek>
                </RegularDayType>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let bank_holidays = HashMap::new();
            let start_date = NaiveDate::from_ymd(2019, 1, 1);
            let end_date = NaiveDate::from_ymd(2019, 1, 8);
            let validity = ValidityPeriod {
                start_date,
                end_date,
            };
            let dates = generate_calendar_dates(&root, &bank_holidays, &validity).unwrap();
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 1)));
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 2)));
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 3)));
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 4)));
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 7)));
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 8)));
        }

        #[test]
        fn with_included_bank_holidays() {
            let xml = r#"<root>
                <RegularDayType>
                    <DaysOfWeek>
                        <MondayToFriday />
                    </DaysOfWeek>
                </RegularDayType>
                <BankHolidayOperation>
                    <DaysOfOperation>
                        <NewYearsDay />
                    </DaysOfOperation>
                </BankHolidayOperation>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let bank_holidays = HashMap::new();
            // 1st of January 2017 was a Sunday
            let start_date = NaiveDate::from_ymd(2017, 1, 1);
            let end_date = NaiveDate::from_ymd(2017, 1, 8);
            let validity = ValidityPeriod {
                start_date,
                end_date,
            };
            let dates = generate_calendar_dates(&root, &bank_holidays, &validity).unwrap();
            assert!(dates.contains(&NaiveDate::from_ymd(2017, 1, 1)));
            assert!(dates.contains(&NaiveDate::from_ymd(2017, 1, 2)));
            assert!(dates.contains(&NaiveDate::from_ymd(2017, 1, 3)));
            assert!(dates.contains(&NaiveDate::from_ymd(2017, 1, 4)));
            assert!(dates.contains(&NaiveDate::from_ymd(2017, 1, 5)));
            assert!(dates.contains(&NaiveDate::from_ymd(2017, 1, 6)));
        }

        #[test]
        fn with_excluded_bank_holidays() {
            let xml = r#"<root>
                <RegularDayType>
                    <DaysOfWeek>
                        <Weekend />
                    </DaysOfWeek>
                </RegularDayType>
                <BankHolidayOperation>
                    <DaysOfNonOperation>
                        <NewYearsDay />
                    </DaysOfNonOperation>
                </BankHolidayOperation>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let bank_holidays = HashMap::new();
            // 1st of January 2017 was a Sunday
            let start_date = NaiveDate::from_ymd(2017, 1, 1);
            let end_date = NaiveDate::from_ymd(2017, 1, 8);
            let validity = ValidityPeriod {
                start_date,
                end_date,
            };
            let dates = generate_calendar_dates(&root, &bank_holidays, &validity).unwrap();
            assert!(dates.contains(&NaiveDate::from_ymd(2017, 1, 7)));
            assert!(dates.contains(&NaiveDate::from_ymd(2017, 1, 8)));
        }

        #[test]
        fn not_saturday() {
            let xml = r#"<root>
                <RegularDayType>
                    <DaysOfWeek>
                        <NotSaturday />
                    </DaysOfWeek>
                </RegularDayType>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let bank_holidays = HashMap::new();
            let start_date = NaiveDate::from_ymd(2019, 1, 4);
            let end_date = NaiveDate::from_ymd(2019, 1, 6);
            let validity = ValidityPeriod {
                start_date,
                end_date,
            };
            let dates = generate_calendar_dates(&root, &bank_holidays, &validity).unwrap();
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 4)));
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 6)));
        }

        #[test]
        fn inverted_start_end() {
            let xml = r#"<root>
                <RegularDayType>
                    <DaysOfWeek>
                        <Weekend />
                        <Monday />
                    </DaysOfWeek>
                </RegularDayType>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let bank_holidays = HashMap::new();
            let start_date = NaiveDate::from_ymd(2019, 1, 1);
            let end_date = NaiveDate::from_ymd(2019, 1, 8);
            let validity = ValidityPeriod {
                start_date,
                end_date,
            };
            let dates = generate_calendar_dates(&root, &bank_holidays, &validity).unwrap();
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 5)));
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 6)));
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 7)));
        }

        #[test]
        fn unknown_tag() {
            let xml = r#"<root>
                <RegularDayType>
                    <DaysOfWeek>
                        <MondayToSaturday />
                        <MondayToSunday />
                        <UnknownTag />
                    </DaysOfWeek>
                </RegularDayType>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let bank_holidays = HashMap::new();
            let start_date = NaiveDate::from_ymd(2019, 1, 1);
            let end_date = NaiveDate::from_ymd(2019, 1, 8);
            let validity = ValidityPeriod {
                start_date,
                end_date,
            };
            let dates = generate_calendar_dates(&root, &bank_holidays, &validity).unwrap();
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 1)));
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 2)));
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 3)));
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 4)));
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 5)));
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 6)));
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 7)));
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 8)));
        }

        #[test]
        fn no_tag() {
            let xml = r#"<root>
                <RegularDayType>
                    <DaysOfWeek />
                </RegularDayType>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let bank_holidays = HashMap::new();
            let start_date = NaiveDate::from_ymd(2019, 1, 1);
            let end_date = NaiveDate::from_ymd(2019, 1, 8);
            let validity = ValidityPeriod {
                start_date,
                end_date,
            };
            let dates = generate_calendar_dates(&root, &bank_holidays, &validity).unwrap();
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 1)));
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 2)));
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 3)));
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 4)));
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 5)));
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 6)));
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 7)));
            assert!(dates.contains(&NaiveDate::from_ymd(2019, 1, 8)));
        }

        #[test]
        fn holidays_only() {
            let xml = r#"<root>
                <RegularDayType>
                    <HolidaysOnly />
                </RegularDayType>
            </root>"#;
            let root: Element = xml.parse().unwrap();
            let bank_holidays = HashMap::new();
            let start_date = NaiveDate::from_ymd(2019, 1, 1);
            let end_date = NaiveDate::from_ymd(2019, 1, 8);
            let validity = ValidityPeriod {
                start_date,
                end_date,
            };
            let dates = generate_calendar_dates(&root, &bank_holidays, &validity).unwrap();
            assert!(dates.is_empty());
        }
    }

    mod find_duplicate_calendar {
        use super::*;
        use pretty_assertions::assert_eq;

        fn generate_date_set(year: i32, month: u32, day: u32) -> BTreeSet<NaiveDate> {
            let date = NaiveDate::from_ymd(year, month, day);
            let mut dates = BTreeSet::new();
            dates.insert(date);
            dates
        }

        fn init() -> (Collections, CollectionWithId<Calendar>) {
            let mut collections = Collections::default();
            collections
                .calendars
                .push(Calendar {
                    id: String::from("calendar:1"),
                    dates: generate_date_set(2018, 1, 1),
                })
                .unwrap();

            let mut calendars = CollectionWithId::default();
            calendars
                .push(Calendar {
                    id: String::from("calendar:2"),
                    dates: generate_date_set(2019, 6, 15),
                })
                .unwrap();
            (collections, calendars)
        }

        #[test]
        fn no_duplicate() {
            let (collections, calendars) = init();
            let dates = generate_date_set(2020, 12, 31);

            let duplicate = find_duplicate_calendar(&collections, &calendars, &dates);
            assert_eq!(None, duplicate);
        }

        #[test]
        fn duplicate_in_collections() {
            let (collections, calendars) = init();
            let dates = generate_date_set(2018, 1, 1);

            let calendar = find_duplicate_calendar(&collections, &calendars, &dates).unwrap();
            assert_eq!("calendar:1", &calendar.id);
        }

        #[test]
        fn duplicate_in_calendars() {
            let (collections, calendars) = init();
            let dates = generate_date_set(2019, 6, 15);

            let calendar = find_duplicate_calendar(&collections, &calendars, &dates).unwrap();
            assert_eq!("calendar:2", &calendar.id);
        }
    }
}
