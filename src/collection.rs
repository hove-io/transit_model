// Copyright 2017-2018 Kisio Digital and/or its affiliates.
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

//! Collections of objects with typed indices and buildin identifier
//! support.

use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::hash_map::Entry::*;
use std::collections::HashMap;
use std::iter;
use std::marker::PhantomData;
use std::ops;
use std::result::Result as StdResult;
use std::slice;
use Result;

/// An object that has a unique identifier.
pub trait Id<T> {
    /// Returns the unique identifier.
    fn id(&self) -> &str;
}

/// Typed index.
#[derive(Derivative, Debug)]
#[derivative(
    Copy(bound = ""),
    Clone(bound = ""),
    PartialEq(bound = ""),
    Eq(bound = ""),
    Hash(bound = "")
)]
pub struct Idx<T>(u32, PhantomData<T>);

impl<T> Idx<T> {
    fn new(idx: usize) -> Self {
        Idx(idx as u32, PhantomData)
    }
    fn get(&self) -> usize {
        self.0 as usize
    }
}
impl<T> Ord for Idx<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}
impl<T> PartialOrd for Idx<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// The `Collection` object looks like a `Map<Idx<T>, T>`, with opaque
/// keys.  Then, you can easily store indices and don't mess up
/// between different types of indices.
#[derive(Debug, Derivative, Clone)]
#[derivative(Default(bound = ""))]
pub struct Collection<T> {
    objects: Vec<T>,
}

impl<T: PartialEq> PartialEq for Collection<T> {
    fn eq(&self, other: &Collection<T>) -> bool {
        self.objects == other.objects
    }
}

impl<T> Collection<T> {
    /// Creates the `Collection` from a `Vec`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use navitia_model::collection::*;
    /// let _: Collection<i32> = Collection::new(vec![1, 1, 2, 3, 5, 8]);
    /// ```
    pub fn new(v: Vec<T>) -> Self {
        Collection { objects: v }
    }

    /// Returns the number of elements in the collection, also referred to as its 'length'.
    ///
    /// # Examples
    ///
    /// ```
    /// # use navitia_model::collection::*;
    /// let c: Collection<i32> = Collection::new(vec![1, 1, 2, 3, 5, 8]);
    /// assert_eq!(c.len(), 6);
    /// ```
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Iterates over the `(Idx<T>, &T)` of the `Collection`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use navitia_model::collection::*;
    /// let c: Collection<i32> = Collection::new(vec![1, 1, 2, 3, 5, 8]);
    /// let (k, v): (Idx<i32>, &i32) = c.iter().nth(4).unwrap();
    /// assert_eq!(v, &5);
    /// assert_eq!(&c[k], &5);
    /// ```
    pub fn iter(&self) -> Iter<T> {
        self.objects
            .iter()
            .enumerate()
            .map(|(idx, obj)| (Idx::new(idx), obj))
    }

    /// Iterates over the `&T` of the `Collection`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use navitia_model::collection::*;
    /// let c: Collection<i32> = Collection::new(vec![1, 1, 2, 3, 5, 8]);
    /// let values: Vec<&i32> = c.values().collect();
    /// assert_eq!(values, &[&1, &1, &2, &3, &5, &8]);
    /// ```
    pub fn values(&self) -> slice::Iter<T> {
        self.objects.iter()
    }

    /// Iterates over the `&mut T` of the `Collection`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use navitia_model::collection::*;
    /// let mut c: Collection<i32> = Collection::new(vec![1, 1, 2, 3, 5, 8]);
    /// for elem in c.values_mut() {
    ///     *elem *= 2;
    /// }
    /// assert_eq!(c, Collection::new(vec![2, 2, 4, 6, 10, 16]));
    /// ```
    pub fn values_mut(&mut self) -> slice::IterMut<T> {
        self.objects.iter_mut()
    }

    /// Iterates on the objects corresponding to the given indices.
    ///
    /// # Examples
    ///
    /// ```
    /// # use navitia_model::collection::*;
    /// # use std::collections::BTreeSet;
    /// # fn get_transit_indices(c: &Collection<&'static str>) -> BTreeSet<Idx<&'static str>> {
    /// #     c.iter()
    /// #         .filter(|&(_, &v)| v != "bike" && v != "walking" && v != "car")
    /// #         .map(|(k, _)| k)
    /// #         .collect()
    /// # }
    /// let c = Collection::new(vec!["bike", "bus", "walking", "car", "metro", "train"]);
    /// let transit_indices: BTreeSet<Idx<&str>> = get_transit_indices(&c);
    /// let transit_refs: Vec<&&str> = c.iter_from(&transit_indices).collect();
    /// assert_eq!(transit_refs, &[&"bus", &"metro", &"train"]);
    /// ```
    pub fn iter_from<I>(&self, indexes: I) -> impl Iterator<Item = &T>
    where
        I: IntoIterator,
        I::Item: Borrow<Idx<T>>,
    {
        indexes
            .into_iter()
            .map(move |item| &self.objects[item.borrow().get()])
    }

