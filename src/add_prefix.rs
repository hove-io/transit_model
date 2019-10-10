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

//! A trait for every structure that needs to be updated with a prefix

use crate::model::Collections;
use transit_model_collection::{Collection, CollectionWithId, Id};

pub trait AddPrefix {
    fn add_prefix(&mut self, prefix: &str);
    fn add_prefix_with_sep(&mut self, prefix: &str, sep: &str) {
        let prefix = format!("{}{}", prefix, sep);
        self.add_prefix(&prefix);
    }
}

impl<T> AddPrefix for Collection<T>
where
    T: AddPrefix,
{
    fn add_prefix(&mut self, prefix: &str) {
        for obj in &mut self.values_mut() {
            obj.add_prefix(prefix);
        }
    }
}

impl<T> AddPrefix for CollectionWithId<T>
where
    T: Id<T> + AddPrefix,
{
    fn add_prefix(&mut self, prefix: &str) {
        let indexes: Vec<_> = self.iter().map(|(idx, _)| idx).collect();
        for index in indexes {
            self.index_mut(index).add_prefix(prefix);
        }
    }
}

impl AddPrefix for Collections {
    fn add_prefix(&mut self, prefix: &str) {
        self.contributors.add_prefix(&prefix);
        self.datasets.add_prefix(&prefix);
        self.networks.add_prefix(&prefix);
        self.lines.add_prefix(&prefix);
        self.routes.add_prefix(&prefix);
        self.vehicle_journeys.add_prefix(&prefix);
        self.frequencies.add_prefix(&prefix);
        self.stop_areas.add_prefix(&prefix);
        self.stop_points.add_prefix(&prefix);
        self.calendars.add_prefix(&prefix);
        self.companies.add_prefix(&prefix);
        self.comments.add_prefix(&prefix);
        self.equipments.add_prefix(&prefix);
        self.transfers.add_prefix(&prefix);
        self.trip_properties.add_prefix(&prefix);
        self.geometries.add_prefix(&prefix);
        self.admin_stations.add_prefix(&prefix);
        self.prices_v1.add_prefix(&prefix);
        self.od_fares_v1.add_prefix(&prefix);
        self.fares_v1.add_prefix(&prefix);
        self.tickets.add_prefix(&prefix);
        self.ticket_prices.add_prefix(&prefix);
        self.ticket_uses.add_prefix(&prefix);
        self.ticket_use_perimeters.add_prefix(&prefix);
        self.ticket_use_restrictions.add_prefix(&prefix);
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
        fn add_prefix(&mut self, prefix: &str) {
            self.0 = format!("{}:{}", prefix, self.0);
        }
    }

    #[test]
    fn collection() {
        let obj1 = Obj(String::from("some_id"));
        let obj2 = Obj(String::from("other_id"));
        let mut collection = Collection::new(vec![obj1, obj2]);
        collection.add_prefix("pre");
        let mut values = collection.values();
        let element = values.next().unwrap();
        assert_eq!(String::from("pre:some_id"), element.0);
        let element = values.next().unwrap();
        assert_eq!(String::from("pre:other_id"), element.0);
    }

    #[test]
    fn collection_with_id() {
        let obj1 = Obj(String::from("some_id"));
        let obj2 = Obj(String::from("other_id"));
        let mut collection = CollectionWithId::new(vec![obj1, obj2]).unwrap();
        collection.add_prefix("pre");
        let mut values = collection.values();
        let element = values.next().unwrap();
        assert_eq!(String::from("pre:some_id"), element.0);
        let element = values.next().unwrap();
        assert_eq!(String::from("pre:other_id"), element.0);
    }
}
