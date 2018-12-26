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

//! Modeling the relations between objects.
//!
//! This module defines types for modeling the relations between
//! objects, and use them thanks to the `GetCorresponding` custom
//! derive.
//!
//! Let's clarify that with an example. Suppose that `Bike`s have a
//! `Brand`. `Bike`s also have an `Owner`, and these `Owner`s have a
//! `Job`. `Bike`s also have a `Kind`.
//!
//! ```raw
//! Brand - Bike - Owner - Job
//!          |
//!         Kind
//! ```
//!
//! Let's defines these relations and use them a bit:
//!
//! ```no_run
//! # use get_corresponding_derive::*;
//! # use navitia_model::relations::*;
//! # use navitia_model::collection::Idx;
//! # struct Bike;
//! # struct Brand;
//! # struct Owner;
//! # struct Job;
//! # struct Kind;
//! # fn get_mbk_brand() -> Idx<Brand> { unimplemented!() }
//! #[derive(Default, GetCorresponding)]
//! pub struct World {
//!     brands_to_bikes: OneToMany<Brand, Bike>,
//!     owners_to_bikes: OneToMany<Owner, Bike>,
//!     jobs_to_owners: OneToMany<Job, Owner>,
//!     kinds_to_bikes: OneToMany<Kind, Bike>,
//! }
//! fn main() {
//!     let world = World::default();
//!     let mbk: Idx<Brand> = get_mbk_brand();
//!     let owners_with_mbk: IdxSet<Owner> = world.get_corresponding_from_idx(mbk);
//!     let jobs_with_mbk: IdxSet<Job> = world.get_corresponding(&owners_with_mbk);
//!     println!(
//!         "{} owners with {} different jobs own a bike of the brand MBK.",
//!         owners_with_mbk.len(),
//!         jobs_with_mbk.len()
//!     );
//! }
//! ```
//!
//! First, we want to model the relations between the object. One bike
//! has a brand, and a brand has several bikes (hopefully). Thus, we
//! use a `OneToMany<Bike, Brand>` to model this relation.
//!
//! We repeat this process to model every relation. We obtain without
//! too much effort the `World` struct.
//!
//! The `GetCorresponding` derive looks at each field of the `World`
//! struct, keeping the fields containing `_to_` with a type with 2
//! generics, and interpret that as a relation. For example,
//! `bikes_to_brands: OneToMany<Bike, Brand>` is a relation between
//! `Bike` and `Brand`. Using all the relations, it generates a graph,
//! compute the shortest path between all the types, and generate an
//! `impl GetCorresponding` for each feasible path.
//!
//! These `impl GetCorresponding` are used by
//! `World::get_corresponding_from_idx` and `World::get_corresponding`
//! that are helpers to explore the `World`.
//!
//! Thus, when we call `world.get_corresponding_from_idx(mbk)` for
//! `Owner`, we will use the generated code that, basically, gets all
//! the `Bike`s corresponding to the `Brand` MBK, and then gets all
//! the `Owner`s corresponding to these `Bike`s.
//!
//! Imagine that, in our application, we use a lot the `Owner->Kind`
//! and `Brand->Kind` search.  To do these searches, we pass by
//! `Bike`, and there is a lot of `Bike`s in our model.  Thus, as an
//! optimization, we want to precompute these relations.
//!
//! ```raw
//! Brand - Bike - Owner - Job
//!    \     |      /
//!     `-- Kind --'
//! ```
//!
//! The shortcuts `Brand - Kind` and `Kind - Owner` allow our
//! optimization, but we now have a problem for the `Owner->Brand`
//! search: we can do `Owner->Kind->Brand` and `Owner->Bike->Brand`
//! with a cost of 2.  The first solution is clearly wrong, introduced
//! by our shortcuts.  To fix this problem, we can put a weight of 1.9
//! on `Brand - Kind` and `Kind - Owner`.  The path
//! `Owner->Kind->Brand` now cost 3.8 and is discarded.
//!
//! Let's implement that:
//!
//! ```
//! # use get_corresponding_derive::*;
//! # use navitia_model::relations::*;
//! # use navitia_model::collection::Idx;
//! # struct Bike;
//! # struct Brand;
//! # struct Owner;
//! # struct Job;
//! # struct Kind;
//! # fn get_mbk_brand() -> Idx<Brand> { unimplemented!() }
//! #[derive(GetCorresponding)]
//! pub struct World {
//!     brands_to_bikes: OneToMany<Brand, Bike>,
//!     owners_to_bikes: OneToMany<Owner, Bike>,
//!     jobs_to_owners: OneToMany<Job, Owner>,
//!     kinds_to_bikes: OneToMany<Kind, Bike>,
//!
//!     // shortcuts
//!     #[get_corresponding(weight = "1.9")]
//!     brands_to_kinds: ManyToMany<Brand, Kind>,
//!     #[get_corresponding(weight = "1.9")]
//!     kinds_to_owners: ManyToMany<Kind, Owner>,
//! }
//! # fn create_brands_to_bikes() -> OneToMany<Brand, Bike> { unimplemented!() }
//! # fn create_owners_to_bikes() -> OneToMany<Owner, Bike> { unimplemented!() }
//! # fn create_jobs_to_owners() -> OneToMany<Job, Owner> { unimplemented!() }
//! # fn create_kinds_to_bikes() -> OneToMany<Kind, Bike> { unimplemented!() }
//! impl World {
//!     fn new() -> World {
//!         let brands_to_bikes = create_brands_to_bikes();
//!         let owners_to_bikes = create_owners_to_bikes();
//!         let jobs_to_owners = create_jobs_to_owners();
//!         let kinds_to_bikes = create_kinds_to_bikes();
//!         World {
//!             brands_to_kinds: ManyToMany::from_relations_sink(
//!                 &brands_to_bikes,
//!                 &kinds_to_bikes,
//!             ),
//!             kinds_to_owners: ManyToMany::from_relations_sink(
//!                 &kinds_to_bikes,
//!                 &owners_to_bikes,
//!             ),
//!             brands_to_bikes,
//!             owners_to_bikes,
//!             jobs_to_owners,
//!             kinds_to_bikes,
//!         }
//!     }
//! }
//! # fn main() {}
//! ```