    /// Push an element in the `Collection` without control.
    ///
    /// # Examples
    ///
    /// ```
    /// # use navitia_model::collection::*;
    /// # fn run() -> navitia_model::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 } }
    /// let mut c = Collection::new(vec![]);
    /// let foo_idx = c.push(Obj("foo"));
    /// let bar_idx = c.push(Obj("bar"));
    /// assert_eq!(&c[foo_idx], &Obj("foo"));
    /// assert_ne!(&c[foo_idx], &Obj("bar"));
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn push(&mut self, item: T) -> Idx<T> {
        let next_index = self.objects.len();
        self.objects.push(item);
        Idx::new(next_index)
    }

    /// Merge a `Collection` parameter into the current one.
    ///
    /// # Examples
    ///
    /// ```
    /// # use navitia_model::collection::*;
    /// # fn run() -> navitia_model::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 } }
    /// let mut c1 = Collection::new(vec![Obj("foo")]);
    /// let c2 = Collection::new(vec![Obj("bar")]);
    /// c1.merge(c2);
    /// assert_eq!(c1.len(), 2);
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn merge(&mut self, other: Self) -> Result<()> {
        for item in other {
            self.push(item);
        }
        Ok(())
    }

    /// Takes the corresponding vector without clones or allocation,
    /// leaving `self` empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # use navitia_model::collection::*;
    /// # fn run() -> navitia_model::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// let mut c = Collection::new(vec![Obj("foo"), Obj("bar")]);
    /// let v = c.take();
    /// assert_eq!(v, &[Obj("foo"), Obj("bar")]);
    /// assert_eq!(c.len(), 0);
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn take(&mut self) -> Vec<T> {
        ::std::mem::replace(&mut self.objects, Vec::new())
    }

    // Return true if the collection has no objects.
    ///
    /// # Examples
    ///
    /// ```
    /// # use navitia_model::collection::*;
    /// # fn run() -> navitia_model::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj;
    /// let mut c: Collection<Obj> = Collection::new(vec![]);
    /// assert!(c.is_empty());
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }
}

/// The type returned by `Collection::iter`.
pub type Iter<'a, T> =
    iter::Map<iter::Enumerate<slice::Iter<'a, T>>, fn((usize, &T)) -> (Idx<T>, &T)>;

impl<'a, T> IntoIterator for &'a Collection<T> {
    type Item = (Idx<T>, &'a T);
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

impl<T> IntoIterator for Collection<T> {
    type Item = T;
    type IntoIter = ::std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.objects.into_iter()
    }
}

impl<T> ops::Index<Idx<T>> for Collection<T> {
    type Output = T;
    fn index(&self, index: Idx<T>) -> &Self::Output {
        &self.objects[index.get()]
    }
}

impl<T> ::serde::Serialize for Collection<T>
where
    T: ::serde::Serialize,
{
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        self.objects.serialize(serializer)
    }
}
impl<'de, T> ::serde::Deserialize<'de> for Collection<T>
where
    T: ::serde::Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> StdResult<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::serde::Deserialize::deserialize(deserializer).map(Collection::new)
    }
}

/// A `Collection` with identifier support.
#[derive(Debug, Derivative)]
#[derivative(Default(bound = ""))]
pub struct CollectionWithId<T> {
    collection: Collection<T>,
    id_to_idx: HashMap<String, Idx<T>>,
}

impl<T: Id<T>> CollectionWithId<T> {
    /// Creates a `CollectionWithId` from a `Vec`. Fails if there is
    /// duplicates in identifiers.
    ///
    /// # Examples
    ///
    /// ```
    /// # use navitia_model::collection::*;
    /// # fn run() -> navitia_model::Result<()> {
    /// #[derive(PartialEq, Debug)]
    /// struct Obj(&'static str);
    /// impl Id<Obj> for Obj {
    ///     fn id(&self) -> &str { self.0 }
    /// }
    /// let c = CollectionWithId::new(vec![Obj("foo"), Obj("bar")])?;
    /// assert_eq!(c.len(), 2);
    /// assert_eq!(c.get("foo"), Some(&Obj("foo")));
    /// assert!(CollectionWithId::new(vec![Obj("foo"), Obj("foo")]).is_err());
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    pub fn new(v: Vec<T>) -> Result<Self> {
        let mut id_to_idx = HashMap::default();
        for (i, obj) in v.iter().enumerate() {
            ensure!(
                id_to_idx
                    .insert(obj.id().to_string(), Idx::new(i))
                    .is_none(),
                "{} already found",
                obj.id()
            );
        }
        Ok(CollectionWithId {
            collection: Collection::new(v),
            id_to_idx,
        })
    }

