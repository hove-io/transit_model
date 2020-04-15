// Copyright 2017-2019 Kisio Digital and/or its affiliates.
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
use super::offers::{self, GeneralFrameType, CALENDARS_FILENAME, NETEX_CALENDAR};
use crate::{
    netex_utils::{self, FrameType},
    objects::{Date, ValidityPeriod},
    Result,
};
use chrono::{Datelike, NaiveDateTime, Weekday};
use failure::format_err;
use log::{warn, Level as LogLevel};
use minidom::Element;
use minidom_ext::{AttributeElementExt, OnlyChildElementExt};
use skip_error::skip_error_and_log;
use std::{
    cmp::{Ord, Ordering, PartialOrd},
    collections::{BTreeSet, HashMap, HashSet},
};

type OperatingPeriods = HashMap<String, ValidityPeriod>;
type DayTypeAssignments<'a> = HashMap<String, BTreeSet<DayTypeAssignment<'a>>>;
pub type DayTypes = HashMap<String, BTreeSet<Date>>;

#[derive(Debug, Eq, PartialEq)]
enum DayTypeAssignment<'a> {
    OperatingPeriod(&'a ValidityPeriod),
    ActiveDay(Date),
    InactiveDay(Date),
}

// operating periods are treated first
// then active days
// then inactive days
// to ensure that inactive days are removed if they appears before
// operating periods in the XML feed.
impl PartialOrd for DayTypeAssignment<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        use DayTypeAssignment::*;
        match (self, other) {
            (OperatingPeriod(op1), OperatingPeriod(op2)) => {
                op1.start_date.partial_cmp(&op2.start_date)
            }
            (OperatingPeriod(_), _) => Some(Ordering::Less),
            (_, OperatingPeriod(_)) => Some(Ordering::Greater),
            (ActiveDay(active1), ActiveDay(active2)) => active1.partial_cmp(&active2),
            (ActiveDay(_), _) => Some(Ordering::Less),
            (_, ActiveDay(_)) => Some(Ordering::Greater),
            (InactiveDay(inactive1), InactiveDay(inactive2)) => inactive1.partial_cmp(&inactive2),
        }
    }
}
impl Ord for DayTypeAssignment<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        // Unwrap is possible since `PartialOrd` implementation never returns None
        self.partial_cmp(other).unwrap()
    }
}

struct ValidityPatternIterator<'a> {
    current_date: Date,
    end_date: Date,
    weekday_pattern: &'a HashSet<Weekday>,
    day_type_assignments: Option<&'a BTreeSet<DayTypeAssignment<'a>>>,
}
impl<'a> ValidityPatternIterator<'a> {
    fn new(
        validity_period: &ValidityPeriod,
        weekday_pattern: &'a HashSet<Weekday>,
        day_type_assignments: Option<&'a BTreeSet<DayTypeAssignment<'a>>>,
    ) -> ValidityPatternIterator<'a> {
        ValidityPatternIterator {
            current_date: validity_period.start_date.pred(),
            end_date: validity_period.end_date,
            weekday_pattern,
            day_type_assignments,
        }
    }
}

impl<'a> Iterator for ValidityPatternIterator<'a> {
    type Item = Date;
    fn next(&mut self) -> Option<Self::Item> {
        self.current_date = self.current_date.succ();
        if self.current_date > self.end_date {
            return None;
        }
        let fold_with = |is_included, dta: &DayTypeAssignment| match *dta {
            DayTypeAssignment::OperatingPeriod(vp) => {
                (vp.start_date <= self.current_date
                    && vp.end_date >= self.current_date
                    && self.weekday_pattern.contains(&self.current_date.weekday()))
                    || is_included
            }
            DayTypeAssignment::ActiveDay(date) => date == self.current_date || is_included,
            DayTypeAssignment::InactiveDay(date) => is_included && date != self.current_date,
        };
        let is_included = self
            .day_type_assignments
            .iter()
            .flat_map(|day_type_assignments| day_type_assignments.iter())
            .fold(false, fold_with);
        if is_included {
            return Some(self.current_date);
        }
        self.next()
    }
}