use crate::collection::{CollectionWithId, Id, Idx};
use crate::Result;
use derivative::Derivative;
use failure::{format_err, ResultExt};
use std::collections::{BTreeMap, BTreeSet};

/// A set of `Idx<T>`
pub type IdxSet<T> = BTreeSet<Idx<T>>;

/// An object linking 2 types together.
pub trait Relation {
    /// The type of the source object
    type From;

    /// The type of the targer object
    type To;

    /// Returns the complete set of the source objects.
    fn get_from(&self) -> IdxSet<Self::From>;

    /// Returns the complete set of the target objects.
    fn get_to(&self) -> IdxSet<Self::To>;

    /// For a given set of the source objects, returns the
    /// corresponding targets objects.
    fn get_corresponding_forward(&self, from: &IdxSet<Self::From>) -> IdxSet<Self::To>;

    /// For a given set of the target objects, returns the
    /// corresponding source objects.
    fn get_corresponding_backward(&self, from: &IdxSet<Self::To>) -> IdxSet<Self::From>;
}

/// A one to many relation, i.e. to one `T` corresponds many `U`,
/// and a `U` has one corresponding `T`.
#[derive(Derivative, Debug)]
#[derivative(Default(bound = ""))]
pub struct OneToMany<T, U> {
    one_to_many: BTreeMap<Idx<T>, IdxSet<U>>,
    many_to_one: BTreeMap<Idx<U>, Idx<T>>,
}

impl<T, U> OneToMany<T, U>
where
    T: Id<T>,
    U: Id<U> + Id<T>,
{
    fn new_impl(one: &CollectionWithId<T>, many: &CollectionWithId<U>) -> Result<Self> {
        let mut one_to_many = BTreeMap::default();
        let mut many_to_one = BTreeMap::default();
        for (many_idx, obj) in many {
            let one_id = <U as Id<T>>::id(obj);
            let one_idx = one
                .get_idx(one_id)
                .ok_or_else(|| format_err!("id={:?} not found", one_id))?;
            many_to_one.insert(many_idx, one_idx);
            one_to_many
                .entry(one_idx)
                .or_insert_with(IdxSet::default)
                .insert(many_idx);
        }
        Ok(OneToMany {
            one_to_many,
            many_to_one,
        })
    }
    /// Construct the relation automatically from the 2 given
    /// `CollectionWithId`s.
    pub fn new(
        one: &CollectionWithId<T>,
        many: &CollectionWithId<U>,
        rel_name: &str,
    ) -> Result<Self> {
        Ok(Self::new_impl(one, many).with_context(|_| format!("Error indexing {}", rel_name))?)
    }
}

