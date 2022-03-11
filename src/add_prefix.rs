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

//! A trait for every structure that needs to be updated with a prefix

use crate::model::Collections;
use derivative::Derivative;
use std::collections::HashMap;
use typed_index_collection::{Collection, CollectionWithId, Id};

/// Metadata for building the prefix.
#[derive(Derivative, Debug)]
#[derivative(Default)]
pub struct PrefixConfiguration {
    /// Separator used in the prefix, usually ':'.
    #[derivative(Default(value = "\":\".to_string()"))]
    sep: String,
    /// General data prefix (historically a trigram) used for discriminating
    /// referential objects (like Network).  Usually useful to avoid collisions
    /// when merging dataset from different contributors.
    data_prefix: Option<String>,
    /// Sub prefix used for discriminating scheduled
    /// objects (like Calendar).  Usually useful to avoid collisions when
    /// merging datasets from the same contributor.
    schedule_subprefix: Option<String>,
}

impl PrefixConfiguration {
    /// Set the prefix separator for PrefixConfiguration.
    pub fn set_sep<S>(&mut self, sep: S)
    where
        S: ToString,
    {
        self.sep = sep.to_string();
    }

    /// Set the data_prefix in the PrefixConfiguration.
    pub fn set_data_prefix<S>(&mut self, data_prefix: S)
    where
        S: ToString,
    {
        self.data_prefix = Some(data_prefix.to_string());
    }

    /// Set the schedule_subprefix in the PrefixConfiguration.
    pub fn set_schedule_subprefix<S>(&mut self, schedule_subprefix: S)
    where
        S: ToString,
    {
        self.schedule_subprefix = Some(schedule_subprefix.to_string());
    }

    /// Add prefix for referential-type object.
    ///
    /// Example of objects from the referential are Line or StopPoint.
    pub fn referential_prefix(&self, id: &str) -> String {
        let mut prefix = String::new();
        if let Some(data_prefix) = self.data_prefix.as_ref() {
            prefix = prefix + data_prefix + &self.sep;
        }
        prefix + id
    }

    /// Add prefix for schedule-type object.
    ///
    /// Example of objects from the schedule are VehicleJourney or StopTime.
    pub fn schedule_prefix(&self, id: &str) -> String {
        let mut prefix = String::new();
        if let Some(data_prefix) = self.data_prefix.as_ref() {
            prefix = prefix + data_prefix + &self.sep;
        }
        if let Some(schedule_subprefix) = self.schedule_subprefix.as_ref() {
            prefix = prefix + schedule_subprefix + &self.sep;
        }
        prefix + id
    }
}

/// Trait for object that can be prefixed
pub trait AddPrefix {
    /// Add the prefix to all elements of the object that needs to be prefixed.
    #[deprecated(since = "0.24.0", note = "please use `AddPrefix::prefix()` instead")]
    fn add_prefix(&mut self, prefix: &str) {
        let prefix_conf = PrefixConfiguration {
            sep: String::new(),
            data_prefix: Some(prefix.to_string()),
            schedule_subprefix: None,
        };
        self.prefix(&prefix_conf);
    }

    /// Add the prefix to all elements of the object that needs to be prefixed.
    /// A separator will be placed between the prefix and the identifier.
    #[deprecated(since = "0.24.0", note = "please use `AddPrefix::prefix()` instead")]
    fn add_prefix_with_sep(&mut self, prefix: &str, sep: &str) {
        let prefix_conf = PrefixConfiguration {
            sep: String::from(sep),
            data_prefix: Some(prefix.to_string()),
            schedule_subprefix: None,
        };
        self.prefix(&prefix_conf);
    }

    /// Add the prefix to all elements of the object that needs to be prefixed.
    /// PrefixConfiguration contains all the needed metadata to create the
    /// complete prefix.
    fn prefix(&mut self, prefix_conf: &PrefixConfiguration);
}

impl<T> AddPrefix for Collection<T>
where
    T: AddPrefix,
{
    fn prefix(&mut self, prefix_conf: &PrefixConfiguration) {
        for obj in &mut self.values_mut() {
            obj.prefix(prefix_conf);
        }
    }
}

impl<T> AddPrefix for CollectionWithId<T>
where
    T: Id<T> + AddPrefix,
{
    fn prefix(&mut self, prefix_conf: &PrefixConfiguration) {
        for index in self.indexes() {
            self.index_mut(index).prefix(prefix_conf);
        }
    }
}

fn add_prefix_on_vehicle_journey_ids(
    vehicle_journey_ids: &HashMap<(String, u32), String>,
    prefix_conf: &PrefixConfiguration,
) -> HashMap<(String, u32), String> {
    vehicle_journey_ids
        .iter()
        .map(|((trip_id, sequence), value)| {
            (
                (prefix_conf.schedule_prefix(trip_id.as_str()), *sequence),
                value.to_string(),
            )
        })
        .collect()
}

