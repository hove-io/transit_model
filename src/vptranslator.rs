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

//! See function translate

use crate::objects::{Date, ExceptionType, ValidityPeriod};
use chrono::{Datelike, Duration, Weekday};
use num_traits::cast::FromPrimitive;
use std::collections::BTreeSet;
use std::vec::Vec;

///Indicates whether service is available on the date specified.
#[derive(Debug, Eq, PartialEq)]
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
    pub validity_period: Option<ValidityPeriod>,
    ///List of dates where service is available or not in interval.
    pub exceptions: Vec<ExceptionDate>,
}

fn get_prev_monday(date: Date) -> Date {
    date - Duration::days(i64::from(date.weekday().num_days_from_monday()))
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

///Determining the sum of Hamming distances between a validity pattern and a week pattern
fn dists(w: u8, weeks: &[u8]) -> u32 {
    weeks
        .iter()
        .filter(|n| **n != 0u8)
        .map(|n| (*n ^ w).count_ones())
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
            let date = start_date + Duration::days(i64::from(6 - i));
            exception_list.push(ExceptionDate {
                date,
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
    res.sort_by_key(Weekday::num_days_from_monday);
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

    let mut monday_ref = get_prev_monday(start_date);
    let validity_pattern = compute_validity_pattern(monday_ref, end_date, dates);
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
        operating_days,
        validity_period: Some(ValidityPeriod {
            start_date,
            end_date,
        }),
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
    use pretty_assertions::assert_eq;

    fn compute_between(start_date: Date, end_date: Date) -> Vec<u8> {
        let mut dates = BTreeSet::new();
        dates.insert(start_date);
        dates.insert(end_date);
        compute_validity_pattern(get_prev_monday(start_date), end_date, &dates)
    }

    #[test]
    fn one_week() {
        assert_eq!(
            1,
            compute_between(
                Date::from_ymd_opt(2012, 7, 2).unwrap(),
                Date::from_ymd_opt(2012, 7, 8).unwrap()
            )
            .len()
        );
    }

    #[test]
    fn partial_one_week() {
        assert_eq!(
            2,
            compute_between(
                Date::from_ymd_opt(2012, 7, 4).unwrap(),
                Date::from_ymd_opt(2012, 7, 15).unwrap()
            )
            .len()
        );
    }

    #[test]
    fn one_week_partial() {
        assert_eq!(
            2,
            compute_between(
                Date::from_ymd_opt(2012, 7, 2).unwrap(),
                Date::from_ymd_opt(2012, 7, 13).unwrap()
            )
            .len()
        );
    }

    #[test]
    fn partial_one_week_partial_with_nb_partial_6() {
        assert_eq!(
            3,
            compute_between(
                Date::from_ymd_opt(2012, 7, 4).unwrap(),
                Date::from_ymd_opt(2012, 7, 17).unwrap()
            )
            .len()
        );
    }

    #[test]
    fn partial_one_week_partial_with_nb_partial_7() {
        assert_eq!(
            3,
            compute_between(
                Date::from_ymd_opt(2012, 7, 4).unwrap(),
                Date::from_ymd_opt(2012, 7, 18).unwrap()
            )
            .len()
        );
    }

    #[test]
    fn partial_one_week_partial_with_nb_partial_8() {
        assert_eq!(
            3,
            compute_between(
                Date::from_ymd_opt(2012, 7, 4).unwrap(),
                Date::from_ymd_opt(2012, 7, 19).unwrap()
            )
            .len()
        );
    }

    #[test]
    fn prev_monday_from_monday() {
        assert_eq!(
            Date::from_ymd_opt(2012, 7, 2).unwrap(),
            get_prev_monday(Date::from_ymd_opt(2012, 7, 2).unwrap())
        );
    }

    #[test]
    fn prev_monday_from_thursday() {
        assert_eq!(
            Date::from_ymd_opt(2012, 7, 2).unwrap(),
            get_prev_monday(Date::from_ymd_opt(2012, 7, 5).unwrap())
        );
    }

    #[test]
    fn prev_monday_from_sunday() {
        assert_eq!(
            Date::from_ymd_opt(2012, 7, 2).unwrap(),
            get_prev_monday(Date::from_ymd_opt(2012, 7, 8).unwrap())
        );
    }

    fn get_dates_from_bitset(start_date: Date, vpattern: &str) -> BTreeSet<Date> {
        let mut res = BTreeSet::new();
        for (i, c) in vpattern.chars().enumerate() {
            if c == '1' {
                res.insert(start_date + Duration::days(i as i64));
            }
        }
        res
    }

    fn get_week_from_weekday(weekday: Vec<Weekday>) -> u8 {
        let mut res = 0;
        for day in weekday {
            res |= 1 << (7 - day.number_from_monday());
        }
        res
    }

    #[test]
    fn only_first_day() {
        let mut dates = BTreeSet::new();

        dates.insert(Date::from_ymd_opt(2012, 7, 2).unwrap());
        let res = translate(&dates);
        assert_eq!(0b100_0000, get_week_from_weekday(res.operating_days));

        assert!(res.exceptions.is_empty());
        assert_eq!(
            ValidityPeriod {
                start_date: Date::from_ymd_opt(2012, 7, 2).unwrap(),
                end_date: Date::from_ymd_opt(2012, 7, 2).unwrap(),
            },
            res.validity_period.unwrap()
        );
    }

    #[test]
    fn bound_cut() {
        let res = translate(&get_dates_from_bitset(
            Date::from_ymd_opt(2012, 7, 16).unwrap(),
            &format!("{}{}", "0011101", "000"),
        ));

        assert_eq!(0b001_1101, get_week_from_weekday(res.operating_days));
        assert!(res.exceptions.is_empty());
        assert_eq!(
            ValidityPeriod {
                start_date: Date::from_ymd_opt(2012, 7, 18).unwrap(),
                end_date: Date::from_ymd_opt(2012, 7, 22).unwrap(),
            },
            res.validity_period.unwrap()
        );
    }

    #[test]
    fn bound_cut_one_day() {
        let res = translate(&get_dates_from_bitset(
            Date::from_ymd_opt(2012, 7, 16).unwrap(),
            &format!("{}{}", "0000010", "00"),
        ));

        assert_eq!(0b000_0010, get_week_from_weekday(res.operating_days));
        assert!(res.exceptions.is_empty());
        assert_eq!(
            ValidityPeriod {
                start_date: Date::from_ymd_opt(2012, 7, 21).unwrap(),
                end_date: Date::from_ymd_opt(2012, 7, 21).unwrap(),
            },
            res.validity_period.unwrap()
        )
    }

    #[test]
    fn empty_vp() {
        let res = translate(&get_dates_from_bitset(
            Date::from_ymd_opt(2012, 7, 16).unwrap(),
            "0000000",
        ));

        assert_eq!(0b000_0000, get_week_from_weekday(res.operating_days));
        assert!(res.exceptions.is_empty());
        assert!(res.validity_period.is_none());
    }

    #[test]
    fn only_one_thursday() {
        let res = translate(&get_dates_from_bitset(
            Date::from_ymd_opt(2012, 7, 2).unwrap(),
            &format!("{}{}", "0000000", "0001000"),
        ));

        assert_eq!(0b000_1000, get_week_from_weekday(res.operating_days));
        assert!(res.exceptions.is_empty());
        assert_eq!(
            ValidityPeriod {
                start_date: Date::from_ymd_opt(2012, 7, 12).unwrap(),
                end_date: Date::from_ymd_opt(2012, 7, 12).unwrap(),
            },
            res.validity_period.unwrap()
        )
    }

    #[test]
    fn only_one_monday() {
        let res = translate(&get_dates_from_bitset(
            Date::from_ymd_opt(2012, 7, 2).unwrap(),
            &format!("{}{}", "0000000", "1000000"),
        ));

        assert_eq!(0b100_0000, get_week_from_weekday(res.operating_days));
        assert!(res.exceptions.is_empty());
        assert_eq!(
            ValidityPeriod {
                start_date: Date::from_ymd_opt(2012, 7, 9).unwrap(),
                end_date: Date::from_ymd_opt(2012, 7, 9).unwrap(),
            },
            res.validity_period.unwrap()
        )
    }

    #[test]
    fn only_one_sunday() {
        let res = translate(&get_dates_from_bitset(
            Date::from_ymd_opt(2012, 7, 2).unwrap(),
            &format!("{}{}", "0000001", "0000000"),
        ));

        assert_eq!(0b000_0001, get_week_from_weekday(res.operating_days));
        assert!(res.exceptions.is_empty());
        assert_eq!(
            ValidityPeriod {
                start_date: Date::from_ymd_opt(2012, 7, 8).unwrap(),
                end_date: Date::from_ymd_opt(2012, 7, 8).unwrap(),
            },
            res.validity_period.unwrap()
        )
    }

    // only one thursday friday saturday sunday
    #[test]
    fn only_one_tfss() {
        let res = translate(&get_dates_from_bitset(
            Date::from_ymd_opt(2012, 7, 2).unwrap(),
            &format!("{}{}", "0000000", "0001111"),
        ));

        assert_eq!(0b000_1111, get_week_from_weekday(res.operating_days));
        assert!(res.exceptions.is_empty());
        assert_eq!(
            ValidityPeriod {
                start_date: Date::from_ymd_opt(2012, 7, 12).unwrap(),
                end_date: Date::from_ymd_opt(2012, 7, 15).unwrap(),
            },
            res.validity_period.unwrap()
        )
    }

    #[test]
    fn three_complete_weeks() {
        let res = translate(&get_dates_from_bitset(
            Date::from_ymd_opt(2012, 7, 2).unwrap(),
            &format!("{}{}{}", "1111111", "1111111", "1111111"),
        ));

        assert_eq!(0b111_1111, get_week_from_weekday(res.operating_days));
        assert!(res.exceptions.is_empty());
        assert_eq!(
            ValidityPeriod {
                start_date: Date::from_ymd_opt(2012, 7, 2).unwrap(),
                end_date: Date::from_ymd_opt(2012, 7, 22).unwrap(),
            },
            res.validity_period.unwrap()
        )
    }

    #[test]
    fn three_mtwss_excluding_one_day() {
        let res = translate(&get_dates_from_bitset(
            Date::from_ymd_opt(2012, 7, 2).unwrap(),
            &format!("{}{}{}", "1100111", "1100011", "1100111"),
        ));

        assert_eq!(0b110_0111, get_week_from_weekday(res.operating_days));
        assert_eq!(1, res.exceptions.len());
        assert_eq!(
            &ExceptionDate {
                date: Date::from_ymd_opt(2012, 7, 13).unwrap(),
                exception_type: ExceptionType::Remove,
            },
            res.exceptions.first().unwrap()
        );
        assert_eq!(
            ValidityPeriod {
                start_date: Date::from_ymd_opt(2012, 7, 2).unwrap(),
                end_date: Date::from_ymd_opt(2012, 7, 22).unwrap(),
            },
            res.validity_period.unwrap()
        )
    }

    #[test]
    fn three_mtwss_including_one_day() {
        let res = translate(&get_dates_from_bitset(
            Date::from_ymd_opt(2012, 7, 2).unwrap(),
            &format!("{}{}{}", "1100111", "1101111", "1100111"),
        ));

        assert_eq!(0b110_0111, get_week_from_weekday(res.operating_days));
        assert_eq!(1, res.exceptions.len());
        assert_eq!(
            &ExceptionDate {
                date: Date::from_ymd_opt(2012, 7, 12).unwrap(),
                exception_type: ExceptionType::Add,
            },
            res.exceptions.first().unwrap()
        );
        assert_eq!(
            ValidityPeriod {
                start_date: Date::from_ymd_opt(2012, 7, 2).unwrap(),
                end_date: Date::from_ymd_opt(2012, 7, 22).unwrap(),
            },
            res.validity_period.unwrap()
        )
    }

    #[test]
    fn mwtfss_mttfss_mtwfss() {
        let res = translate(&get_dates_from_bitset(
            Date::from_ymd_opt(2012, 7, 2).unwrap(),
            &format!("{}{}{}", "1011111", "1101111", "1110111"),
        ));

        assert_eq!(0b111_1111, get_week_from_weekday(res.operating_days));
        assert_eq!(3, res.exceptions.len());

        assert_eq!(
            ExceptionDate {
                date: Date::from_ymd_opt(2012, 7, 3).unwrap(),
                exception_type: ExceptionType::Remove,
            },
            res.exceptions[0]
        );
        assert_eq!(
            ExceptionDate {
                date: Date::from_ymd_opt(2012, 7, 11).unwrap(),
                exception_type: ExceptionType::Remove,
            },
            res.exceptions[1]
        );
        assert_eq!(
            ExceptionDate {
                date: Date::from_ymd_opt(2012, 7, 19).unwrap(),
                exception_type: ExceptionType::Remove,
            },
            res.exceptions[2]
        );
        assert_eq!(
            ValidityPeriod {
                start_date: Date::from_ymd_opt(2012, 7, 2).unwrap(),
                end_date: Date::from_ymd_opt(2012, 7, 22).unwrap(),
            },
            res.validity_period.unwrap()
        )
    }

    #[test]
    fn t_w_t() {
        let res = translate(&get_dates_from_bitset(
            Date::from_ymd_opt(2012, 7, 2).unwrap(),
            &format!("{}{}{}", "0100000", "0010000", "0001000"),
        ));

        assert_eq!(0b000_0000, get_week_from_weekday(res.operating_days));
        assert_eq!(3, res.exceptions.len());
        assert_eq!(
            ExceptionDate {
                date: Date::from_ymd_opt(2012, 7, 3).unwrap(),
                exception_type: ExceptionType::Add,
            },
            res.exceptions[0]
        );
        assert_eq!(
            ExceptionDate {
                date: Date::from_ymd_opt(2012, 7, 11).unwrap(),
                exception_type: ExceptionType::Add,
            },
            res.exceptions[1]
        );
        assert_eq!(
            ExceptionDate {
                date: Date::from_ymd_opt(2012, 7, 19).unwrap(),
                exception_type: ExceptionType::Add,
            },
            res.exceptions[2]
        );
        assert_eq!(
            ValidityPeriod {
                start_date: Date::from_ymd_opt(2012, 7, 3).unwrap(),
                end_date: Date::from_ymd_opt(2012, 7, 19).unwrap(),
            },
            res.validity_period.unwrap()
        )
    }

    #[test]
    fn bound_compression() {
        let res = translate(&get_dates_from_bitset(
            Date::from_ymd_opt(2012, 7, 2).unwrap(),
            &format!("{}{}{}", "0000111", "0001111", "0001110"),
        ));

        assert_eq!(0b000_1111, get_week_from_weekday(res.operating_days));
        assert!(res.exceptions.is_empty());
        assert_eq!(
            ValidityPeriod {
                start_date: Date::from_ymd_opt(2012, 7, 6).unwrap(),
                end_date: Date::from_ymd_opt(2012, 7, 21).unwrap(),
            },
            res.validity_period.unwrap()
        )
    }

    // ROADEF 2015 example
    #[test]
    fn may2015() {
        let res = translate(&get_dates_from_bitset(
            Date::from_ymd_opt(2015, 4, 27).unwrap(),
            &format!(
                "{}{}{}{}{}",
                "1111000", "1111000", "1110100", "1111100", "0111110"
            ),
        ));

        assert_eq!(0b111_1100, get_week_from_weekday(res.operating_days));
        assert_eq!(5, res.exceptions.len());
        assert_eq!(
            ExceptionDate {
                date: Date::from_ymd_opt(2015, 5, 1).unwrap(),
                exception_type: ExceptionType::Remove,
            },
            res.exceptions[0]
        );
        assert_eq!(
            ExceptionDate {
                date: Date::from_ymd_opt(2015, 5, 8).unwrap(),
                exception_type: ExceptionType::Remove,
            },
            res.exceptions[1]
        );
        assert_eq!(
            ExceptionDate {
                date: Date::from_ymd_opt(2015, 5, 14).unwrap(),
                exception_type: ExceptionType::Remove,
            },
            res.exceptions[2]
        );
        assert_eq!(
            ExceptionDate {
                date: Date::from_ymd_opt(2015, 5, 30).unwrap(),
                exception_type: ExceptionType::Add,
            },
            res.exceptions[3]
        );
        assert_eq!(
            ExceptionDate {
                date: Date::from_ymd_opt(2015, 5, 25).unwrap(),
                exception_type: ExceptionType::Remove,
            },
            res.exceptions[4]
        );
        assert_eq!(
            ValidityPeriod {
                start_date: Date::from_ymd_opt(2015, 4, 27).unwrap(),
                end_date: Date::from_ymd_opt(2015, 5, 30).unwrap(),
            },
            res.validity_period.unwrap()
        )
    }
}
