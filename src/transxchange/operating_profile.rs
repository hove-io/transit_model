// SPDX-License-Identifier: AGPL-3.0-only
//
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

use crate::{
    minidom_utils::TryOnlyChild,
    objects::{Date, ValidityPeriod},
    transxchange::bank_holidays::BankHoliday,
};
use chrono::{Datelike, Weekday};
use log::warn;
use minidom::Element;
use std::{
    collections::{HashMap, HashSet},
    convert::From,
};

#[derive(Debug, Default)]
struct IncludeExclude<T>
where
    T: Default,
{
    include: T,
    exclude: T,
}

pub struct OperatingProfile {
    week_pattern: HashSet<Weekday>,
    bank_holidays: IncludeExclude<HashSet<BankHoliday>>,
}
type BankHolidays = HashMap<BankHoliday, Vec<Date>>;

impl OperatingProfile {
    fn regular_days(days_of_week: &Element) -> HashSet<Weekday> {
        let mut regular_days = HashSet::new();
        use chrono::Weekday::*;
        if days_of_week.children().count() == 0 {
            regular_days.insert(Mon);
            regular_days.insert(Tue);
            regular_days.insert(Wed);
            regular_days.insert(Thu);
            regular_days.insert(Fri);
            regular_days.insert(Sat);
            regular_days.insert(Sun);
        } else {
            for element in days_of_week.children() {
                match element.name() {
                    "Monday" => {
                        regular_days.insert(Mon);
                    }
                    "Tuesday" => {
                        regular_days.insert(Tue);
                    }
                    "Wednesday" => {
                        regular_days.insert(Wed);
                    }
                    "Thursday" => {
                        regular_days.insert(Thu);
                    }
                    "Friday" => {
                        regular_days.insert(Fri);
                    }
                    "Saturday" => {
                        regular_days.insert(Sat);
                    }
                    "Sunday" => {
                        regular_days.insert(Sun);
                    }
                    "MondayToFriday" => {
                        regular_days.insert(Mon);
                        regular_days.insert(Tue);
                        regular_days.insert(Wed);
                        regular_days.insert(Thu);
                        regular_days.insert(Fri);
                    }
                    "MondayToSaturday" => {
                        regular_days.insert(Mon);
                        regular_days.insert(Tue);
                        regular_days.insert(Wed);
                        regular_days.insert(Thu);
                        regular_days.insert(Fri);
                        regular_days.insert(Sat);
                    }
                    "MondayToSunday" => {
                        regular_days.insert(Mon);
                        regular_days.insert(Tue);
                        regular_days.insert(Wed);
                        regular_days.insert(Thu);
                        regular_days.insert(Fri);
                        regular_days.insert(Sat);
                        regular_days.insert(Sun);
                    }
                    "NotSaturday" => {
                        regular_days.insert(Mon);
                        regular_days.insert(Tue);
                        regular_days.insert(Wed);
                        regular_days.insert(Thu);
                        regular_days.insert(Fri);
                        regular_days.insert(Sun);
                    }
                    "Weekend" => {
                        regular_days.insert(Sat);
                        regular_days.insert(Sun);
                    }
                    unknown_tag => warn!("Tag '{}' is not a valid tag for DaysOfWeek", unknown_tag),
                };
            }
        }
        regular_days
    }

