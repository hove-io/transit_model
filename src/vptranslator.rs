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
#[derive(Debug, PartialEq)]
pub struct ExceptionDate {
    ///Date of exception
    pub date: Date,
    ///exception type
    pub exception_type: ExceptionType,
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
    date - Duration::days(date.weekday().num_days_from_monday() as i64)
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

///Determining a Hamming distance betwenn a validity pattern and a week pattern
fn dists(w: u8, weeks: &[u8]) -> u32 {
    weeks
        .iter()
        .filter(|n| **n != 0u8)
        .map(|n| (*n as u8 ^ w).count_ones())
        .sum()
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

fn get_operating_days(week: u8) -> Vec<Weekday> {
    let mut res = Vec::new();
    for i in 0..7 {
        if week & (1 << i) == (1 << i) {
            res.push(Weekday::from_u8(6 - i).unwrap());
        }
    }
    res.sort_by_key(|w| w.num_days_from_monday());
    res
}

fn clean_extra_dates(start_date: Date, end_date: Date, dates: &mut Vec<ExceptionDate>) {
    dates.retain(|d| d.date >= start_date && d.date <= end_date);
}

///Allows you to present a list of dates in a readable way.
pub fn translate(dates: &BTreeSet<Date>) -> BlockPattern {
    let start_date = match dates.iter().next() {
        Some(d) => *d,
        None => return BlockPattern::default(),
    };
    let end_date: Date = *dates.iter().next_back().unwrap();
    let validity_period = vec![ValidityPeriod {
        start_date: start_date,
        end_date: end_date,
    }];

    let mut monday_ref = get_prev_monday(start_date);
    let validity_pattern = compute_validity_pattern(monday_ref, end_date, &dates);
    let best_week = get_min_week_pattern(&validity_pattern);
    let operating_days = get_operating_days(best_week);
    let mut exceptions_list = Vec::new();

    for week in validity_pattern {
        if week != best_week {
            let exception: u8 = (!best_week) & week;
            fill_exceptions(
                monday_ref,
                exception,
                ExceptionType::Add,
                &mut exceptions_list,
            );

            let exception: u8 = (week ^ best_week) & best_week;
            fill_exceptions(
                monday_ref,
                exception,
                ExceptionType::Remove,
                &mut exceptions_list,
            );
        }
        monday_ref += Duration::days(7);
    }
    clean_extra_dates(start_date, end_date, &mut exceptions_list);
    BlockPattern {
        operating_days: operating_days,
        validity_periods: validity_period,
        exceptions: exceptions_list,
    }
}

//       July 2012
// Mo Tu We Th Fr Sa Su
//                    1
//  2  3  4  5  6  7  8
//  9 10 11 12 13 14 15
// 16 17 18 19 20 21 22
// 23 24 25 26 27 28 29
// 30 31

#[cfg(test)]
mod tests {
    use super::*;

    fn compute_between(start_date: Date, end_date: Date) -> Vec<u8> {
        let mut dates = BTreeSet::new();
        dates.insert(start_date);
        dates.insert(end_date);
        compute_validity_pattern(get_prev_monday(start_date), end_date, &dates)
    }

    #[test]
    fn one_week() {
        assert_eq!(
            compute_between(Date::from_ymd(2012, 7, 2), Date::from_ymd(2012, 7, 8)).len(),
            1
        );
    }

    #[test]
    fn partial_one_week() {
        assert_eq!(
            compute_between(Date::from_ymd(2012, 7, 4), Date::from_ymd(2012, 7, 15)).len(),
            2
        );
    }

    #[test]
    fn one_week_partial() {
        assert_eq!(
            compute_between(Date::from_ymd(2012, 7, 2), Date::from_ymd(2012, 7, 13)).len(),
            2
        );
    }

    #[test]
    fn partial_one_week_partial_with_nb_partial_6() {
        assert_eq!(
            compute_between(Date::from_ymd(2012, 7, 4), Date::from_ymd(2012, 7, 17)).len(),
            3
        );
    }

    #[test]
    fn partial_one_week_partial_with_nb_partial_7() {
        assert_eq!(
            compute_between(Date::from_ymd(2012, 7, 4), Date::from_ymd(2012, 7, 18)).len(),
            3
        );
    }

    #[test]
    fn partial_one_week_partial_with_nb_partial_8() {
        assert_eq!(
            compute_between(Date::from_ymd(2012, 7, 4), Date::from_ymd(2012, 7, 19)).len(),
            3
        );
    }

    #[test]
    fn prev_monday_from_monday() {
        assert_eq!(
            get_prev_monday(Date::from_ymd(2012, 7, 2)),
            Date::from_ymd(2012, 7, 2)
        );
    }

    #[test]
    fn prev_monday_from_thursday() {
        assert_eq!(
            get_prev_monday(Date::from_ymd(2012, 7, 5)),
            Date::from_ymd(2012, 7, 2)
        );
    }

    #[test]
    fn prev_monday_from_sunday() {
        assert_eq!(
            get_prev_monday(Date::from_ymd(2012, 7, 8)),
            Date::from_ymd(2012, 7, 2)
        );
    }
}
