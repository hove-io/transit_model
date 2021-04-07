// Copyright (C) 2017 Kisio Digital and/or its affiliates.
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by the
// Free Software Foundation, version 3.

// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.

// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>

//! The `transit_model` crate proposes a model to manage transit data.
//! It can import and export data from
//! [GTFS](https://gtfs.org/reference/static) and
//! [NTFS](https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fr.md).
//!
//! # Features
//! `transit_model` has 2 possible features: `proj` and `xmllint`.
//!
//! ## `proj`
//! `proj` feature is used for geolocation conversion (see
//! [Proj]). `proj` feature is used, for example, to export NeTEx France format.
//!
//! [Proj]: https://proj.org
//!
//! ## `xmllint`
//! Most likely, you don't need this feature as it's only used for additional
//! tests. It doesn't add any functionality to `transit_model`. If you're a
//! contributor to the project, you might be interested to run these tests. In
//! this case, take a look at the [`CONTRIBUTING.md`] for more information on
//! this feature.
//!
//! ## `mutable-model`
//! This is an experimental feature that allows you to get some abilities to
//! mutate a `Model`. It might not be completely stable at the moment so use
//! with care (or not at all!).
//!
//! [`CONTRIBUTING.md`]: https://github.com/CanalTP/transit_model/blob/master/CONTRIBUTING.md

#![deny(missing_docs)]

#[macro_use]
mod utils;
mod add_prefix;
pub use add_prefix::{AddPrefix, PrefixConfiguration};
pub mod calendars;
#[macro_use]
pub mod objects;
mod enhancers;
pub mod gtfs;
pub mod model;
#[cfg(feature = "proj")]
pub mod netex_france;
pub mod netex_utils;
pub mod ntfs;
pub mod read_utils;
#[doc(hidden)]
pub mod test_utils;
pub mod transfers;
pub mod validity_period;
mod version_utils;
pub mod vptranslator;

/// Current version of the NTFS format
pub const NTFS_VERSION: &str = "0.11.4";

/// The max distance in meters to compute the transfer
pub const TRANSFER_MAX_DISTANCE: &str = "300";

/// The walking speed in meters per second
pub const TRANSFER_WAKING_SPEED: &str = "0.785";

/// Waiting time at stop in second
pub const TRANSFER_WAITING_TIME: &str = "60";

lazy_static::lazy_static! {
    /// Current datetime
    pub static ref CURRENT_DATETIME: String = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
}

/// The error type used by the crate.
pub type Error = failure::Error;

/// The corresponding result type used by the crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

pub use crate::model::Model;

pub use crate::version_utils::{binary_full_version, GIT_VERSION};
