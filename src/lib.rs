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

//! The `navitia_model` crate proposes a model to manage transit data.
//! It can import and export data from [GTFS](http://gtfs.org/) and
//! [NTFS](https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/ntfs_fr.md).

#![deny(missing_docs)]

#[macro_use]
pub(crate) mod utils;
pub mod apply_rules;
pub mod collection;
pub(crate) mod common_format;
pub mod gtfs;
pub mod merge_stop_areas;
pub mod model;
pub mod netex;
pub mod ntfs;
pub mod objects;
mod read_utils;
pub mod relations;
#[doc(hidden)]
pub mod test_utils;
pub mod transfers;

#[cfg(test)]
#[path = "../model-builder/src/builder.rs"]
pub mod model_builder;

/// The error type used by the crate.
pub type Error = failure::Error;

/// The corresponding result type used by the crate.
pub type Result<T> = std::result::Result<T, Error>;

pub use crate::model::Model;
