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

use chrono::{Duration, Weekday};
use std::collections::BTreeSet;
use transit_model;
use transit_model::objects::{Date, ExceptionType, ValidityPeriod};
use transit_model::vptranslator::{translate, ExceptionDate};

//       July 2012
// Mo Tu We Th Fr Sa Su
//                    1
//  2  3  4  5  6  7  8
//  9 10 11 12 13 14 15
// 16 17 18 19 20 21 22
// 23 24 25 26 27 28 29
// 30 31

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

    dates.insert(Date::from_ymd(2012, 7, 2));
    let res = translate(&dates);
    assert_eq!(get_week_from_weekday(res.operating_days), 0b1000000);

    assert!(res.exceptions.is_empty());
    assert_eq!(
        res.validity_periods.iter().next().unwrap(),
        &ValidityPeriod {
            start_date: Date::from_ymd(2012, 7, 2),
            end_date: Date::from_ymd(2012, 7, 2),
        }
    );
}

#[test]
fn bound_cut() {
    let res = translate(&get_dates_from_bitset(
        Date::from_ymd(2012, 7, 16),
        &format!("{}{}", "0011101", "000"),
    ));

    assert_eq!(get_week_from_weekday(res.operating_days), 0b0011101);
    assert!(res.exceptions.is_empty());
    assert_eq!(
        res.validity_periods.iter().next().unwrap(),
        &ValidityPeriod {
            start_date: Date::from_ymd(2012, 7, 18),
            end_date: Date::from_ymd(2012, 7, 22),
        }
    );
}

#[test]
fn bound_cut_one_day() {
    let res = translate(&get_dates_from_bitset(
        Date::from_ymd(2012, 7, 16),
        &format!("{}{}", "0000010", "00"),
    ));

    assert_eq!(get_week_from_weekday(res.operating_days), 0b0000010);
    assert!(res.exceptions.is_empty());
    assert_eq!(
        res.validity_periods.iter().next().unwrap(),
        &ValidityPeriod {
            start_date: Date::from_ymd(2012, 7, 21),
            end_date: Date::from_ymd(2012, 7, 21),
        }
    )
}

#[test]
fn empty_vp() {
    let res = translate(&get_dates_from_bitset(
        Date::from_ymd(2012, 7, 16),
        &format!("{}", "0000000"),
    ));

    assert_eq!(get_week_from_weekday(res.operating_days), 0b0000000);
    assert!(res.exceptions.is_empty());
    assert!(res.validity_periods.is_empty());
}

#[test]
fn only_one_thursday() {
    let res = translate(&get_dates_from_bitset(
        Date::from_ymd(2012, 7, 2),
        &format!("{}{}", "0000000", "0001000"),
    ));

    assert_eq!(get_week_from_weekday(res.operating_days), 0b0001000);
    assert!(res.exceptions.is_empty());
    assert_eq!(
        res.validity_periods.iter().next().unwrap(),
        &ValidityPeriod {
            start_date: Date::from_ymd(2012, 7, 12),
            end_date: Date::from_ymd(2012, 7, 12),
        }
    )
}

#[test]
fn only_one_monday() {
    let res = translate(&get_dates_from_bitset(
        Date::from_ymd(2012, 7, 2),
        &format!("{}{}", "0000000", "1000000"),
    ));

    assert_eq!(get_week_from_weekday(res.operating_days), 0b1000000);
    assert!(res.exceptions.is_empty());
    assert_eq!(
        res.validity_periods.iter().next().unwrap(),
        &ValidityPeriod {
            start_date: Date::from_ymd(2012, 7, 9),
            end_date: Date::from_ymd(2012, 7, 9),
        }
    )
}

