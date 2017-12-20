use std::collections::{HashMap, HashSet};
use collection::{Collection, Id, Idx};

pub type IdxSet<T> = HashSet<Idx<T>>;

pub struct OneToMany<T, U> {
    one_to_many: HashMap<Idx<T>, IdxSet<U>>,
    many_to_one: HashMap<Idx<U>, Idx<T>>,
}

impl<T, U> OneToMany<T, U>
where
    T: Id<T>,
    U: Id<U> + Id<T>,
{
    pub fn new(one: &Collection<T>, many: &Collection<U>) -> Self {
        let mut one_to_many = HashMap::default();
        let mut many_to_one = HashMap::default();
        for (many_idx, obj) in many.iter() {
            let one_idx = one.get_idx(<U as Id<T>>::id(obj)).unwrap();
            many_to_one.insert(many_idx, one_idx);
            one_to_many
                .entry(one_idx)
                .or_insert_with(HashSet::default)
                .insert(many_idx);
        }
        OneToMany {
            one_to_many: one_to_many,
            many_to_one: many_to_one,
        }
    }

    pub fn get_corresponding_forward(&self, from: &IdxSet<T>) -> IdxSet<U> {
        from.iter()
            .filter_map(|from_idx| self.one_to_many.get(from_idx))
            .flat_map(|indices| indices.iter().cloned())
            .collect()
    }

    pub fn get_corresponding_backward(&self, from: &IdxSet<U>) -> IdxSet<T> {
        from.iter()
            .filter_map(|from_idx| self.many_to_one.get(from_idx))
            .cloned()
            .collect()
    }
}