    /// Access to a mutable reference of the corresponding object.
    ///
    /// The `drop` of the proxy object panic if the identifier is
    /// modified to an indentifier already on the collection.
    ///
    /// # Examples
    ///
    /// ```
    /// # use navitia_model::collection::*;
    /// # fn run() -> navitia_model::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 } }
    /// let mut c = CollectionWithId::new(vec![Obj("foo"), Obj("bar")])?;
    /// let idx = c.get_idx("foo").unwrap();
    /// c.index_mut(idx).0 = "baz";
    /// assert!(c.get("foo").is_none());
    /// assert_eq!(c.get("baz"), Some(&Obj("baz")));
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    ///
    /// ```should_panic
    /// # use navitia_model::collection::*;
    /// # fn run() -> navitia_model::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 } }
    /// let mut c = CollectionWithId::new(vec![Obj("foo"), Obj("bar")])?;
    /// let idx = c.get_idx("foo").unwrap();
    /// c.index_mut(idx).0 = "bar"; // panic
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn index_mut(&mut self, idx: Idx<T>) -> RefMut<T> {
        RefMut {
            idx,
            old_id: self.objects[idx.get()].id().to_string(),
            collection: self,
        }
    }

    /// Returns an option of a mutable reference of the corresponding object.
    ///
    /// # Examples
    ///
    /// ```
    /// # use navitia_model::collection::*;
    /// # fn run() -> navitia_model::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 } }
    /// let mut c = CollectionWithId::new(vec![Obj("foo"), Obj("bar")])?;
    /// c.get_mut("foo").unwrap().0 = "baz";
    /// assert!(c.get("foo").is_none());
    /// assert_eq!(c.get("baz"), Some(&Obj("baz")));
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn get_mut(&mut self, id: &str) -> Option<RefMut<T>> {
        self.get_idx(id).map(move |idx| self.index_mut(idx))
    }

    /// Push an element in the `CollectionWithId`.  Fails if the
    /// identifier of the new object is already in the collection.
    ///
    /// # Examples
    ///
    /// ```
    /// # use navitia_model::collection::*;
    /// # fn run() -> navitia_model::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 } }
    /// let mut c = CollectionWithId::new(vec![Obj("foo"), Obj("bar")])?;
    /// let baz_idx = c.push(Obj("baz"))?;
    /// assert_eq!(&c[baz_idx], &Obj("baz"));
    /// assert!(c.push(Obj("baz")).is_err());
    /// let foobar_idx = c.push(Obj("foobar"))?;
    /// assert_eq!(&c[baz_idx], &Obj("baz"));
    /// assert_eq!(&c[foobar_idx], &Obj("foobar"));
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn push(&mut self, item: T) -> Result<Idx<T>> {
        let next_index = self.collection.objects.len();
        let idx = Idx::new(next_index);
        match self.id_to_idx.entry(item.id().to_string()) {
            Occupied(_) => bail!("{} already found", item.id()),
            Vacant(v) => {
                v.insert(idx);
                self.collection.objects.push(item);
                Ok(idx)
            }
        }
    }

    /// Merge a `CollectionWithId` parameter into the current one. Fails if any identifier into the
    /// `CollectionWithId` parameter is already in the collection.
    ///
    /// # Examples
    ///
    /// ```
    /// # use navitia_model::collection::*;
    /// # fn run() -> navitia_model::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 } }
    /// let mut c1 = CollectionWithId::new(vec![Obj("foo"), Obj("bar")])?;
    /// let mut c2 = CollectionWithId::new(vec![Obj("foo"), Obj("qux")])?;
    /// let mut c3 = CollectionWithId::new(vec![Obj("corge"), Obj("grault")])?;
    /// assert!(c1.merge(c2).is_err());
    /// c1.merge(c3);
    /// assert_eq!(c1.len(), 4);
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn merge(&mut self, other: Self) -> Result<()> {
        for item in other {
            self.push(item)?;
        }
        Ok(())
    }

    // Return true if the collection has no objects.
    ///
    /// # Examples
    ///
    /// ```
    /// # use navitia_model::collection::*;
    /// # fn run() -> navitia_model::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 } }
    /// let mut c: CollectionWithId<Obj> = CollectionWithId::new(vec![])?;
    /// assert!(c.is_empty());
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn is_empty(&self) -> bool {
        self.collection.is_empty()
    }
}

