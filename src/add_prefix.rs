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

use crate::collection::{Collection, CollectionWithId, Id};

pub trait AddPrefix {
    fn add_prefix(&mut self, prefix: &str);
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(element.0, String::from("pre:some_id"));
        let element = values.next().unwrap();
        assert_eq!(element.0, String::from("pre:other_id"));
    }

    #[test]
    fn collection_with_id() {
        let obj1 = Obj(String::from("some_id"));
        let obj2 = Obj(String::from("other_id"));
        let mut collection = CollectionWithId::new(vec![obj1, obj2]).unwrap();
        collection.add_prefix("pre");
        let mut values = collection.values();
        let element = values.next().unwrap();
        assert_eq!(element.0, String::from("pre:some_id"));
        let element = values.next().unwrap();
        assert_eq!(element.0, String::from("pre:other_id"));
    }
}