fn add_prefix_on_vehicle_journey_ids_and_values(
    vehicle_journey_ids: &HashMap<(String, u32), String>,
    prefix_conf: &PrefixConfiguration,
) -> HashMap<(String, u32), String> {
    vehicle_journey_ids
        .iter()
        .map(|((trip_id, sequence), value)| {
            (
                (prefix_conf.schedule_prefix(trip_id.as_str()), *sequence),
                prefix_conf.schedule_prefix(value.as_str()),
            )
        })
        .collect()
}

impl AddPrefix for Collections {
    fn prefix(&mut self, prefix_conf: &PrefixConfiguration) {
        self.contributors.prefix(prefix_conf);
        self.datasets.prefix(prefix_conf);
        self.networks.prefix(prefix_conf);
        self.lines.prefix(prefix_conf);
        self.routes.prefix(prefix_conf);
        self.vehicle_journeys.prefix(prefix_conf);
        self.frequencies.prefix(prefix_conf);
        self.stop_areas.prefix(prefix_conf);
        self.stop_points.prefix(prefix_conf);
        self.stop_locations.prefix(prefix_conf);
        self.calendars.prefix(prefix_conf);
        self.companies.prefix(prefix_conf);
        self.comments.prefix(prefix_conf);
        self.equipments.prefix(prefix_conf);
        self.transfers.prefix(prefix_conf);
        self.trip_properties.prefix(prefix_conf);
        self.geometries.prefix(prefix_conf);
        self.admin_stations.prefix(prefix_conf);
        self.prices_v1.prefix(prefix_conf);
        self.od_fares_v1.prefix(prefix_conf);
        self.fares_v1.prefix(prefix_conf);
        self.tickets.prefix(prefix_conf);
        self.ticket_prices.prefix(prefix_conf);
        self.ticket_uses.prefix(prefix_conf);
        self.ticket_use_perimeters.prefix(prefix_conf);
        self.ticket_use_restrictions.prefix(prefix_conf);
        self.pathways.prefix(prefix_conf);
        self.levels.prefix(prefix_conf);
        self.grid_calendars.prefix(prefix_conf);
        self.grid_exception_dates.prefix(prefix_conf);
        self.grid_periods.prefix(prefix_conf);
        self.grid_rel_calendar_line.prefix(prefix_conf);
        self.stop_time_headsigns =
            add_prefix_on_vehicle_journey_ids(&self.stop_time_headsigns, prefix_conf);
        self.stop_time_ids =
            add_prefix_on_vehicle_journey_ids_and_values(&self.stop_time_ids, prefix_conf);
        self.stop_time_comments =
            add_prefix_on_vehicle_journey_ids_and_values(&self.stop_time_comments, prefix_conf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    struct Obj(String);
    impl Id<Obj> for Obj {
        fn id(&self) -> &str {
            self.0.as_str()
        }
        fn set_id(&mut self, _id: String) {
            unimplemented!()
        }
    }
    impl AddPrefix for Obj {
        fn prefix(&mut self, prefix_conf: &PrefixConfiguration) {
            self.0 = prefix_conf.schedule_prefix(self.0.as_str());
        }
    }

    #[test]
    fn collection_referential() {
        let obj1 = Obj(String::from("some_id"));
        let obj2 = Obj(String::from("other_id"));
        let mut collection = Collection::new(vec![obj1, obj2]);
        let mut prefix_conf = PrefixConfiguration::default();
        prefix_conf.set_data_prefix("pre");
        collection.prefix(&prefix_conf);
        let mut values = collection.values();
        let element = values.next().unwrap();
        assert_eq!(String::from("pre:some_id"), element.0);
        let element = values.next().unwrap();
        assert_eq!(String::from("pre:other_id"), element.0);
    }

    #[test]
    fn collection_referential_and_schedule() {
        let obj1 = Obj(String::from("some_id"));
        let obj2 = Obj(String::from("other_id"));
        let mut collection = Collection::new(vec![obj1, obj2]);
        let mut prefix_conf = PrefixConfiguration::default();
        prefix_conf.set_data_prefix("pre");
        prefix_conf.set_schedule_subprefix("winter");
        collection.prefix(&prefix_conf);
        let mut values = collection.values();
        let element = values.next().unwrap();
        assert_eq!(String::from("pre:winter:some_id"), element.0);
        let element = values.next().unwrap();
        assert_eq!(String::from("pre:winter:other_id"), element.0);
    }

    #[test]
    fn collection_schedule() {
        let obj1 = Obj(String::from("some_id"));
        let obj2 = Obj(String::from("other_id"));
        let mut collection = Collection::new(vec![obj1, obj2]);
        let mut prefix_conf = PrefixConfiguration::default();
        prefix_conf.set_schedule_subprefix("winter");
        collection.prefix(&prefix_conf);
        let mut values = collection.values();
        let element = values.next().unwrap();
        assert_eq!(String::from("winter:some_id"), element.0);
        let element = values.next().unwrap();
        assert_eq!(String::from("winter:other_id"), element.0);
    }

    #[test]
    fn collection_no_prefix() {
        let obj1 = Obj(String::from("some_id"));
        let obj2 = Obj(String::from("other_id"));
        let mut collection = Collection::new(vec![obj1, obj2]);
        let prefix_conf = PrefixConfiguration::default();
        collection.prefix(&prefix_conf);
        let mut values = collection.values();
        let element = values.next().unwrap();
        assert_eq!(String::from("some_id"), element.0);
        let element = values.next().unwrap();
        assert_eq!(String::from("other_id"), element.0);
    }

    #[test]
    #[allow(deprecated)]
    fn collection_deprecated() {
        let obj1 = Obj(String::from("some_id"));
        let obj2 = Obj(String::from("other_id"));
        let mut collection = Collection::new(vec![obj1, obj2]);
        collection.add_prefix("pre:");
        let mut values = collection.values();
        let element = values.next().unwrap();
        assert_eq!(String::from("pre:some_id"), element.0);
        let element = values.next().unwrap();
        assert_eq!(String::from("pre:other_id"), element.0);
    }

    #[test]
    fn collection_with_id_referential() {
        let obj1 = Obj(String::from("some_id"));
        let obj2 = Obj(String::from("other_id"));
        let mut collection = CollectionWithId::new(vec![obj1, obj2]).unwrap();
        let mut prefix_conf = PrefixConfiguration::default();
        prefix_conf.set_data_prefix("pre");
        collection.prefix(&prefix_conf);
        let mut values = collection.values();
        let element = values.next().unwrap();
        assert_eq!(String::from("pre:some_id"), element.0);
        let element = values.next().unwrap();
        assert_eq!(String::from("pre:other_id"), element.0);
    }

    #[test]
    fn collection_with_id_referential_and_schedule() {
        let obj1 = Obj(String::from("some_id"));
        let obj2 = Obj(String::from("other_id"));
        let mut collection = CollectionWithId::new(vec![obj1, obj2]).unwrap();
        let mut prefix_conf = PrefixConfiguration::default();
        prefix_conf.set_data_prefix("pre");
        prefix_conf.set_schedule_subprefix("summer");
        collection.prefix(&prefix_conf);
        let mut values = collection.values();
        let element = values.next().unwrap();
        assert_eq!(String::from("pre:summer:some_id"), element.0);
        let element = values.next().unwrap();
        assert_eq!(String::from("pre:summer:other_id"), element.0);
    }

    #[test]
    fn collection_with_id_schedule() {
        let obj1 = Obj(String::from("some_id"));
        let obj2 = Obj(String::from("other_id"));
        let mut collection = CollectionWithId::new(vec![obj1, obj2]).unwrap();
        let mut prefix_conf = PrefixConfiguration::default();
        prefix_conf.set_schedule_subprefix("summer");
        collection.prefix(&prefix_conf);
        let mut values = collection.values();
        let element = values.next().unwrap();
        assert_eq!(String::from("summer:some_id"), element.0);
        let element = values.next().unwrap();
        assert_eq!(String::from("summer:other_id"), element.0);
    }

    #[test]
    fn collection_with_id_no_prefix() {
        let obj1 = Obj(String::from("some_id"));
        let obj2 = Obj(String::from("other_id"));
        let mut collection = CollectionWithId::new(vec![obj1, obj2]).unwrap();
        let prefix_conf = PrefixConfiguration::default();
        collection.prefix(&prefix_conf);
        let mut values = collection.values();
        let element = values.next().unwrap();
        assert_eq!(String::from("some_id"), element.0);
        let element = values.next().unwrap();
        assert_eq!(String::from("other_id"), element.0);
    }

    #[test]
    #[allow(deprecated)]
    fn collection_with_id_deprecated() {
        let obj1 = Obj(String::from("some_id"));
        let obj2 = Obj(String::from("other_id"));
        let mut collection = CollectionWithId::new(vec![obj1, obj2]).unwrap();
        collection.add_prefix("pre:");
        let mut values = collection.values();
        let element = values.next().unwrap();
        assert_eq!(String::from("pre:some_id"), element.0);
        let element = values.next().unwrap();
        assert_eq!(String::from("pre:other_id"), element.0);
    }
}
