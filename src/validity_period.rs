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

//! Some utilities to set and/or calculate validity periods.
use crate::{
    objects::{Calendar, Dataset, ValidityPeriod},
    Result,
};
use std::collections::BTreeSet;
use typed_index_collection::CollectionWithId;

fn get_validity_period(calendars: &CollectionWithId<Calendar>) -> Option<ValidityPeriod> {
    let dates = calendars.values().fold(BTreeSet::new(), |acc, c| {
        acc.union(&c.dates).cloned().collect()
    });

    if dates.is_empty() {
        return None;
    }

    Some(ValidityPeriod {
        start_date: *dates.iter().next().unwrap(),
        end_date: *dates.iter().next_back().unwrap(),
    })
}

/// Define the Validity Period of the dataset from all the available services.
pub fn compute_dataset_validity_period(
    dataset: &mut Dataset,
    calendars: &CollectionWithId<Calendar>,
) -> Result<()> {
    let validity_period = get_validity_period(calendars);

    if let Some(vp) = validity_period {
        dataset.start_date = vp.start_date;
        dataset.end_date = vp.end_date;
    }

    Ok(())
}

/// Set the validity period of a dataset.
///
/// Take also a look at the `compute_dataset_validity_period` function that
/// can automatically calculate the validity period from the Services dates.
pub fn set_dataset_validity_period(
    dataset: &mut Dataset,
    service_validity_period: &ValidityPeriod,
) {
    dataset.start_date = if service_validity_period.start_date < dataset.start_date {
        service_validity_period.start_date
    } else {
        dataset.start_date
    };
    dataset.end_date = if service_validity_period.end_date > dataset.end_date {
        service_validity_period.end_date
    } else {
        dataset.end_date
    };
}

#[cfg(test)]
mod tests {

    mod set_validity_period {
        use super::super::*;
        use crate::objects::{Dataset, Date, ValidityPeriod};
        use chrono::naive::{MAX_DATE, MIN_DATE};
        use pretty_assertions::assert_eq;

        #[test]
        fn no_existing_validity_period() {
            let start_date = Date::from_ymd(2019, 1, 1);
            let end_date = Date::from_ymd(2019, 6, 30);
            let mut dataset = Dataset {
                id: String::from("dataset_id"),
                contributor_id: String::from("contributor_id"),
                start_date: MAX_DATE,
                end_date: MIN_DATE,
                ..Default::default()
            };
            let service_validity_period = ValidityPeriod {
                start_date,
                end_date,
            };
            set_dataset_validity_period(&mut dataset, &service_validity_period);
            assert_eq!(start_date, dataset.start_date);
            assert_eq!(end_date, dataset.end_date);
        }

        #[test]
        fn with_extended_validity_period() {
            let start_date = Date::from_ymd(2019, 1, 1);
            let end_date = Date::from_ymd(2019, 6, 30);
            let mut dataset = Dataset {
                id: String::from("dataset_id"),
                contributor_id: String::from("contributor_id"),
                start_date: Date::from_ymd(2019, 3, 1),
                end_date: Date::from_ymd(2019, 4, 30),
                ..Default::default()
            };
            let service_validity_period = ValidityPeriod {
                start_date,
                end_date,
            };
            set_dataset_validity_period(&mut dataset, &service_validity_period);
            assert_eq!(start_date, dataset.start_date);
            assert_eq!(end_date, dataset.end_date);
        }

        #[test]
        fn with_included_validity_period() {
            let start_date = Date::from_ymd(2019, 1, 1);
            let end_date = Date::from_ymd(2019, 6, 30);
            let mut dataset = Dataset {
                id: String::from("dataset_id"),
                contributor_id: String::from("contributor_id"),
                start_date,
                end_date,
                ..Default::default()
            };
            let service_validity_period = ValidityPeriod {
                start_date: Date::from_ymd(2019, 3, 1),
                end_date: Date::from_ymd(2019, 4, 30),
            };
            set_dataset_validity_period(&mut dataset, &service_validity_period);
            assert_eq!(start_date, dataset.start_date);
            assert_eq!(end_date, dataset.end_date);
        }
    }

    mod compute_dataset_validity_period {
        use super::super::*;
        use crate::{
            calendars, configuration::*, file_handler::PathFileHandler, model::Collections,
            test_utils::*,
        };

        #[test]
        fn test_compute_dataset_validity_period() {
            let calendars_content = "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
                                 1,1,1,1,1,1,0,0,20180501,20180508\n\
                                 2,0,0,0,0,0,1,1,20180514,20180520";

            let calendar_dates_content = "service_id,date,exception_type\n\
                                      2,20180520,2";

            test_in_tmp_dir(|path| {
                let mut handler = PathFileHandler::new(path.to_path_buf());
                create_file_with_content(path, "calendar.txt", calendars_content);
                create_file_with_content(path, "calendar_dates.txt", calendar_dates_content);

                let mut collections = Collections::default();
                let (_, mut dataset, _) = read_config(None::<&str>).unwrap();

                calendars::manage_calendars(&mut handler, &mut collections).unwrap();
                compute_dataset_validity_period(&mut dataset, &collections.calendars).unwrap();

                assert_eq!(
                    Dataset {
                        id: "default_dataset".to_string(),
                        contributor_id: "default_contributor".to_string(),
                        start_date: chrono::NaiveDate::from_ymd(2018, 5, 1),
                        end_date: chrono::NaiveDate::from_ymd(2018, 5, 19),
                        dataset_type: None,
                        extrapolation: false,
                        desc: None,
                        system: None,
                    },
                    dataset
                );
            });
        }

        #[test]
        fn test_compute_dataset_validity_period_with_only_one_date() {
            let calendars_content = "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
                                 1,1,1,1,1,1,0,0,20180501,20180501";

            test_in_tmp_dir(|path| {
                let mut handler = PathFileHandler::new(path.to_path_buf());
                create_file_with_content(path, "calendar.txt", calendars_content);

                let mut collections = Collections::default();
                let (_, mut dataset, _) = read_config(None::<&str>).unwrap();

                calendars::manage_calendars(&mut handler, &mut collections).unwrap();
                compute_dataset_validity_period(&mut dataset, &collections.calendars).unwrap();

                assert_eq!(
                    Dataset {
                        id: "default_dataset".to_string(),
                        contributor_id: "default_contributor".to_string(),
                        start_date: chrono::NaiveDate::from_ymd(2018, 5, 1),
                        end_date: chrono::NaiveDate::from_ymd(2018, 5, 1),
                        dataset_type: None,
                        extrapolation: false,
                        desc: None,
                        system: None,
                    },
                    dataset
                );
            });
        }
    }
}
