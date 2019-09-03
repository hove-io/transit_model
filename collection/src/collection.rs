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

//! Collections of objects with typed indices and buildin identifier
//! support.

use crate::Result;
use derivative::Derivative;
use failure::{bail, ensure};
use log::warn;
use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::hash_map::Entry::*;
use std::collections::HashMap;
use std::iter;
use std::marker::PhantomData;
use std::ops;
use std::result::Result as StdResult;
use std::slice;

pub trait WithId {
    fn with_id(id: &str) -> Self;
}

/// An object that has a unique identifier.
pub trait Id<T> {
    /// Returns the unique identifier.
    fn id(&self) -> &str;

    /// Set the identifier
    fn set_id(&mut self, id: String);
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
    fn get(self) -> usize {
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

/// Creates a `Collection` from one element.
///
/// # Examples
///
/// ```
/// # use crate::collection::*;
/// let collection: Collection<i32> = Collection::from(42);
/// assert_eq!(collection.len(), 1);
/// let integer = collection.into_iter().next().unwrap();
/// assert_eq!(integer, 42);
/// ```
impl<T> From<T> for Collection<T> {
    fn from(object: T) -> Self {
        Collection::new(vec![object])
    }
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
    /// # use crate::collection::*;
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
    /// # use crate::collection::*;
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
    /// # use crate::collection::*;
    /// let c: Collection<i32> = Collection::new(vec![1, 1, 2, 3, 5, 8]);
    /// let (k, v): (Idx<i32>, &i32) = c.iter().nth(4).unwrap();
    /// assert_eq!(v, &5);
    /// assert_eq!(&c[k], &5);
    /// ```
    pub fn iter(&self) -> Iter<'_, T> {
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
    /// # use crate::collection::*;
    /// let c: Collection<i32> = Collection::new(vec![1, 1, 2, 3, 5, 8]);
    /// let values: Vec<&i32> = c.values().collect();
    /// assert_eq!(values, &[&1, &1, &2, &3, &5, &8]);
    /// ```
    pub fn values(&self) -> slice::Iter<'_, T> {
        self.objects.iter()
    }

    /// Iterates over the `&mut T` of the `Collection`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::collection::*;
    /// let mut c: Collection<i32> = Collection::new(vec![1, 1, 2, 3, 5, 8]);
    /// for elem in c.values_mut() {
    ///     *elem *= 2;
    /// }
    /// assert_eq!(c, Collection::new(vec![2, 2, 4, 6, 10, 16]));
    /// ```
    pub fn values_mut(&mut self) -> slice::IterMut<'_, T> {
        self.objects.iter_mut()
    }

    /// Iterates on the objects corresponding to the given indices.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::collection::*;
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
    /// # use crate::collection::*;
    /// # fn run() -> crate::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 }
    /// # fn set_id(&mut self, id: String) { unimplemented!(); }}
    /// let mut c = Collection::default();
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
    /// # use crate::collection::*;
    /// # fn run() -> crate::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 }
    /// # fn set_id(&mut self, id: String) { unimplemented!(); }}
    /// let mut c1 = Collection::from(Obj("foo"));
    /// let c2 = Collection::from(Obj("bar"));
    /// c1.merge(c2);
    /// assert_eq!(c1.len(), 2);
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn merge(&mut self, other: Self) {
        for item in other {
            self.push(item);
        }
    }