    fn bank_holidays(days_operation: &Element) -> HashSet<BankHoliday> {
        let mut bank_holidays = HashSet::new();
        for element in days_operation.children() {
            use crate::transxchange::bank_holidays::BankHoliday::*;
            match element.name() {
                "AllBankHolidays" => {
                    bank_holidays.insert(NewYear);
                    bank_holidays.insert(JanuarySecond);
                    bank_holidays.insert(GoodFriday);
                    bank_holidays.insert(SaintAndrews);
                    bank_holidays.insert(EasterMonday);
                    bank_holidays.insert(EarlyMay);
                    bank_holidays.insert(Spring);
                    bank_holidays.insert(Summer);
                    bank_holidays.insert(Christmas);
                    bank_holidays.insert(BoxingDay);
                    bank_holidays.insert(NewYearHoliday);
                    bank_holidays.insert(JanuarySecondHoliday);
                    bank_holidays.insert(SaintAndrewsHoliday);
                    bank_holidays.insert(ChristmasHoliday);
                    bank_holidays.insert(BoxingDayHoliday);
                }
                "EarlyRunOff" => {
                    bank_holidays.insert(ChristmasEve);
                    bank_holidays.insert(NewYearEve);
                }
                "AllHolidaysExceptChristmas" => {
                    bank_holidays.insert(NewYear);
                    bank_holidays.insert(JanuarySecond);
                    bank_holidays.insert(GoodFriday);
                    bank_holidays.insert(SaintAndrews);
                    bank_holidays.insert(EasterMonday);
                    bank_holidays.insert(EarlyMay);
                    bank_holidays.insert(Spring);
                    bank_holidays.insert(Summer);
                }
                "Holidays" => {
                    bank_holidays.insert(NewYear);
                    bank_holidays.insert(JanuarySecond);
                    bank_holidays.insert(GoodFriday);
                    bank_holidays.insert(SaintAndrews);
                }
                "HolidayMondays" => {
                    bank_holidays.insert(EasterMonday);
                    bank_holidays.insert(EarlyMay);
                    bank_holidays.insert(Spring);
                    bank_holidays.insert(Summer);
                }
                "Christmas" => {
                    bank_holidays.insert(Christmas);
                    bank_holidays.insert(BoxingDay);
                }
                "DisplacementHolidays" => {
                    bank_holidays.insert(NewYearHoliday);
                    bank_holidays.insert(JanuarySecondHoliday);
                    bank_holidays.insert(SaintAndrewsHoliday);
                    bank_holidays.insert(ChristmasHoliday);
                    bank_holidays.insert(BoxingDayHoliday);
                }
                "NewYearsDay" => {
                    bank_holidays.insert(NewYear);
                }
                "Jan2ndScotland" => {
                    bank_holidays.insert(JanuarySecond);
                }
                "GoodFriday" => {
                    bank_holidays.insert(GoodFriday);
                }
                "StAndrewsDay" => {
                    bank_holidays.insert(SaintAndrews);
                }
                "EasterMonday" => {
                    bank_holidays.insert(EasterMonday);
                }
                "MayDay" => {
                    bank_holidays.insert(EarlyMay);
                }
                "SpringBank" => {
                    bank_holidays.insert(Spring);
                }
                "AugustBankHolidayScotland" | "LateSummerBankHolidayNotScotland" => {
                    bank_holidays.insert(Summer);
                }
                "ChristmasDay" => {
                    bank_holidays.insert(Christmas);
                }
                "BoxingDay" => {
                    bank_holidays.insert(BoxingDay);
                }
                "NewYearsDayHoliday" => {
                    bank_holidays.insert(NewYearHoliday);
                }
                "Jan2ndScotlandHoliday" => {
                    bank_holidays.insert(JanuarySecondHoliday);
                }
                "StAndrewsDayHoliday" => {
                    bank_holidays.insert(SaintAndrewsHoliday);
                }
                "ChristmasDayHoliday" => {
                    bank_holidays.insert(ChristmasHoliday);
                }
                "BoxingDayHoliday" => {
                    bank_holidays.insert(BoxingDayHoliday);
                }
                unknown_tag => warn!(
                    "Tag '{}' is not a valid tag BankHolidayOperation",
                    unknown_tag
                ),
            }
        }
        bank_holidays
    }
}

impl From<&Element> for OperatingProfile {
    fn from(operating_profile: &Element) -> Self {
        let week_pattern = operating_profile
            .try_only_child("RegularDayType")
            .and_then(|regular_day_type| regular_day_type.try_only_child("DaysOfWeek"))
            .map(|days_of_week| OperatingProfile::regular_days(days_of_week))
            .unwrap_or_default();
        let bank_holidays = operating_profile
            .try_only_child("BankHolidayOperation")
            .and_then(|bank_holiday_operation| {
                let include = bank_holiday_operation
                    .try_only_child("DaysOfOperation")
                    .map(OperatingProfile::bank_holidays)
                    .unwrap_or_default();
                let exclude = bank_holiday_operation
                    .try_only_child("DaysOfNonOperation")
                    .map(OperatingProfile::bank_holidays)
                    .unwrap_or_default();
                Ok(IncludeExclude { include, exclude })
            })
            .unwrap_or_default();
        Self {
            week_pattern,
            bank_holidays,
        }
    }
}

pub struct ValidityPatternIterator<'a> {
    operating_profile: &'a OperatingProfile,
    bank_holidays_dates: IncludeExclude<HashSet<Date>>,
    validity_period: &'a ValidityPeriod,
    current_date: Date,
}

impl OperatingProfile {
    pub fn iter_with_bank_holidays_between<'a>(
        &'a self,
        bank_holidays: &'a BankHolidays,
        validity_period: &'a ValidityPeriod,
    ) -> ValidityPatternIterator<'a> {
        let filter_dates = |list_bank_holidays: &HashSet<BankHoliday>| {
            list_bank_holidays
                .iter()
                .flat_map(|bank_holiday| bank_holidays.get(&bank_holiday))
                .flatten()
                .filter(|date| {
                    **date >= validity_period.start_date && **date <= validity_period.end_date
                })
                .cloned()
                .collect()
        };
        let include_bank_holidays_dates: HashSet<Date> = filter_dates(&self.bank_holidays.include);
        let exclude_bank_holidays_dates: HashSet<Date> = filter_dates(&self.bank_holidays.exclude);
        ValidityPatternIterator {
            operating_profile: self,
            bank_holidays_dates: IncludeExclude {
                include: include_bank_holidays_dates,
                exclude: exclude_bank_holidays_dates,
            },
            validity_period,
            current_date: validity_period.start_date.pred(),
        }
    }
}

