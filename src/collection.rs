use std::collections::HashMap;
use std::marker::PhantomData;
use std::iter;
use std::slice;
use std::ops;
use std::cmp::Ordering;

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

impl<T> Collection<T> {
    pub fn new(v: Vec<T>) -> Self {
        Collection { objects: v }
    }
}

impl<T> Default for Collection<T> {
    fn default() -> Self {
        Collection::new(Vec::default())
    }
}

impl<T> ops::Deref for Collection<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Vec<T> {
        &self.objects
    }
}

impl<T> ::serde::Serialize for Collection<T>
where
    T: ::serde::Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
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
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
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

impl<T> ::serde::Serialize for CollectionWithId<T>
where
    T: ::serde::Serialize + Id<T>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        self.collection.objects.serialize(serializer)
    }
}
impl<'de, T> ::serde::Deserialize<'de> for CollectionWithId<T>
where
    T: ::serde::Deserialize<'de> + Id<T>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::serde::Deserialize::deserialize(deserializer).map(CollectionWithId::new)
    }
}

pub type Iter<'a, T> =
    iter::Map<iter::Enumerate<slice::Iter<'a, T>>, fn((usize, &T)) -> (Idx<T>, &T)>;

impl<T: Id<T>> CollectionWithId<T> {
    pub fn new(v: Vec<T>) -> Self {
        let mut id_to_idx = HashMap::default();
        for (i, obj) in v.iter().enumerate() {
            assert!(
                id_to_idx
                    .insert(obj.id().to_string(), Idx::new(i))
                    .is_none(),
                "{} already found",
                obj.id()
            );
        }
        CollectionWithId {
            collection: Collection::new(v),
            id_to_idx: id_to_idx,
        }
    }
    pub fn index_mut(&mut self, idx: Idx<T>) -> RefMut<T> {
        RefMut {
            idx: idx,
            old_id: self.collection.objects[idx.get()].id().to_string(),
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
        &self.collection.collection.objects[self.idx.get()]
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
    pub fn iter(&self) -> Iter<T> {
        self.collection
            .objects
            .iter()
            .enumerate()
            .map(|(idx, obj)| (Idx::new(idx), obj))
    }

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
        self.collection.objects.len()
    }
}

impl<T> ops::Index<Idx<T>> for CollectionWithId<T> {
    type Output = T;
    fn index(&self, index: Idx<T>) -> &Self::Output {
        &self.collection.objects[index.get()]
    }
}