fn parse_validity_period(valid_between: &Element) -> Result<ValidityPeriod> {
    fn parse_date(valid_between: &Element, node: &str) -> Result<Date> {
        let date = valid_between
            .try_only_child(node)
            .map_err(|e| format_err!("{}", e))?
            .text()
            .parse::<NaiveDateTime>()?
            .date();
        Ok(date)
    }
    let start_date = parse_date(valid_between, "FromDate")?;
    let end_date = parse_date(valid_between, "ToDate")?;
    Ok(ValidityPeriod {
        start_date,
        end_date,
    })
}

fn parse_operating_periods<'a, I>(operating_period_elements: I) -> OperatingPeriods
where
    I: Iterator<Item = &'a Element>,
{
    operating_period_elements
        .filter_map(|operating_period_element| {
            let id = operating_period_element.attribute("id")?;
            let validity_period = parse_validity_period(operating_period_element).ok()?;
            Some((id, validity_period))
        })
        .collect()
}

fn parse_day_type_assignments<'a, I>(
    day_type_assignment_elements: I,
    operating_periods: &OperatingPeriods,
) -> DayTypeAssignments
where
    I: Iterator<Item = &'a Element>,
{
    let mut day_type_assignments = DayTypeAssignments::default();
    for dta_element in day_type_assignment_elements {
        let day_type_ref: String = skip_error_and_log!(
            dta_element
                .try_only_child("DayTypeRef")
                .and_then(|dtr_element| dtr_element.try_attribute("ref")),
            LogLevel::Warn
        );
        if let Some(operating_period_ref_element) = dta_element.only_child("OperatingPeriodRef") {
            let operating_period = skip_error_and_log!(
                operating_period_ref_element
                    .attribute::<String>("ref")
                    .and_then(|op_ref| operating_periods.get(&op_ref))
                    .ok_or_else(|| {
                        format_err!(
                            "OperatingPeriod referenced by DayTypeAssignment '{}' can't be found",
                            dta_element.attribute("id").unwrap_or_else(String::new),
                        )
                    }),
                LogLevel::Warn
            );
            day_type_assignments
                .entry(day_type_ref)
                .or_insert_with(BTreeSet::new)
                .insert(DayTypeAssignment::OperatingPeriod(operating_period));
        } else if let Some(date_element) = dta_element.only_child("Date") {
            let date = skip_error_and_log!(date_element.text().parse::<Date>(), LogLevel::Warn);
            let status = dta_element
                .only_child("isAvailable")
                .and_then(|el| el.text().parse::<bool>().ok())
                .unwrap_or(true);
            let day_type_assignment = if status {
                DayTypeAssignment::ActiveDay(date)
            } else {
                DayTypeAssignment::InactiveDay(date)
            };
            day_type_assignments
                .entry(day_type_ref)
                .or_insert_with(BTreeSet::new)
                .insert(day_type_assignment);
        } else {
            warn!("DayTypeAssignment '{}' is ignored because it does not have 'OperatingPeriodRef' nor 'Date'+'isAvailable'",
                dta_element
                    .attribute("id")
                    .unwrap_or_else(String::new));
        }
    }
    day_type_assignments
}