#[test]
fn only_one_sunday() {
    let res = translate(&get_dates_from_bitset(
        Date::from_ymd(2012, 7, 2),
        &format!("{}{}", "0000001", "0000000"),
    ));

    assert_eq!(get_week_from_weekday(res.operating_days), 0b0000001);
    assert!(res.exceptions.is_empty());
    assert_eq!(
        res.validity_periods.iter().next().unwrap(),
        &ValidityPeriod {
            start_date: Date::from_ymd(2012, 7, 8),
            end_date: Date::from_ymd(2012, 7, 8),
        }
    )
}

// only one thursday friday saturday sunday
#[test]
fn only_one_tfss() {
    let res = translate(&get_dates_from_bitset(
        Date::from_ymd(2012, 7, 2),
        &format!("{}{}", "0000000", "0001111"),
    ));

    assert_eq!(get_week_from_weekday(res.operating_days), 0b0001111);
    assert!(res.exceptions.is_empty());
    assert_eq!(
        res.validity_periods.iter().next().unwrap(),
        &ValidityPeriod {
            start_date: Date::from_ymd(2012, 7, 12),
            end_date: Date::from_ymd(2012, 7, 15),
        }
    )
}

#[test]
fn three_complete_weeks() {
    let res = translate(&get_dates_from_bitset(
        Date::from_ymd(2012, 7, 2),
        &format!("{}{}{}", "1111111", "1111111", "1111111"),
    ));

    assert_eq!(get_week_from_weekday(res.operating_days), 0b1111111);
    assert!(res.exceptions.is_empty());
    assert_eq!(
        res.validity_periods.iter().next().unwrap(),
        &ValidityPeriod {
            start_date: Date::from_ymd(2012, 7, 2),
            end_date: Date::from_ymd(2012, 7, 22),
        }
    )
}

#[test]
fn three_mtwss_excluding_one_day() {
    let res = translate(&get_dates_from_bitset(
        Date::from_ymd(2012, 7, 2),
        &format!("{}{}{}", "1100111", "1100011", "1100111"),
    ));

    assert_eq!(get_week_from_weekday(res.operating_days), 0b1100111);
    assert_eq!(res.exceptions.len(), 1);
    assert_eq!(
        res.exceptions.iter().next().unwrap(),
        &ExceptionDate {
            date: Date::from_ymd(2012, 07, 13),
            exception_type: ExceptionType::Remove,
        }
    );
    assert_eq!(
        res.validity_periods.iter().next().unwrap(),
        &ValidityPeriod {
            start_date: Date::from_ymd(2012, 7, 2),
            end_date: Date::from_ymd(2012, 7, 22),
        }
    )
}

#[test]
fn three_mtwss_including_one_day() {
    let res = translate(&get_dates_from_bitset(
        Date::from_ymd(2012, 7, 2),
        &format!("{}{}{}", "1100111", "1101111", "1100111"),
    ));

    assert_eq!(get_week_from_weekday(res.operating_days), 0b1100111);
    assert_eq!(res.exceptions.len(), 1);
    assert_eq!(
        res.exceptions.iter().next().unwrap(),
        &ExceptionDate {
            date: Date::from_ymd(2012, 07, 12),
            exception_type: ExceptionType::Add,
        }
    );
    assert_eq!(
        res.validity_periods.iter().next().unwrap(),
        &ValidityPeriod {
            start_date: Date::from_ymd(2012, 7, 2),
            end_date: Date::from_ymd(2012, 7, 22),
        }
    )
}

#[test]
fn mwtfss_mttfss_mtwfss() {
    let res = translate(&get_dates_from_bitset(
        Date::from_ymd(2012, 7, 2),
        &format!("{}{}{}", "1011111", "1101111", "1110111"),
    ));

    assert_eq!(get_week_from_weekday(res.operating_days), 0b1111111);
    assert_eq!(res.exceptions.len(), 3);

    assert_eq!(
        res.exceptions[0],
        ExceptionDate {
            date: Date::from_ymd(2012, 7, 3),
            exception_type: ExceptionType::Remove,
        }
    );
    assert_eq!(
        res.exceptions[1],
        ExceptionDate {
            date: Date::from_ymd(2012, 7, 11),
            exception_type: ExceptionType::Remove,
        }
    );
    assert_eq!(
        res.exceptions[2],
        ExceptionDate {
            date: Date::from_ymd(2012, 7, 19),
            exception_type: ExceptionType::Remove,
        }
    );
    assert_eq!(
        res.validity_periods.iter().next().unwrap(),
        &ValidityPeriod {
            start_date: Date::from_ymd(2012, 7, 2),
            end_date: Date::from_ymd(2012, 7, 22),
        }
    )
}