impl<T, U> Relation for OneToMany<T, U> {
    type From = T;
    type To = U;
    fn get_from(&self) -> IdxSet<T> {
        self.one_to_many.keys().cloned().collect()
    }
    fn get_to(&self) -> IdxSet<U> {
        self.many_to_one.keys().cloned().collect()
    }
    fn get_corresponding_forward(&self, from: &IdxSet<T>) -> IdxSet<U> {
        get_corresponding(&self.one_to_many, from)
    }
    fn get_corresponding_backward(&self, from: &IdxSet<U>) -> IdxSet<T> {
        from.iter()
            .filter_map(|from_idx| self.many_to_one.get(from_idx))
            .cloned()
            .collect()
    }
}

/// A many to many relation, i.e. a `T` can have multiple `U`, and
/// vice versa.
#[derive(Default, Debug)]
pub struct ManyToMany<T, U> {
    forward: BTreeMap<Idx<T>, IdxSet<U>>,
    backward: BTreeMap<Idx<U>, IdxSet<T>>,
}

impl<T, U> ManyToMany<T, U> {
    /// Constructor from the forward relation.
    pub fn from_forward(forward: BTreeMap<Idx<T>, IdxSet<U>>) -> Self {
        let mut backward = BTreeMap::default();
        forward
            .iter()
            .flat_map(|(&from_idx, obj)| obj.iter().map(move |&to_idx| (from_idx, to_idx)))
            .for_each(|(from_idx, to_idx)| {
                backward
                    .entry(to_idx)
                    .or_insert_with(IdxSet::default)
                    .insert(from_idx);
            });
        ManyToMany { forward, backward }
    }

    /// Constructor from 2 chained relations, i.e. from the relations
    /// `A->B` and `B->C`, constructs the relation `A->C`.
    pub fn from_relations_chain<R1, R2>(r1: &R1, r2: &R2) -> Self
    where
        R1: Relation<From = T>,
        R2: Relation<From = R1::To, To = U>,
    {
        let forward = r1
            .get_from()
            .into_iter()
            .map(|idx| {
                let from = Some(idx).into_iter().collect();
                let tmp = r1.get_corresponding_forward(&from);
                (idx, r2.get_corresponding_forward(&tmp))
            })
            .collect();
        Self::from_forward(forward)
    }

    /// Constructor from 2 relations with a common sink, i.e. from the
    /// relations `A->B` and `C->B`, constructs the relation `A->C`.
    pub fn from_relations_sink<R1, R2>(r1: &R1, r2: &R2) -> Self
    where
        R1: Relation<From = T>,
        R2: Relation<From = U, To = R1::To>,
    {
        let forward = r1
            .get_from()
            .into_iter()
            .map(|idx| {
                let from = Some(idx).into_iter().collect();
                let tmp = r1.get_corresponding_forward(&from);
                (idx, r2.get_corresponding_backward(&tmp))
            })
            .collect();
        Self::from_forward(forward)
    }

    /// Constructor from 2 relations with a common source, i.e. from
    /// the relations `B->A` and `B->C`, constructs the relation
    /// `A->C`.
    pub fn from_relations_source<R1, R2>(r1: &R1, r2: &R2) -> Self
    where
        R1: Relation<To = T>,
        R2: Relation<From = R1::From, To = U>,
    {
        let forward = r1
            .get_to()
            .into_iter()
            .map(|idx| {
                let from = Some(idx).into_iter().collect();
                let tmp = r1.get_corresponding_backward(&from);
                (idx, r2.get_corresponding_forward(&tmp))
            })
            .collect();
        Self::from_forward(forward)
    }
}

impl<T, U> Relation for ManyToMany<T, U> {
    type From = T;
    type To = U;
    fn get_from(&self) -> IdxSet<T> {
        self.forward.keys().cloned().collect()
    }
    fn get_to(&self) -> IdxSet<U> {
        self.backward.keys().cloned().collect()
    }
    fn get_corresponding_forward(&self, from: &IdxSet<T>) -> IdxSet<U> {
        get_corresponding(&self.forward, from)
    }
    fn get_corresponding_backward(&self, from: &IdxSet<U>) -> IdxSet<T> {
        get_corresponding(&self.backward, from)
    }
}

fn get_corresponding<T, U>(map: &BTreeMap<Idx<T>, IdxSet<U>>, from: &IdxSet<T>) -> IdxSet<U> {
    from.iter()
        .filter_map(|from_idx| map.get(from_idx))
        .flat_map(|indices| indices.iter().cloned())
        .collect()
}