fn parse_day_types<'a, I>(
    day_type_elements: I,
    validity_period: &ValidityPeriod,
    day_type_assignments: &DayTypeAssignments,
) -> DayTypes
where
    I: Iterator<Item = &'a Element>,
{
    fn to_weekday(day_of_week: String) -> Option<Weekday> {
        use chrono::Weekday::*;
        match day_of_week.as_str() {
            "Monday" => Some(Mon),
            "Tuesday" => Some(Tue),
            "Wednesday" => Some(Wed),
            "Thursday" => Some(Thu),
            "Friday" => Some(Fri),
            "Saturday" => Some(Sat),
            "Sunday" => Some(Sun),
            value => {
                warn!("'{}' is an invalid value for 'DaysOfWeek'", value);
                None
            }
        }
    }
    let mut day_types = DayTypes::default();
    for dt_element in day_type_elements {
        let id: String = skip_error_and_log!(dt_element.try_attribute("id"), LogLevel::Warn);
        let weekdays: HashSet<_> = dt_element
            .only_child("properties")
            .map(|properties| {
                properties
                    .children()
                    .filter_map(|property_of_day| property_of_day.only_child("DaysOfWeek"))
                    .map(Element::text)
                    .filter_map(to_weekday)
                    .collect()
            })
            .unwrap_or_else(HashSet::new);
        let days = ValidityPatternIterator::new(
            &validity_period,
            &weekdays,
            day_type_assignments.get(&id),
        )
        .collect();
        day_types.insert(id, days);
    }
    day_types
}

pub fn parse_calendars(calendars: &Element) -> Result<(DayTypes, ValidityPeriod)> {
    let frames = netex_utils::parse_frames_by_type(calendars.try_only_child("dataObjects")?)?;
    let general_frames = frames
        .get(&FrameType::General)
        .ok_or_else(|| format_err!("Failed to find a GeneralFrame in {}", CALENDARS_FILENAME))?;
    let general_frames_by_type = offers::parse_general_frame_by_type(general_frames)?;
    let calendar_general_frame = general_frames_by_type
        .get(&GeneralFrameType::Calendar)
        .ok_or_else(|| format_err!("Failed to find the GeneralFrame of type {}", NETEX_CALENDAR))?;
    let validity_period =
        parse_validity_period(calendar_general_frame.try_only_child("ValidBetween")?)?;
    let members = calendar_general_frame.try_only_child("members")?;
    let operating_period_elements = members
        .children()
        .filter(|child| child.name() == "OperatingPeriod");
    let operating_periods = parse_operating_periods(operating_period_elements);
    let day_type_assignment_elements = members
        .children()
        .filter(|child| child.name() == "DayTypeAssignment");
    let day_type_assignments =
        parse_day_type_assignments(day_type_assignment_elements, &operating_periods);
    let day_type_elements = members.children().filter(|child| child.name() == "DayType");
    let day_types = parse_day_types(day_type_elements, &validity_period, &day_type_assignments);
    Ok((day_types, validity_period))
}

#[cfg(test)]
mod tests {
    use super::*;

    mod parse_validity_period {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn valid_validity_period() {
            let xml = r#"<ValidBetween>
                    <FromDate>2019-07-09T00:00:00</FromDate>
                    <ToDate>2019-08-07T00:00:00</ToDate>
                </ValidBetween>"#;
            let root: Element = xml.parse().unwrap();
            let validity_period = parse_validity_period(&root).unwrap();
            assert_eq!(Date::from_ymd(2019, 7, 9), validity_period.start_date);
            assert_eq!(Date::from_ymd(2019, 8, 7), validity_period.end_date);
        }

