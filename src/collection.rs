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

use csv;
use failure::ResultExt;
use serde;
use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::iter;
use std::marker::PhantomData;
use std::ops;
use std::path;
use std::result::Result as StdResult;
use std::slice;

use Result;
use objects::AddPrefix;

pub trait Id<T> {
    fn id(&self) -> &str;
}

#[derive(Derivative, Debug)]
#[derivative(Copy(bound = ""), Clone(bound = ""), PartialEq(bound = ""), Eq(bound = ""),
             Hash(bound = ""))]
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

#[derive(Debug)]
pub struct Collection<T> {
    objects: Vec<T>,
}

impl<T: PartialEq> PartialEq for Collection<T> {
    fn eq(&self, other: &Collection<T>) -> bool {
        self.objects == other.objects
    }
}

impl<T> Collection<T> {
    pub fn new(v: Vec<T>) -> Self {
        Collection { objects: v }
    }

    pub fn iter_from<'a, I>(&'a self, indexes: I) -> Box<Iterator<Item = &T> + 'a>
    where
        I: IntoIterator + 'a,
        I::Item: Borrow<Idx<T>>,
    {
        Box::new(
            indexes
                .into_iter()
                .map(move |item| &self.objects[item.borrow().get()]),
        )
    }
}

impl<T> Default for Collection<T> {
    fn default() -> Self {
        Collection::new(Vec::default())
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

#[derive(Debug)]
pub struct CollectionWithId<T> {
    collection: Collection<T>,
    id_to_idx: HashMap<String, Idx<T>>,
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

pub type Iter<'a, T> =
    iter::Map<iter::Enumerate<slice::Iter<'a, T>>, fn((usize, &T)) -> (Idx<T>, &T)>;

impl<T: Id<T>> CollectionWithId<T> {
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
    pub fn index_mut(&mut self, idx: Idx<T>) -> RefMut<T> {
        RefMut {
            idx,
            old_id: self.objects[idx.get()].id().to_string(),
            collection: self,
        }
    }
}
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

impl<T> Default for CollectionWithId<T> {
    fn default() -> Self {
        CollectionWithId {
            collection: Collection::new(Vec::default()),
            id_to_idx: HashMap::default(),
        }
    }
}

impl<T> CollectionWithId<T> {
    pub fn get_idx(&self, id: &str) -> Option<Idx<T>> {
        self.id_to_idx.get(id).cloned()
    }

    pub fn get(&self, id: &str) -> Option<&T> {
        self.get_idx(id).map(|idx| &self[idx])
    }

    pub fn into_vec(self) -> Vec<T> {
        self.collection.objects
    }

    pub fn take(&mut self) -> Vec<T> {
        self.id_to_idx.clear();
        ::std::mem::replace(&mut self.collection.objects, Vec::new())
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }
}

impl<T> ops::Index<Idx<T>> for Collection<T> {
    type Output = T;
    fn index(&self, index: Idx<T>) -> &Self::Output {
        &self.objects[index.get()]
    }
}

impl<T> Collection<T> {
    pub fn iter(&self) -> Iter<T> {
        self.objects
            .iter()
            .enumerate()
            .map(|(idx, obj)| (Idx::new(idx), obj))
    }
}

pub fn make_opt_collection_with_id<T>(path: &path::Path, file: &str) -> Result<CollectionWithId<T>>
where
    T: Id<T>,
    for<'de> T: serde::Deserialize<'de>,
{
    if !path.join(file).exists() {
        info!("Skipping {}", file);
        Ok(CollectionWithId::default())
    } else {
        make_collection_with_id(path, file)
    }
}

pub fn make_collection_with_id<T>(path: &path::Path, file: &str) -> Result<CollectionWithId<T>>
where
    T: Id<T>,
    for<'de> T: serde::Deserialize<'de>,
{
    info!("Reading {}", file);
    let path = path.join(file);
    let mut rdr = csv::Reader::from_path(&path).with_context(ctx_from_path!(path))?;
    let vec = rdr.deserialize()
        .collect::<StdResult<_, _>>()
        .with_context(ctx_from_path!(path))?;
    CollectionWithId::new(vec)
}

pub fn make_opt_collection<T>(path: &path::Path, file: &str) -> Result<Collection<T>>
where
    for<'de> T: serde::Deserialize<'de>,
{
    if !path.join(file).exists() {
        info!("Skipping {}", file);
        Ok(Collection::default())
    } else {
        make_collection(path, file)
    }
}

pub fn make_collection<T>(path: &path::Path, file: &str) -> Result<Collection<T>>
where
    for<'de> T: serde::Deserialize<'de>,
{
    info!("Reading {}", file);
    let path = path.join(file);
    let mut rdr = csv::Reader::from_path(&path).with_context(ctx_from_path!(path))?;
    let vec = rdr.deserialize()
        .collect::<StdResult<_, _>>()
        .with_context(ctx_from_path!(path))?;
    Ok(Collection::new(vec))
}

pub fn add_prefix<T>(collection: &mut CollectionWithId<T>, prefix: &str) -> Result<()>
where
    T: AddPrefix + Id<T>,
{
    let mut objects = collection.take();
    for obj in &mut objects {
        obj.add_prefix(prefix);
    }

    *collection = CollectionWithId::new(objects)?;

    Ok(())
}