#[test]
fn t_w_t() {
    let res = translate(&get_dates_from_bitset(
        Date::from_ymd(2012, 7, 2),
        &format!("{}{}{}", "0100000", "0010000", "0001000"),
    ));

    assert_eq!(get_week_from_weekday(res.operating_days), 0b0000000);
    assert_eq!(res.exceptions.len(), 3);
    assert_eq!(
        res.exceptions[0],
        ExceptionDate {
            date: Date::from_ymd(2012, 7, 3),
            exception_type: ExceptionType::Add,
        }
    );
    assert_eq!(
        res.exceptions[1],
        ExceptionDate {
            date: Date::from_ymd(2012, 7, 11),
            exception_type: ExceptionType::Add,
        }
    );
    assert_eq!(
        res.exceptions[2],
        ExceptionDate {
            date: Date::from_ymd(2012, 7, 19),
            exception_type: ExceptionType::Add,
        }
    );
    assert_eq!(
        res.validity_periods.iter().next().unwrap(),
        &ValidityPeriod {
            start_date: Date::from_ymd(2012, 7, 3),
            end_date: Date::from_ymd(2012, 7, 19),
        }
    )
}

#[test]
fn bound_compression() {
    let res = translate(&get_dates_from_bitset(
        Date::from_ymd(2012, 7, 2),
        &format!("{}{}{}", "0000111", "0001111", "0001110"),
    ));

    assert_eq!(get_week_from_weekday(res.operating_days), 0b0001111);
    assert!(res.exceptions.is_empty());
    assert_eq!(
        res.validity_periods.iter().next().unwrap(),
        &ValidityPeriod {
            start_date: Date::from_ymd(2012, 7, 6),
            end_date: Date::from_ymd(2012, 7, 21),
        }
    )
}

// ROADEF 2015 example
#[test]
fn may2015() {
    let res = translate(&get_dates_from_bitset(
        Date::from_ymd(2015, 4, 27),
        &format!(
            "{}{}{}{}{}",
            "1111000", "1111000", "1110100", "1111100", "0111110"
        ),
    ));

    assert_eq!(get_week_from_weekday(res.operating_days), 0b1111100);
    assert_eq!(res.exceptions.len(), 5);
    assert_eq!(
        res.exceptions[0],
        ExceptionDate {
            date: Date::from_ymd(2015, 5, 1),
            exception_type: ExceptionType::Remove,
        }
    );
    assert_eq!(
        res.exceptions[1],
        ExceptionDate {
            date: Date::from_ymd(2015, 5, 8),
            exception_type: ExceptionType::Remove,
        }
    );
    assert_eq!(
        res.exceptions[2],
        ExceptionDate {
            date: Date::from_ymd(2015, 5, 14),
            exception_type: ExceptionType::Remove,
        }
    );
    assert_eq!(
        res.exceptions[3],
        ExceptionDate {
            date: Date::from_ymd(2015, 5, 30),
            exception_type: ExceptionType::Add,
        }
    );
    assert_eq!(
        res.exceptions[4],
        ExceptionDate {
            date: Date::from_ymd(2015, 5, 25),
            exception_type: ExceptionType::Remove,
        }
    );
    assert_eq!(
        res.validity_periods.iter().next().unwrap(),
        &ValidityPeriod {
            start_date: Date::from_ymd(2015, 4, 27),
            end_date: Date::from_ymd(2015, 5, 30),
        }
    )
}