impl<T: Id<T>> iter::Extend<T> for CollectionWithId<T> {
    /// Extend a `CollectionWithId` with the content of an iterator of
    /// CollectionWithId without duplicated ids.
    ///
    /// # Examples
    ///
    /// ```
    /// # use navitia_model::collection::*;
    /// # fn run() -> navitia_model::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 } }
    /// let mut c1 = CollectionWithId::new(vec![Obj("foo"), Obj("bar")])?;
    /// let mut c2 = CollectionWithId::new(vec![Obj("foo"), Obj("qux")])?;
    /// c1.extend(c2);
    /// assert_eq!(c1.len(), 3);
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            skip_fail!(self.push(item));
        }
    }
}

impl<T> CollectionWithId<T> {
    /// Returns the index corresponding to the identifier.
    ///
    /// # Examples
    ///
    /// ```
    /// # use navitia_model::collection::*;
    /// # fn run() -> navitia_model::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 } }
    /// let c = CollectionWithId::new(vec![Obj("foo"), Obj("bar")])?;
    /// let idx = c.get_idx("foo").unwrap();
    /// assert_eq!(&c[idx], &Obj("foo"));
    /// assert!(c.get_idx("baz").is_none());
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn get_idx(&self, id: &str) -> Option<Idx<T>> {
        self.id_to_idx.get(id).cloned()
    }

    /// Returns a reference to the object corresponding to the
    /// identifier.
    ///
    /// # Examples
    ///
    /// ```
    /// # use navitia_model::collection::*;
    /// # fn run() -> navitia_model::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 } }
    /// let c = CollectionWithId::new(vec![Obj("foo"), Obj("bar")])?;
    /// assert_eq!(c.get("foo"), Some(&Obj("foo")));
    /// assert!(c.get("baz").is_none());
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn get(&self, id: &str) -> Option<&T> {
        self.get_idx(id).map(|idx| &self[idx])
    }

    /// Converts `self` into a vector without clones or allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use navitia_model::collection::*;
    /// # fn run() -> navitia_model::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 } }
    /// let c = CollectionWithId::new(vec![Obj("foo"), Obj("bar")])?;
    /// let v = c.into_vec();
    /// assert_eq!(v, &[Obj("foo"), Obj("bar")]);
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn into_vec(self) -> Vec<T> {
        self.collection.objects
    }

    /// Takes the corresponding vector without clones or allocation,
    /// leaving `self` empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # use navitia_model::collection::*;
    /// # fn run() -> navitia_model::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 } }
    /// let mut c = CollectionWithId::new(vec![Obj("foo"), Obj("bar")])?;
    /// let v = c.take();
    /// assert_eq!(v, &[Obj("foo"), Obj("bar")]);
    /// assert_eq!(c.len(), 0);
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn take(&mut self) -> Vec<T> {
        self.id_to_idx.clear();
        ::std::mem::replace(&mut self.collection.objects, Vec::new())
    }
}

/// The structure returned by `CollectionWithId::index_mut`.
pub struct RefMut<'a, T: 'a + Id<T>> {
    idx: Idx<T>,
    collection: &'a mut CollectionWithId<T>,
    old_id: String,
}
impl<'a, T: Id<T>> ops::DerefMut for RefMut<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.collection.collection.objects[self.idx.get()]
    }
}
impl<'a, T: Id<T>> ops::Deref for RefMut<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.collection.objects[self.idx.get()]
    }
}
impl<'a, T: Id<T>> Drop for RefMut<'a, T> {
    fn drop(&mut self) {
        if self.id() != self.old_id {
            self.collection.id_to_idx.remove(&self.old_id);
            let new_id = self.id().to_string();
            assert!(
                self.collection.id_to_idx.insert(new_id, self.idx).is_none(),
                "changing id {} to {} already used",
                self.old_id,
                self.id()
            );
        }
    }
}

impl<T: PartialEq> PartialEq for CollectionWithId<T> {
    fn eq(&self, other: &CollectionWithId<T>) -> bool {
        self.collection == other.collection
    }
}

impl<T> ops::Deref for CollectionWithId<T> {
    type Target = Collection<T>;
    fn deref(&self) -> &Collection<T> {
        &self.collection
    }
}

impl<'a, T> IntoIterator for &'a CollectionWithId<T> {
    type Item = (Idx<T>, &'a T);
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T> IntoIterator for CollectionWithId<T> {
    type Item = T;
    type IntoIter = ::std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.collection.into_iter()
    }
}

impl<T> ::serde::Serialize for CollectionWithId<T>
where
    T: ::serde::Serialize + Id<T>,
{
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        self.objects.serialize(serializer)
    }
}
impl<'de, T> ::serde::Deserialize<'de> for CollectionWithId<T>
where
    T: ::serde::Deserialize<'de> + Id<T>,
{
    fn deserialize<D>(deserializer: D) -> StdResult<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        use serde::de::Error;
        ::serde::Deserialize::deserialize(deserializer)
            .and_then(|v| CollectionWithId::new(v).map_err(D::Error::custom))
    }
}