    /// Takes the corresponding vector without clones or allocation,
    /// leaving `self` empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::collection::*;
    /// # fn run() -> crate::Result<()> {
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
    /// # use crate::collection::*;
    /// # fn run() -> crate::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj;
    /// let mut c: Collection<Obj> = Collection::default();
    /// assert!(c.is_empty());
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// Retains the elements matching predicate parameter from the current `CollectionWithId` object
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::collection::*;
    /// # use std::collections::HashSet;
    /// # fn run() -> crate::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// let mut c = Collection::new(vec![Obj("foo"), Obj("bar"), Obj("qux")]);
    /// let mut ids_to_keep: HashSet<String> = HashSet::new();
    /// ids_to_keep.insert("foo".to_string());
    /// ids_to_keep.insert("qux".to_string());
    /// c.retain(|item| ids_to_keep.contains(item.0));
    /// assert_eq!(c.len(), 2);
    /// assert_eq!(c.values().map(|obj| obj.0).collect::<Vec<&str>>(), ["foo", "qux"]);
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        let mut purged = self.take();
        purged.retain(f);
        *self = Self::new(purged);
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
#[derive(Debug, Derivative, Clone)]
#[derivative(Default(bound = ""))]
pub struct CollectionWithId<T> {
    collection: Collection<T>,
    id_to_idx: HashMap<String, Idx<T>>,
}

/// Creates a `CollectionWithId` from one element.
///
/// # Examples
///
/// ```
/// # use crate::collection::*;
/// #[derive(PartialEq, Debug)]
/// struct Obj(&'static str);
/// impl Id<Obj> for Obj {
///     fn id(&self) -> &str { self.0 }
///     fn set_id(&mut self, id: String) { unimplemented!(); }
/// }
/// let collection: CollectionWithId<Obj> = CollectionWithId::from(Obj("some_id"));
/// assert_eq!(collection.len(), 1);
/// let obj = collection.into_iter().next().unwrap();
/// assert_eq!(obj.id(), "some_id");
/// ```
impl<T: Id<T>> From<T> for CollectionWithId<T> {
    fn from(object: T) -> Self {
        // This cannot fail since there will be a unique identifier in the
        // collection hence no identifier's collision.
        CollectionWithId::new(vec![object]).unwrap()
    }
}

impl<T: Id<T>> CollectionWithId<T> {
    /// Creates a `CollectionWithId` from a `Vec`. Fails if there is
    /// duplicates in identifiers.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::collection::*;
    /// # fn run() -> crate::Result<()> {
    /// #[derive(PartialEq, Debug)]
    /// struct Obj(&'static str);
    /// impl Id<Obj> for Obj {
    ///     fn id(&self) -> &str { self.0 }
    ///     fn set_id(&mut self, id: String) { unimplemented!(); }
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

    /// Get a reference to the `String` to `Idx<T>` internal mapping.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::collection::*;
    /// # use std::collections::HashMap;
    /// # fn run() -> Result<()> {
    /// #[derive(PartialEq, Debug)]
    /// struct Obj(&'static str);
    /// impl Id<Obj> for Obj {
    ///     fn id(&self) -> &str { self.0 }
    ///     fn set_id(&mut self, id: String) { unimplemented!(); }
    /// }
    /// let c = CollectionWithId::new(vec![Obj("foo"), Obj("bar")])?;
    /// assert_eq!(c.len(), 2);
    /// assert_eq!(c.get_id_to_idx().len(), 2);
    /// # Ok(())
    /// # }
    pub fn get_id_to_idx(&self) -> &HashMap<String, Idx<T>> {
        &self.id_to_idx
    }

    /// Access to a mutable reference of the corresponding object.
    ///
    /// The `drop` of the proxy object panic if the identifier is
    /// modified to an identifier already on the collection.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::collection::*;
    /// # fn run() -> crate::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 }
    /// # fn set_id(&mut self, id: String) { unimplemented!(); }}
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
    /// # use crate::collection::*;
    /// # fn run() -> crate::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 }
    /// # fn set_id(&mut self, id: String) { unimplemented!(); }}
    /// let mut c = CollectionWithId::new(vec![Obj("foo"), Obj("bar")])?;
    /// let idx = c.get_idx("foo").unwrap();
    /// c.index_mut(idx).0 = "bar"; // panic
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn index_mut(&mut self, idx: Idx<T>) -> RefMut<'_, T> {
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
    /// # use crate::collection::*;
    /// # fn run() -> crate::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 }
    /// # fn set_id(&mut self, id: String) { unimplemented!(); }}
    /// let mut c = CollectionWithId::new(vec![Obj("foo"), Obj("bar")])?;
    /// c.get_mut("foo").unwrap().0 = "baz";
    /// assert!(c.get("foo").is_none());
    /// assert_eq!(c.get("baz"), Some(&Obj("baz")));
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn get_mut(&mut self, id: &str) -> Option<RefMut<'_, T>> {
        self.get_idx(id).map(move |idx| self.index_mut(idx))
    }

    /// Push an element in the `CollectionWithId`.  Fails if the
    /// identifier of the new object is already in the collection.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::collection::*;
    /// # fn run() -> crate::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 }
    /// # fn set_id(&mut self, id: String) { unimplemented!(); }}
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

