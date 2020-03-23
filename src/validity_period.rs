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
}
