use std::collections::HashMap;
use std::marker::PhantomData;
use std::iter;
use std::slice;
use std::ops;

pub trait Id<T> {
    fn id(&self) -> &str;
}

// TODO: no serde on Idx
#[derive(Derivative, Serialize, Deserialize, Debug)]
#[derivative(Copy(bound=""), Clone(bound=""), PartialEq(bound=""), Eq(bound=""), Hash(bound=""))]
pub struct Idx<T>(u32, PhantomData<T>);

impl<T> Idx<T> {
    fn new(idx: usize) -> Self {
        Idx(idx as u32, PhantomData)
    }
    fn get(&self) -> usize {
        self.0 as usize
    }
}

// TODO: serde only on objects
#[derive(Serialize, Deserialize, Debug)]
pub struct Collection<T> {
    objects: Vec<T>,
    id_to_idx: HashMap<String, Idx<T>>,
}

pub type Iter<'a, T> = iter::Map<iter::Enumerate<slice::Iter<'a, T>>, fn((usize, &T)) -> (Idx<T>, &T)>;

impl<T: Id<T>> Collection<T> {
    pub fn from_vec(v: Vec<T>) -> Self {
        let mut res = Collection { objects: v, id_to_idx: HashMap::default() };
        res.id_to_idx = res.iter().map(|(idx, obj)| (obj.id().to_string(), idx)).collect();
        res
    }
}

impl<T> Default for Collection<T> {
    fn default() -> Self {
        Collection { objects: Vec::default(), id_to_idx: HashMap::default() }
    }
}

impl<T> Collection<T> {
    pub fn iter<'a>(&'a self) -> Iter<'a, T> {
        self.objects.iter().enumerate().map(|(idx, obj)| (Idx::new(idx), obj))
    }

    pub fn get_idx(&self, id: &str) -> Option<Idx<T>> {
        self.id_to_idx.get(id).map(Clone::clone)
    }

    pub fn get(&self, id: &str) -> Option<&T> {
        self.get_idx(id).map(|idx| &self[idx])
    }
}

impl<T> ops::Index<Idx<T>> for Collection<T> {
    type Output = T;
    fn index(&self, index: Idx<T>) -> &Self::Output {
        &self.objects[index.get()]
    }
}