    /// Retains the elements matching predicate parameter from the current `CollectionWithId` object
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::collection::*;
    /// # use std::collections::HashSet;
    /// # fn run() -> crate::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 }
    /// # fn set_id(&mut self, id: String) { unimplemented!(); }}
    /// let mut c = CollectionWithId::new(vec![Obj("foo"), Obj("bar"), Obj("qux")])?;
    /// let mut ids_to_keep: HashSet<String> = HashSet::new();
    /// ids_to_keep.insert("foo".to_string());
    /// ids_to_keep.insert("qux".to_string());
    /// c.retain(|item| ids_to_keep.contains(item.id()));
    /// assert_eq!(c.len(), 2);
    /// assert_eq!(c.get("foo"), Some(&Obj("foo")));
    /// assert_eq!(c.get("qux"), Some(&Obj("qux")));
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        let mut purged = self.take();
        purged.retain(f);
        *self = Self::new(purged).unwrap(); // can't fail as we have a subset of a valid Collection
    }

    /// Merge a `CollectionWithId` parameter into the current one. Fails if any identifier into the
    /// `CollectionWithId` parameter is already in the collection.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::collection::*;
    /// # fn run() -> crate::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 }
    /// # fn set_id(&mut self, id: String) { unimplemented!(); }}
    /// let mut c1 = CollectionWithId::new(vec![Obj("foo"), Obj("bar")])?;
    /// let mut c2 = CollectionWithId::new(vec![Obj("foo"), Obj("qux")])?;
    /// let mut c3 = CollectionWithId::new(vec![Obj("corge"), Obj("grault")])?;
    /// assert!(c1.try_merge(c2).is_err());
    /// c1.try_merge(c3);
    /// assert_eq!(c1.len(), 4);
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn try_merge(&mut self, other: Self) -> Result<()> {
        for item in other {
            self.push(item)?;
        }
        Ok(())
    }

    /// Merge a `CollectionWithId` parameter into the current one. If any identifier into the
    /// `CollectionWithId` parameter is already in the collection, `CollectionWithId` is not added.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::collection::*;
    /// # fn run() -> crate::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 }
    /// # fn set_id(&mut self, id: String) { unimplemented!(); }}
    /// let mut c1 = CollectionWithId::new(vec![Obj("foo"), Obj("bar")])?;
    /// let mut c2 = CollectionWithId::new(vec![Obj("foo"), Obj("qux")])?;
    /// c1.merge(c2);
    /// assert_eq!(c1.len(), 3);
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn merge(&mut self, other: Self) {
        for item in other {
            match self.push(item) {
                _ => continue,
            }
        }
    }

    /// Merge all elements of an `Iterator` into the current `CollectionWithId`.
    /// If any identifier of an inserted element is already in the collection,
    /// the closure is called, with first parameter being the element with this
    /// identifier already in the collection, and the second parameter is the
    /// element to be inserted.
    /// ```
    /// # use crate::collection::*;
    /// #[derive(Debug, Default)]
    /// struct ObjectId<'a> {
    ///    id: &'a str,
    ///    name: &'a str,
    /// }
    /// impl Id<ObjectId<'_>> for ObjectId<'_> {
    ///    fn id(&self) -> &str {
    ///        self.id
    ///    }
    ///    fn set_id(&mut self, _id: String) {
    ///        unimplemented!()
    ///    }
    /// }
    ///
    /// let mut collection = CollectionWithId::default();
    /// let _ = collection.push(ObjectId {
    ///     id: "foo",
    ///     name: "Bob",
    /// });
    /// let vec = vec![ObjectId {
    ///     id: "bar",
    ///     name: "SpongeBob SquarePants",
    /// }];
    /// // Merge without collision of identifiers
    /// collection.merge_with(vec, |_, _| {
    ///   // Should never come here
    ///   assert!(false);
    /// });
    /// assert!(collection.get("bar").is_some());
    ///
    /// let vec = vec![ObjectId {
    ///     id: "foo",
    ///     name: "Bob Marley",
    /// }];
    /// // Merge with collision of identifiers
    /// collection.merge_with(vec, |source, to_merge| {
    ///     source.name = to_merge.name;
    /// });
    /// let foo = collection.get("foo").unwrap();
    /// assert_eq!(foo.name, "Bob Marley");
    /// ```
    pub fn merge_with<I, F>(&mut self, iterator: I, mut f: F)
    where
        F: FnMut(&mut T, &T),
        I: IntoIterator<Item = T>,
    {
        for e in iterator {
            if let Some(mut source) = self.get_mut(e.id()) {
                use std::ops::DerefMut;
                f(source.deref_mut(), &e);
                continue;
            }
            self.push(e).unwrap();
        }
    }

    // Return true if the collection has no objects.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::collection::*;
    /// # fn run() -> crate::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 }
    /// # fn set_id(&mut self, id: String) { unimplemented!(); }}
    /// let mut c: CollectionWithId<Obj> = CollectionWithId::default();
    /// assert!(c.is_empty());
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn is_empty(&self) -> bool {
        self.collection.is_empty()
    }
}