        #[test]
        #[should_panic(
            expected = "No children with name \\'ToDate\\' in Element \\'ValidBetween\\'"
        )]
        fn missing_date() {
            let xml = r#"<ValidBetween>
                    <FromDate>2019-07-09T00:00:00</FromDate>
                </ValidBetween>"#;
            let root: Element = xml.parse().unwrap();
            parse_validity_period(&root).unwrap();
        }
    }
    mod parse_operating_periods {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn operating_periods() {
            let xml = r#"<root>
                    <OperatingPeriod id="operating_period_1">
                        <FromDate>2019-07-09T00:00:00</FromDate>
                        <ToDate>2019-10-09T00:00:00</ToDate>
                    </OperatingPeriod>
                    <!-- Missing 'ToDate' -->
                    <OperatingPeriod id="operating_period_2">
                        <FromDate>2019-07-09T00:00:00</FromDate>
                    </OperatingPeriod>
                    <!-- Missing 'id' attribute -->
                    <OperatingPeriod>
                        <FromDate>2019-07-09T00:00:00</FromDate>
                        <ToDate>2019-10-09T00:00:00</ToDate>
                    </OperatingPeriod>
                    <!-- Invalid date -->
                    <OperatingPeriod id="operating_period_3">
                        <FromDate>NotADate</FromDate>
                        <ToDate>2019-10-09T00:00:00</ToDate>
                    </OperatingPeriod>
                </root>"#;
            let root: Element = xml.parse().unwrap();
            let operating_periods = parse_operating_periods(root.children());
            assert_eq!(1, operating_periods.len());
            let operating_period = operating_periods.get("operating_period_1").unwrap();
            assert_eq!(Date::from_ymd(2019, 7, 9), operating_period.start_date);
            assert_eq!(Date::from_ymd(2019, 10, 9), operating_period.end_date);
        }
    }

    mod parse_day_type_assignments {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn day_type_assignments() {
            let xml = r#"<root>
                    <DayTypeAssignment id="dta_1">
                        <OperatingPeriodRef ref="operating_period_1" />
                        <DayTypeRef ref="day_type_1" />
                    </DayTypeAssignment>
                    <DayTypeAssignment id="dta_2">
                        <Date>2019-08-15</Date>
                        <DayTypeRef ref="day_type_2" />
                        <isAvailable>false</isAvailable>
                    </DayTypeAssignment>
                    <DayTypeAssignment id="dta_3">
                        <Date>2019-08-16</Date>
                        <DayTypeRef ref="day_type_2" />
                        <isAvailable>true</isAvailable>
                    </DayTypeAssignment>
                    <!-- Missing 'id' -->
                    <DayTypeAssignment>
                        <OperatingPeriodRef ref="operating_period_1" />
                        <DayTypeRef ref="day_type_3" />
                    </DayTypeAssignment>
                    <!-- Missing 'OperatingPeriodRef' or 'Date'+'isAvailable' -->
                    <DayTypeAssignment id="dta_4">
                        <DayTypeRef ref="day_type_4" />
                    </DayTypeAssignment>
                    <!-- Missing 'isAvailable' -->
                    <DayTypeAssignment id="dta_5">
                        <Date>2019-08-15</Date>
                        <DayTypeRef ref="day_type_5" />
                    </DayTypeAssignment>
                    <!-- Invalid date -->
                    <DayTypeAssignment id="dta_6">
                        <Date>NotADate</Date>
                        <DayTypeRef ref="day_type_6" />
                        <isAvailable>true</isAvailable>
                    </DayTypeAssignment>
                    <!-- Unknown operating period -->
                    <DayTypeAssignment id="dta_7">
                        <OperatingPeriodRef ref="unknown_operating_period_ref" />
                        <DayTypeRef ref="day_type_8" />
                    </DayTypeAssignment>
                </root>"#;
            let root: Element = xml.parse().unwrap();
            let mut operating_periods = OperatingPeriods::default();
            let start_date = Date::from_ymd(2019, 1, 1);
            let end_date = Date::from_ymd(2019, 2, 1);
            operating_periods.insert(
                String::from("operating_period_1"),
                ValidityPeriod {
                    start_date,
                    end_date,
                },
            );
            let day_type_assignments =
                parse_day_type_assignments(root.children(), &operating_periods);

            let day_type_assignment = day_type_assignments.get("day_type_1").unwrap();
            assert_eq!(1, day_type_assignment.len());
            assert!(
                day_type_assignment.contains(&DayTypeAssignment::OperatingPeriod(
                    &ValidityPeriod {
                        start_date,
                        end_date
                    }
                ))
            );

            let day_type_assignment = day_type_assignments.get("day_type_2").unwrap();
            assert_eq!(2, day_type_assignment.len());
            assert!(day_type_assignment
                .contains(&DayTypeAssignment::InactiveDay(Date::from_ymd(2019, 8, 15))));
            assert!(day_type_assignment
                .contains(&DayTypeAssignment::ActiveDay(Date::from_ymd(2019, 8, 16))));
        }
    }

    mod validity_pattern_iterator {
        use super::{DayTypeAssignment::*, *};
        use pretty_assertions::assert_eq;

        #[test]
        fn days_in_operating_period() {
            let validity_period = ValidityPeriod {
                start_date: Date::from_ymd(2019, 6, 1),
                end_date: Date::from_ymd(2019, 7, 31),
            };
            let weekday_pattern = vec![Weekday::Sat, Weekday::Sun].into_iter().collect();
            let operating_period = ValidityPeriod {
                start_date: Date::from_ymd(2019, 7, 1),
                end_date: Date::from_ymd(2019, 7, 7),
            };
            let day_type_assignments = vec![OperatingPeriod(&operating_period)]
                .into_iter()
                .collect();
            let mut validity_pattern_iterator = ValidityPatternIterator::new(
                &validity_period,
                &weekday_pattern,
                Some(&day_type_assignments),
            );
            let date = validity_pattern_iterator.next().unwrap();
            assert_eq!(Date::from_ymd(2019, 7, 6), date);
            let date = validity_pattern_iterator.next().unwrap();
            assert_eq!(Date::from_ymd(2019, 7, 7), date);
            assert!(validity_pattern_iterator.next().is_none());
        }

        #[test]
        fn operating_period_removing_one_day() {
            let validity_period = ValidityPeriod {
                start_date: Date::from_ymd(2019, 6, 1),
                end_date: Date::from_ymd(2019, 7, 31),
            };
            let weekday_pattern = vec![Weekday::Sat, Weekday::Sun].into_iter().collect();
            let operating_period = ValidityPeriod {
                start_date: Date::from_ymd(2019, 7, 1),
                end_date: Date::from_ymd(2019, 7, 7),
            };
            let inactive_day = Date::from_ymd(2019, 7, 6);
            let day_type_assignments = vec![
                OperatingPeriod(&operating_period),
                InactiveDay(inactive_day),
            ]
            .into_iter()
            .collect();
            let mut validity_pattern_iterator = ValidityPatternIterator::new(
                &validity_period,
                &weekday_pattern,
                Some(&day_type_assignments),
            );
            let date = validity_pattern_iterator.next().unwrap();
            assert_eq!(Date::from_ymd(2019, 7, 7), date);
            assert!(validity_pattern_iterator.next().is_none());
        }

        #[test]
        fn operating_period_with_additional_day() {
            let validity_period = ValidityPeriod {
                start_date: Date::from_ymd(2019, 6, 1),
                end_date: Date::from_ymd(2019, 7, 31),
            };
            let weekday_pattern = vec![Weekday::Sat, Weekday::Sun].into_iter().collect();
            let operating_period = ValidityPeriod {
                start_date: Date::from_ymd(2019, 7, 1),
                end_date: Date::from_ymd(2019, 7, 7),
            };
            let active_day = Date::from_ymd(2019, 6, 3);
            let day_type_assignments =
                vec![OperatingPeriod(&operating_period), ActiveDay(active_day)]
                    .into_iter()
                    .collect();
            let mut validity_pattern_iterator = ValidityPatternIterator::new(
                &validity_period,
                &weekday_pattern,
                Some(&day_type_assignments),
            );
            let date = validity_pattern_iterator.next().unwrap();
            assert_eq!(Date::from_ymd(2019, 6, 3), date);
            let date = validity_pattern_iterator.next().unwrap();
            assert_eq!(Date::from_ymd(2019, 7, 6), date);
            let date = validity_pattern_iterator.next().unwrap();
            assert_eq!(Date::from_ymd(2019, 7, 7), date);
            assert!(validity_pattern_iterator.next().is_none());
        }

        #[test]
        fn operating_period_out_of_validity_period() {
            let validity_period = ValidityPeriod {
                start_date: Date::from_ymd(2019, 6, 1),
                end_date: Date::from_ymd(2019, 7, 31),
            };
            let weekday_pattern = vec![Weekday::Sat, Weekday::Sun].into_iter().collect();
            let operating_period = ValidityPeriod {
                start_date: Date::from_ymd(2018, 7, 1),
                end_date: Date::from_ymd(2018, 7, 7),
            };
            let day_type_assignments = vec![OperatingPeriod(&operating_period)]
                .into_iter()
                .collect();
            let mut validity_pattern_iterator = ValidityPatternIterator::new(
                &validity_period,
                &weekday_pattern,
                Some(&day_type_assignments),
            );
            assert!(validity_pattern_iterator.next().is_none());
        }

        #[test]
        fn active_day_out_of_validity_period() {
            let validity_period = ValidityPeriod {
                start_date: Date::from_ymd(2019, 6, 1),
                end_date: Date::from_ymd(2019, 7, 31),
            };
            let weekday_pattern = vec![Weekday::Sat, Weekday::Sun].into_iter().collect();
            let active_day = Date::from_ymd(2019, 5, 1);
            let day_type_assignments = vec![ActiveDay(active_day)].into_iter().collect();
            let mut validity_pattern_iterator = ValidityPatternIterator::new(
                &validity_period,
                &weekday_pattern,
                Some(&day_type_assignments),
            );
            assert!(validity_pattern_iterator.next().is_none());
        }

        #[test]
        fn only_one_active_day() {
            let validity_period = ValidityPeriod {
                start_date: Date::from_ymd(2019, 6, 1),
                end_date: Date::from_ymd(2019, 7, 31),
            };
            let weekday_pattern = HashSet::new();
            let active_day = Date::from_ymd(2019, 7, 6);
            let day_type_assignments = vec![ActiveDay(active_day)].into_iter().collect();
            let mut validity_pattern_iterator = ValidityPatternIterator::new(
                &validity_period,
                &weekday_pattern,
                Some(&day_type_assignments),
            );
            let date = validity_pattern_iterator.next().unwrap();
            assert_eq!(Date::from_ymd(2019, 7, 6), date);
            assert!(validity_pattern_iterator.next().is_none());
        }

        #[test]
        fn inactive_day_before_operating_period() {
            let validity_period = ValidityPeriod {
                start_date: Date::from_ymd(2019, 6, 1),
                end_date: Date::from_ymd(2019, 7, 31),
            };
            let weekday_pattern = vec![Weekday::Sat, Weekday::Sun].into_iter().collect();
            let operating_period = ValidityPeriod {
                start_date: Date::from_ymd(2019, 7, 6),
                end_date: Date::from_ymd(2019, 7, 7),
            };
            let inactive_day = Date::from_ymd(2019, 7, 6);
            let day_type_assignments = vec![
                InactiveDay(inactive_day),
                OperatingPeriod(&operating_period),
            ]
            .into_iter()
            .collect();
            let mut validity_pattern_iterator = ValidityPatternIterator::new(
                &validity_period,
                &weekday_pattern,
                Some(&day_type_assignments),
            );
            let date = validity_pattern_iterator.next().unwrap();
            assert_eq!(Date::from_ymd(2019, 7, 7), date);
            assert!(validity_pattern_iterator.next().is_none());
        }

        #[test]
        fn day_type_assignments_order() {
            let dta1 = DayTypeAssignment::ActiveDay(Date::from_ymd(2020, 1, 1));
            let validity_period = ValidityPeriod {
                start_date: Date::from_ymd(2020, 1, 1),
                end_date: Date::from_ymd(2020, 1, 1),
            };
            let dta2 = DayTypeAssignment::OperatingPeriod(&validity_period);
            assert_eq!(Ordering::Greater, dta1.cmp(&dta2));
        }
    }
}