impl Iterator for ValidityPatternIterator<'_> {
    type Item = Date;
    fn next(&mut self) -> Option<Self::Item> {
        self.current_date = self.current_date.succ();
        if self.current_date > self.validity_period.end_date {
            return None;
        }
        let is_included = self
            .operating_profile
            .week_pattern
            .contains(&self.current_date.weekday());
        let is_included = if is_included {
            // Check if it's excluded as a Bank Holiday
            !self
                .bank_holidays_dates
                .exclude
                .contains(&self.current_date)
        } else {
            // Check if it's included as a Bank holiday
            self.bank_holidays_dates
                .include
                .contains(&self.current_date)
        };
        if is_included {
            Some(self.current_date)
        } else {
            self.next()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod operating_profile {
        use super::*;

        mod regular_days {
            use super::*;
            use chrono::Weekday::*;

            #[test]
            fn work_week() {
                let xml = r#"<root>
                        <MondayToFriday />
                        <UnknownTag />
                    </root>"#;
                let root: Element = xml.parse().unwrap();
                let regular_days = OperatingProfile::regular_days(&root);
                assert!(regular_days.contains(&Mon));
                assert!(regular_days.contains(&Tue));
                assert!(regular_days.contains(&Wed));
                assert!(regular_days.contains(&Thu));
                assert!(regular_days.contains(&Fri));
            }

            #[test]
            fn default() {
                let xml = r#"<root />"#;
                let root: Element = xml.parse().unwrap();
                let regular_days = OperatingProfile::regular_days(&root);
                assert!(regular_days.contains(&Mon));
                assert!(regular_days.contains(&Tue));
                assert!(regular_days.contains(&Wed));
                assert!(regular_days.contains(&Thu));
                assert!(regular_days.contains(&Fri));
            }
        }

        mod bank_holidays {
            use super::*;
            use crate::transxchange::bank_holidays::BankHoliday::*;

            #[test]
            fn christmas_and_displacement_holidays() {
                let xml = r#"<root>
                        <Christmas />
                        <DisplacementHolidays />
                        <UnknownTag />
                    </root>"#;
                let root: Element = xml.parse().unwrap();
                let bank_holidays = OperatingProfile::bank_holidays(&root);
                assert!(bank_holidays.contains(&Christmas));
                assert!(bank_holidays.contains(&BoxingDay));
                assert!(bank_holidays.contains(&NewYearHoliday));
                assert!(bank_holidays.contains(&JanuarySecondHoliday));
                assert!(bank_holidays.contains(&SaintAndrewsHoliday));
                assert!(bank_holidays.contains(&ChristmasHoliday));
                assert!(bank_holidays.contains(&BoxingDayHoliday));
            }
        }

        mod from {
            use super::*;
            use crate::transxchange::bank_holidays::BankHoliday::*;
            use chrono::Weekday::*;

            #[test]
            fn regular_day_type() {
                let xml = r#"<root>
                        <RegularDayType>
                            <DaysOfWeek>
                                <Weekend />
                            </DaysOfWeek>
                        </RegularDayType>
                    </root>"#;
                let root: Element = xml.parse().unwrap();
                let operating_profile = OperatingProfile::from(&root);
                assert!(operating_profile.week_pattern.contains(&Sat));
                assert!(operating_profile.week_pattern.contains(&Sun));
                assert!(operating_profile.bank_holidays.include.is_empty());
                assert!(operating_profile.bank_holidays.exclude.is_empty());
            }

            #[test]
            fn with_days_of_operation() {
                let xml = r#"<root>
                        <RegularDayType>
                            <DaysOfWeek>
                                <Weekend />
                            </DaysOfWeek>
                        </RegularDayType>
                        <BankHolidayOperation>
                            <DaysOfOperation>
                                <EasterMonday />
                            </DaysOfOperation>
                        </BankHolidayOperation>
                    </root>"#;
                let root: Element = xml.parse().unwrap();
                let operating_profile = OperatingProfile::from(&root);
                assert!(operating_profile.week_pattern.contains(&Sat));
                assert!(operating_profile.week_pattern.contains(&Sun));
                assert!(operating_profile
                    .bank_holidays
                    .include
                    .contains(&EasterMonday));
                assert!(operating_profile.bank_holidays.exclude.is_empty());
            }

            #[test]
            fn with_days_of_non_operation() {
                let xml = r#"<root>
                        <RegularDayType>
                            <DaysOfWeek>
                                <Weekend />
                            </DaysOfWeek>
                        </RegularDayType>
                        <BankHolidayOperation>
                            <DaysOfNonOperation>
                                <Jan2ndScotland />
                            </DaysOfNonOperation>
                        </BankHolidayOperation>
                    </root>"#;
                let root: Element = xml.parse().unwrap();
                let operating_profile = OperatingProfile::from(&root);
                assert!(operating_profile.week_pattern.contains(&Sat));
                assert!(operating_profile.week_pattern.contains(&Sun));
                assert!(operating_profile.bank_holidays.include.is_empty());
                assert!(operating_profile
                    .bank_holidays
                    .exclude
                    .contains(&JanuarySecond));
            }
        }
    }

    mod validity_pattern_iterator {
        use super::*;
        use crate::transxchange::bank_holidays::BankHoliday::*;
        use chrono::Weekday::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn weekend() {
            let mut week_pattern = HashSet::new();
            week_pattern.insert(Sat);
            week_pattern.insert(Sun);
            let include = HashSet::new();
            let exclude = HashSet::new();
            let bank_holidays = IncludeExclude { include, exclude };
            let operating_profile = OperatingProfile {
                week_pattern,
                bank_holidays,
            };
            let bank_holidays = HashMap::new();
            let validity_period = ValidityPeriod {
                start_date: Date::from_ymd(2019, 1, 1),
                end_date: Date::from_ymd(2019, 1, 8),
            };
            let dates: Vec<Date> = operating_profile
                .iter_with_bank_holidays_between(&bank_holidays, &validity_period)
                .collect();
            assert_eq!(Date::from_ymd(2019, 1, 5), dates[0]);
            assert_eq!(Date::from_ymd(2019, 1, 6), dates[1]);
        }

        #[test]
        fn with_bank_holiday() {
            let mut week_pattern = HashSet::new();
            week_pattern.insert(Sat);
            week_pattern.insert(Sun);
            let mut include = HashSet::new();
            include.insert(NewYear);
            include.insert(NewYearHoliday);
            let exclude = HashSet::new();
            let bank_holidays = IncludeExclude { include, exclude };
            let operating_profile = OperatingProfile {
                week_pattern,
                bank_holidays,
            };
            let mut bank_holidays = HashMap::new();
            bank_holidays.insert(NewYear, vec![Date::from_ymd(2017, 1, 1)]);
            bank_holidays.insert(NewYearHoliday, vec![Date::from_ymd(2017, 1, 2)]);
            let validity_period = ValidityPeriod {
                start_date: Date::from_ymd(2017, 1, 1),
                end_date: Date::from_ymd(2017, 1, 8),
            };
            let dates: Vec<Date> = operating_profile
                .iter_with_bank_holidays_between(&bank_holidays, &validity_period)
                .collect();
            assert_eq!(Date::from_ymd(2017, 1, 1), dates[0]);
            assert_eq!(Date::from_ymd(2017, 1, 2), dates[1]);
            assert_eq!(Date::from_ymd(2017, 1, 7), dates[2]);
            assert_eq!(Date::from_ymd(2017, 1, 8), dates[3]);
        }

        #[test]
        fn without_bank_holiday() {
            let mut week_pattern = HashSet::new();
            week_pattern.insert(Mon);
            week_pattern.insert(Tue);
            week_pattern.insert(Wed);
            week_pattern.insert(Thu);
            week_pattern.insert(Fri);
            let include = HashSet::new();
            let mut exclude = HashSet::new();
            exclude.insert(NewYear);
            let bank_holidays = IncludeExclude { include, exclude };
            let operating_profile = OperatingProfile {
                week_pattern,
                bank_holidays,
            };
            let mut bank_holidays = HashMap::new();
            bank_holidays.insert(NewYear, vec![Date::from_ymd(2018, 1, 1)]);
            let validity_period = ValidityPeriod {
                start_date: Date::from_ymd(2018, 1, 1),
                end_date: Date::from_ymd(2018, 1, 8),
            };
            let dates: Vec<Date> = operating_profile
                .iter_with_bank_holidays_between(&bank_holidays, &validity_period)
                .collect();
            assert_eq!(Date::from_ymd(2018, 1, 2), dates[0]);
            assert_eq!(Date::from_ymd(2018, 1, 3), dates[1]);
            assert_eq!(Date::from_ymd(2018, 1, 4), dates[2]);
            assert_eq!(Date::from_ymd(2018, 1, 5), dates[3]);
        }
    }
}
