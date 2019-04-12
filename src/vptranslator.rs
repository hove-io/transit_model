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

//! See function translate

use crate::objects::{Date, ExceptionType, ValidityPeriod};
use chrono::{Datelike, Duration, Weekday};
use num_traits::cast::FromPrimitive;
use std::collections::BTreeSet;
use std::vec::Vec;

///Indicates whether service is available on the date specified.
#[derive(Debug)]
pub struct ExceptionDate {
    ///Date of exception
    pub date: Date,
    ///exception type
    pub exception_type: ExceptionType,
}

impl PartialEq for ExceptionDate {
    fn eq(&self, other: &ExceptionDate) -> bool {
        self.exception_type == other.exception_type
    }
}

///Presents a list of dates in the form of intervals and exception dates.
#[derive(Default, Debug)]
pub struct BlockPattern {
    ///Indicates operating days of the service
    pub operating_days: Vec<Weekday>,
    ///Start and end service day for the service interval
    pub validity_periods: Vec<ValidityPeriod>,
    ///List of dates where service is available or not in interval.
    pub exceptions: Vec<ExceptionDate>,
}

fn get_prev_monday(date: Date) -> Date {
    date + Duration::days(-1 * date.weekday().num_days_from_monday() as i64)
}

fn compute_validity_pattern(start_date: Date, end_date: Date, dates: &BTreeSet<Date>) -> Vec<u8> {
    let length = (end_date.signed_duration_since(start_date).num_weeks() + 1) as usize;
    let mut res = vec![0; length];
    for date in dates {
        let w = date.signed_duration_since(start_date).num_weeks() as usize;
        res[w] |= 1 << (7 - date.weekday().number_from_monday());
    }
    res
}

fn dists(w: u8, weeks: &[u8]) -> u32 {
    let res = weeks
        .iter()
        .filter(|n| **n != 0u8)
        .map(|n| (*n as u8 ^ w).count_ones())
        .sum();
    res
}

fn get_min_week_pattern(weeks: &[u8]) -> u8 {
    let mut best: u8 = 0;
    let mut best_score = std::u32::MAX;
    for i in 0..128 {
        let score = dists(i, weeks);
        if (score < best_score)
            || ((score == best_score) && (score.count_ones() < best_score.count_ones()))
        {
            best_score = score;
            best = i;
        }
    }
    best
}

fn fill_exceptions(
    start_date: Date,
    exception: u8,
    exception_type: ExceptionType,
    exception_list: &mut Vec<ExceptionDate>,
) {
    for i in 0..7 {
        if exception & (1 << i) == (1 << i) {
            let date = start_date + Duration::days((6 - i) as i64);
            exception_list.push(ExceptionDate {
                date: date,
                exception_type: exception_type.clone(),
            });
        }
    }
}

fn fill_operating_days(week: u8, operating_days: &mut Vec<Weekday>) {
    for i in 0..7 {
        if week & (1 << i) == (1 << i) {
            operating_days.push(Weekday::from_u8(6 - i).unwrap());
        }
    }
    operating_days.sort_by_key(|w| w.num_days_from_monday());
}

fn clean_extra_dates(start_date: Date, end_date: Date, dates: &mut Vec<ExceptionDate>) {
    dates.retain(|d| d.date >= start_date && d.date <= end_date);
}
///Allows you to present a list of dates in a readable way.
pub fn translate(dates: &BTreeSet<Date>) -> BlockPattern {
    let mut res = BlockPattern::default();
    let start_date = match dates.iter().next() {
        Some(d) => *d,
        None => return res,
    };
    let end_date = match dates.iter().next_back() {
        Some(d) => *d,
        None => return res,
    };

    let validity_period = ValidityPeriod {
        start_date: start_date,
        end_date: end_date,
    };
    res.validity_periods.push(validity_period);

    let mut monday_ref = get_prev_monday(start_date);
    let validity_pattern = compute_validity_pattern(monday_ref, end_date, &dates);
    let best_week = get_min_week_pattern(&validity_pattern);
    fill_operating_days(best_week, &mut res.operating_days);

    for week in validity_pattern {
        if week != best_week {
            let exception: u8 = (!best_week) & week;
            fill_exceptions(
                monday_ref,
                exception,
                ExceptionType::Add,
                &mut res.exceptions,
            );

            let exception: u8 = (week ^ best_week) & best_week;
            fill_exceptions(
                monday_ref,
                exception,
                ExceptionType::Remove,
                &mut res.exceptions,
            );
        }
        monday_ref += Duration::days(7);
    }
    clean_extra_dates(start_date, end_date, &mut res.exceptions);
    res
}

//       July 2012
// Mo Tu We Th Fr Sa Su
//                    1
//  2  3  4  5  6  7  8
//  9 10 11 12 13 14 15
// 16 17 18 19 20 21 22
// 23 24 25 26 27 28 29
// 30 31

#[test]
fn nb_weeks() {
    let mut dates = BTreeSet::new();
    let mut start_date: Date;
    let mut end_date: Date;

    start_date = Date::from_ymd(2012, 7, 2);
    assert_eq!(get_prev_monday(start_date), start_date);

    start_date = Date::from_ymd(2012, 7, 5);
    assert_eq!(get_prev_monday(start_date), Date::from_ymd(2012, 7, 2));

    start_date = Date::from_ymd(2012, 7, 8);
    assert_eq!(get_prev_monday(start_date), Date::from_ymd(2012, 7, 2));

    // one week
    start_date = Date::from_ymd(2012, 7, 2);
    end_date = Date::from_ymd(2012, 7, 8);
    dates.insert(start_date);
    dates.insert(end_date);
    assert_eq!(
        compute_validity_pattern(get_prev_monday(start_date), end_date, &dates).len(),
        1
    );

    // partial + one week
    dates.clear();
    start_date = Date::from_ymd(2012, 7, 4);
    end_date = Date::from_ymd(2012, 7, 15);
    dates.insert(start_date);
    dates.insert(end_date);
    assert_eq!(
        compute_validity_pattern(get_prev_monday(start_date), end_date, &dates).len(),
        2
    );

    // one week + partial
    dates.clear();
    start_date = Date::from_ymd(2012, 7, 2);
    end_date = Date::from_ymd(2012, 7, 13);
    dates.insert(start_date);
    dates.insert(end_date);
    assert_eq!(
        compute_validity_pattern(get_prev_monday(start_date), end_date, &dates).len(),
        2
    );

    // partial + one week + partial with nb partial = 6
    dates.clear();
    start_date = Date::from_ymd(2012, 7, 4);
    end_date = Date::from_ymd(2012, 7, 17);
    dates.insert(start_date);
    dates.insert(end_date);
    assert_eq!(
        compute_validity_pattern(get_prev_monday(start_date), end_date, &dates).len(),
        3
    );

    // partial + one week + partial with nb partial = 7
    dates.clear();
    start_date = Date::from_ymd(2012, 7, 4);
    end_date = Date::from_ymd(2012, 7, 18);
    dates.insert(start_date);
    dates.insert(end_date);
    assert_eq!(
        compute_validity_pattern(get_prev_monday(start_date), end_date, &dates).len(),
        3
    );

    // partial + one week + partial with nb partial = 8
    dates.clear();
    start_date = Date::from_ymd(2012, 7, 4);
    end_date = Date::from_ymd(2012, 7, 19);
    dates.insert(start_date);
    dates.insert(end_date);
    assert_eq!(
        compute_validity_pattern(get_prev_monday(start_date), end_date, &dates).len(),
        3
    );
}