impl<T: Id<T> + WithId> CollectionWithId<T> {
    /// Get a mutable reference of the corresponding object or create it
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::collection::*;
    /// # use crate::WithId;
    /// # fn run() -> crate::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(String);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { &self.0 }
    /// # fn set_id(&mut self, id: String) { self.0 = id; }}
    /// # impl WithId for Obj {
    /// #     fn with_id(id: &str) -> Self {
    /// #         let mut r = Obj("id".into());
    /// #         r.0 = id.to_owned();
    /// #         r
    /// #     }
    /// # }
    /// let mut c = CollectionWithId::from(Obj("1".into()));
    /// let obj = c.get_or_create("2");
    /// assert_eq!(obj.0, "2");
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn get_or_create<'a>(&'a mut self, id: &str) -> RefMut<'a, T> {
        self.get_or_create_with(id, || T::with_id(id))
    }
}

impl<T: Id<T>> CollectionWithId<T> {
    /// Get a mutable reference of the corresponding object or create it
    /// and apply a function on it.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::collection::*;
    /// # use crate::WithId;
    /// # fn run() -> crate::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(String, String);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { &self.0 }
    /// # fn set_id(&mut self, id: String) { self.0 = id; }}
    /// # impl WithId for Obj {
    /// #     fn with_id(id: &str) -> Self {
    /// #         let mut r = Obj("id".into(), "name".into());
    /// #         r.0 = id.to_owned();
    /// #         r
    /// #     }
    /// # }
    /// let mut c = CollectionWithId::from(Obj("1".into(), "foo".into()));
    /// let obj = c.get_or_create_with("2", || Obj("bob".into(), "bar".into()));
    /// assert_eq!(obj.0, "2");
    /// assert_eq!(obj.1, "bar");
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn get_or_create_with<'a, F>(&'a mut self, id: &str, mut f: F) -> RefMut<'a, T>
    where
        F: FnMut() -> T,
    {
        let elt = self.get_idx(id).unwrap_or_else(|| {
            let mut o = f();

            o.set_id(id.to_string());
            self.push(o).unwrap()
        });
        self.index_mut(elt)
    }
}

impl<T: Id<T>> iter::Extend<T> for CollectionWithId<T> {
    /// Extend a `CollectionWithId` with the content of an iterator of
    /// CollectionWithId without duplicated ids.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::collection::*;
    /// # fn run() -> crate::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 }
    /// # fn set_id(&mut self, id: String) { unimplemented!(); }}
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
            match self.push(item) {
                Ok(val) => val,
                Err(e) => {
                    warn!("{}", e);
                    continue;
                }
            };
        }
    }
}

impl<T> CollectionWithId<T> {
    /// Returns the index corresponding to the identifier.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::collection::*;
    /// # fn run() -> crate::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 }
    /// # fn set_id(&mut self, id: String) { unimplemented!(); }}
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
    /// # use crate::collection::*;
    /// # fn run() -> crate::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 }
    /// # fn set_id(&mut self, id: String) { unimplemented!(); }}
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
    /// # use crate::collection::*;
    /// # fn run() -> crate::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 }
    /// # fn set_id(&mut self, id: String) { unimplemented!(); }}
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
    /// # use crate::collection::*;
    /// # fn run() -> crate::Result<()> {
    /// # #[derive(PartialEq, Debug)] struct Obj(&'static str);
    /// # impl Id<Obj> for Obj { fn id(&self) -> &str { self.0 }
    /// # fn set_id(&mut self, id: String) { unimplemented!(); }}
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
pub struct RefMut<'a, T: Id<T>> {
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
