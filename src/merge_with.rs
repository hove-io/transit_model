use crate::collection::{CollectionWithId, Id};

pub trait MergeWith<T> {
    fn merge_with<I, F>(&mut self, iterator: I, f: F)
    where
        F: FnMut(&mut Self, &T),
        I: IntoIterator<Item = T>;
}

impl<V> MergeWith<V> for CollectionWithId<V>
where
    V: Id<V>,
{
    fn merge_with<I, F>(&mut self, iterator: I, mut f: F)
    where
        F: FnMut(&mut Self, &V),
        I: IntoIterator<Item = V>,
    {
        for e in iterator {
            if self.get_mut(e.id()).is_some() {
                f(self, &e);
            } else {
                let _ = self.push(e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default)]
    struct ObjectId<'a> {
        id: &'a str,
        name: &'a str,
    }

    impl Id<ObjectId<'_>> for ObjectId<'_> {
        fn id(&self) -> &str {
            self.id
        }
        fn set_id(&mut self, _id: String) {
            unimplemented!()
        }
    }

    #[test]
    fn it_works() {
        let mut collection = CollectionWithId::default();
        let _ = collection.push(ObjectId {
            id: "foo",
            ..Default::default()
        });
        let vec = vec![ObjectId {
            id: "bar",
            ..Default::default()
        }];
        collection.merge_with(vec, |_, _| {
            // Should not go in there
            assert!(false)
        });
        assert!(collection.get("foo").is_some());
        assert!(collection.get("bar").is_some());
    }

    #[test]
    fn empty_iterator() {
        let mut collection = CollectionWithId::default();
        let _ = collection.push(ObjectId {
            id: "foo",
            ..Default::default()
        });
        let vec = vec![];
        collection.merge_with(vec, |_, _| {
            // Should not go in there
            assert!(false)
        });
        assert!(collection.get("foo").is_some());
    }

    #[test]
    fn empty_collections() {
        let mut collection: CollectionWithId<ObjectId> = CollectionWithId::default();
        let vec = vec![ObjectId {
            id: "bar",
            ..Default::default()
        }];
        collection.merge_with(vec, |_, _| {
            // Should not go in there
            assert!(false)
        });
        assert!(collection.get("bar").is_some());
    }

    #[test]
    fn merge() {
        let mut collection = CollectionWithId::default();
        let _ = collection.push(ObjectId {
            id: "foo",
            name: "Bob",
        });
        let vec = vec![ObjectId {
            id: "foo",
            name: "Marley",
        }];
        collection.merge_with(vec, |collection, to_merge| {
            let mut foo = collection.get_mut("foo").unwrap();
            assert_eq!(foo.name, "Bob");
            assert_eq!(to_merge.id, "foo");
            foo.name = to_merge.name;
        });
        let foo = collection.get("foo").unwrap();
        assert_eq!(foo.name, "Marley");
    }
}
