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

//! Module to handle Bank Holidays in UK
//! The data structure is based on the JSON provided by the UK government at
//! https://www.gov.uk/bank-holidays.json

use crate::{
    objects::{Date, ValidityPeriod},
    Result,
};
use chrono::Datelike;
use serde::Deserialize;
use std::{collections::HashMap, fs::File, path::Path};

#[derive(Debug, Deserialize)]
pub struct BankHolidayRegion {
    events: Vec<BankHolidayEvent>,
}

pub fn date_from_string<'de, D>(deserializer: D) -> std::result::Result<Date, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    Date::parse_from_str(&s, "%Y-%m-%d").map_err(serde::de::Error::custom)
}

pub fn bank_holiday_from_string<'de, D>(
    deserializer: D,
) -> std::result::Result<BankHoliday, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let title = String::deserialize(deserializer)?;
    use BankHoliday::*;
    // All of the following are equivalent and should be `BankHoliday::EarlyMay`
    // - "Early May bank holiday"
    // - "Early May bank holiday (VE Day)"
    // Therefore, we trim anything that is in parenthesis at the end
    let parenthesis_offset = title.find('(').unwrap_or_else(|| title.len());
    let day = match title[0..parenthesis_offset].trim() {
        "New Year’s Day" => NewYearHoliday,
        "2nd January" => JanuarySecondHoliday,
        "St Patrick’s Day" => SaintPatrick,
        "Good Friday" => GoodFriday,
        "Easter Monday" => EasterMonday,
        "Early May bank holiday" => EarlyMay,
        "Spring bank holiday" => Spring,
        "Queen’s Diamond Jubilee" => QueensDiamondJubilee,
        "Battle of the Boyne" => BattleOfTheBoyne,
        "Summer bank holiday" => Summer,
        "St Andrew’s Day" => SaintAndrewsHoliday,
        "Christmas Day" => ChristmasHoliday,
        "Boxing Day" => BoxingDayHoliday,
        title => {
            return Err(serde::de::Error::custom(format!(
                "Failed to match '{}' with a known bank holiday",
                title
            )))
        }
    };
    Ok(day)
}

#[derive(Debug, Deserialize)]
struct BankHolidayEvent {
    #[serde(deserialize_with = "bank_holiday_from_string")]
    title: BankHoliday,
    #[serde(deserialize_with = "date_from_string")]
    date: Date,
}

#[derive(Clone, Debug, Ord, PartialOrd, PartialEq, Eq, Hash)]
pub enum BankHoliday {
    NewYear,
    // Bank Holiday for New Year, not necessarily on the 1st of January
    NewYearHoliday,
    JanuarySecond,
    // Bank Holiday for January Second, not necessarily on the 2nd of January
    JanuarySecondHoliday,
    SaintPatrick,
    GoodFriday,
    EasterMonday,
    EarlyMay,
    Spring,
    QueensDiamondJubilee,
    BattleOfTheBoyne,
    Summer,
    SaintAndrews,
    // Bank Holiday for Saint Andrews, not necessarily on the 30th of November
    SaintAndrewsHoliday,
    ChristmasEve,
    Christmas,
    // Bank Holiday for Christmas, not necessarily on the 25th of December
    ChristmasHoliday,
    BoxingDay,
    // Bank Holiday for Boxing Day, not necessarily on the 26th of December
    BoxingDayHoliday,
    NewYearEve,
}

pub fn get_bank_holiday<P: AsRef<Path>>(
    bank_holiday_path: P,
) -> Result<HashMap<BankHoliday, Vec<Date>>> {
    let bank_holidays_file = File::open(bank_holiday_path)?;
    let region: BankHolidayRegion = serde_json::from_reader(bank_holidays_file)?;
    let mut day_per_bank_holiday: HashMap<BankHoliday, Vec<Date>> = HashMap::new();
    for event in region.events {
        day_per_bank_holiday
            .entry(event.title)
            .or_insert_with(Vec::new)
            .push(event.date);
    }
    Ok(day_per_bank_holiday)
}

// Generate a list of all fixed dates between two dates.
// For example, let's say you want to generate all the Christmas dates between
// the 1st of January 2000 and the 31st December of 2020
// ```
// let validity_period = ValidityPeriod {
//   start_date: NaiveDate::from_ymd(2000, 1, 1),
//   end_date: NaiveDate::from_ymd(2020, 12, 31),
// };
// let dates = get_fixed_days(25, 12, &validity_period);
// for year in 2000..=2020 {
//   let date = NaiveDate::from_ymd(year, 12, 25);
//   assert!(dates.contains(&date));
// }
// ```
pub fn get_fixed_days(day: u32, month: u32, validity_period: &ValidityPeriod) -> Vec<Date> {
    let start_year = if Date::from_ymd(validity_period.start_date.year(), month, day)
        >= validity_period.start_date
    {
        validity_period.start_date.year()
    } else {
        validity_period.start_date.year() + 1
    };
    let end_year = if Date::from_ymd(validity_period.end_date.year(), month, day)
        <= validity_period.end_date
    {
        validity_period.end_date.year()
    } else {
        validity_period.end_date.year() - 1
    };
    (start_year..=end_year)
        .map(|year| Date::from_ymd(year, month, day))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    mod fixed_years {
        use super::*;
        use chrono::NaiveDate;

        #[test]
        fn included_limits() {
            let validity_period = ValidityPeriod {
                start_date: NaiveDate::from_ymd(2000, 12, 25),
                end_date: NaiveDate::from_ymd(2002, 12, 25),
            };
            let dates = get_fixed_days(25, 12, &validity_period);
            let date = NaiveDate::from_ymd(2000, 12, 25);
            assert!(dates.contains(&date));
            let date = NaiveDate::from_ymd(2001, 12, 25);
            assert!(dates.contains(&date));
            let date = NaiveDate::from_ymd(2002, 12, 25);
            assert!(dates.contains(&date));
        }

        #[test]
        fn excluded_limits() {
            let validity_period = ValidityPeriod {
                start_date: NaiveDate::from_ymd(2000, 12, 26),
                end_date: NaiveDate::from_ymd(2002, 12, 24),
            };
            let dates = get_fixed_days(25, 12, &validity_period);
            let date = NaiveDate::from_ymd(2001, 12, 25);
            assert!(dates.contains(&date));
        }
    }
}
