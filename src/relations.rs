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
//! Let's defines these relations and use them a bit:
//!
//! ```no_run
//! # #[macro_use] extern crate get_corresponding_derive;
//! # extern crate navitia_model;
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
//!     bikes_to_brands: OneToMany<Bike, Brand>,
//!     bikes_to_owners: OneToMany<Bike, Owner>,
//!     owners_to_jobs: OneToMany<Owner, Job>,
//!     #[get_corresponding(weight = "1.1")]
//!     bikes_to_kinds: OneToMany<Bike, Kind>,
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
//! have a brand, and a brand have several bikes (hopefully). Thus, we
//! use a `OneToMany<Bike, Brand>` to model this relation.
//!
//! We repeat this process to model every relations. We obtain without
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
//! The weight of the relations can be tuned as shown for the
//! `bikes_to_kinds` relation. This is not really useful for this
//! example, but can be important when the relations are more complex.
//!
//! These `impl GetCorresponding` are used by
//! `World::get_corresponding_from_idx` and `World::get_corresponding`
//! that are helpers to explore the `World`.
//!
//! Thus, when we call `world.get_corresponding_from_idx(mbk)` for
//! `Owner`, we will use the generated code that, basically, gets all
//! the `Bike`s correponding to the `Brand` MBK, and then gets all the
//! `Owner`s corresponding to these `Bike`s.

use Result;
use collection::{CollectionWithId, Id, Idx};
use failure::ResultExt;
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

    /// For a given set of the source objects, returns the
    /// correponding targets objects.
    fn get_corresponding_forward(&self, from: &IdxSet<Self::From>) -> IdxSet<Self::To>;

    /// For a given set of the target objects, returns the
    /// correponding source objects.
    fn get_corresponding_backward(&self, from: &IdxSet<Self::To>) -> IdxSet<Self::From>;
}

/// A one to many relation, i.e. a `T` have one correponding `U`, and
/// a `U` can have multiple corresponding `T`.
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
        for (many_idx, obj) in many.iter() {
            let one_id = <U as Id<T>>::id(obj);
            let one_idx = one.get_idx(one_id)
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
        let forward = r1.get_from()
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
        let forward = r1.get_from()
            .into_iter()
            .map(|idx| {
                let from = Some(idx).into_iter().collect();
                let tmp = r1.get_corresponding_forward(&from);
                (idx, r2.get_corresponding_backward(&tmp))
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
