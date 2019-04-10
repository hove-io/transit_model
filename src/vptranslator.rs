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

//use crate::common_format;

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use num_traits::FromPrimitive;
use std::collections::BTreeSet;
use std::vec::Vec;

#[derive(Debug, PartialOrd)]
struct ValidityPeriod {
    start_date: NaiveDate,
    end_date: NaiveDate,
}

impl Default for ValidityPeriod {
    fn default() -> Self {
        ValidityPeriod {
            start_date: chrono::naive::MIN_DATE,
            end_date: chrono::naive::MIN_DATE,
        }
    }
}

impl PartialEq for ValidityPeriod {
    fn eq(&self, other: &Self) -> bool {
        (self.start_date, &self.end_date) == (other.start_date, &other.end_date)
    }
}

#[derive(Clone, Debug, PartialEq)]
enum ExceptionType {
    Sub,
    Add,
}

#[derive(Debug)]
struct ExceptionDate {
    date: NaiveDate,
    exception_type: ExceptionType,
}

impl PartialEq for ExceptionDate {
    fn eq(&self, other: &ExceptionDate) -> bool {
        self.exception_type == other.exception_type
    }
}

#[derive(Default, Debug)]
pub struct BlockPattern {
    week: u8,
    nb_weeks: i64,
    operating_days: Vec<Weekday>,
    validity_periods: Vec<ValidityPeriod>,
    exceptions: Vec<ExceptionDate>,
}

fn get_prev_monday(date: NaiveDate) -> NaiveDate {
    let res = date + Duration::days(-1 * date.weekday().num_days_from_monday() as i64);
    res
}

fn weeks(dates: &BTreeSet<NaiveDate>) -> Vec<u8> {
    let start_date: NaiveDate = get_prev_monday(*dates.iter().next().unwrap());
    let end_date: NaiveDate = *dates.iter().next_back().unwrap();
    let length = (end_date.signed_duration_since(start_date).num_weeks() + 1) as usize;

    let mut res = vec![0; length];
    for date in dates {
        let w = date.signed_duration_since(start_date).num_weeks() as usize;
        res[w] |= 1 << (7 - date.weekday().number_from_monday());
    }
    res
}

fn dists(w: u8, weeks: &Vec<u8>) -> u32 {
    let mut res: u32 = 0;
    for n in weeks.iter() {
        if *n != 0 {
            res += (*n as u8 ^ w).count_ones();
        }
    }
    res
}

fn get_min_week_pattern(weeks: &Vec<u8>) -> u8 {
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
    start_date: NaiveDate,
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

fn clean_extra_dates(start_date: NaiveDate, end_date: NaiveDate, dates: &mut Vec<ExceptionDate>) {
    dates.retain(|d| d.date >= start_date && d.date <= end_date);
}

pub fn translate(dates: &BTreeSet<NaiveDate>) -> BlockPattern {
    let mut res = BlockPattern {
        week: 0,
        nb_weeks: 0,
        operating_days: Vec::new(),
        validity_periods: Vec::new(),
        exceptions: Vec::new(),
    };
    if !dates.is_empty() {
        let validity_pattern = &weeks(&dates);
        res.week = get_min_week_pattern(validity_pattern);
        res.nb_weeks = validity_pattern.len() as i64;
        fill_operating_days(res.week, &mut res.operating_days);

        let start_date: NaiveDate = *dates.iter().next().unwrap();
        let end_date: NaiveDate = *dates.iter().next_back().unwrap();

        let validity_period = ValidityPeriod {
            start_date: start_date,
            end_date: end_date,
        };
        res.validity_periods.push(validity_period);

        let mut monday_ref = get_prev_monday(start_date);
        for week in validity_pattern {
            if *week != res.week {
                let exception: u8 = (!res.week) & week;
                fill_exceptions(
                    monday_ref,
                    exception,
                    ExceptionType::Add,
                    &mut res.exceptions,
                );

                let exception: u8 = (week ^ res.week) & res.week;
                fill_exceptions(
                    monday_ref,
                    exception,
                    ExceptionType::Sub,
                    &mut res.exceptions,
                );
            }
            monday_ref += Duration::days(7);
        }
        clean_extra_dates(start_date, end_date, &mut res.exceptions);
    };
    res
}
