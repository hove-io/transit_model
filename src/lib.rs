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
//! It can import and export data from [GTFS](http://gtfs.org/) and
//! [NTFS](https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fr.md).

#![deny(missing_docs)]

#[macro_use]
mod utils;
mod add_prefix;
pub use add_prefix::AddPrefix;
pub mod apply_rules;
pub mod calendars;
#[macro_use]
pub mod objects;
pub mod gtfs;
pub mod hellogo_fares;
pub mod merge_stop_areas;
pub mod model;
#[cfg(feature = "proj")]
pub mod netex_france;
pub mod netex_utils;
pub mod ntfs;
pub mod read_utils;
pub mod report;
#[doc(hidden)]
pub mod test_utils;
pub mod transfers;
pub mod validity_period;
pub mod vptranslator;

/// Current version of the NTFS format
pub const NTFS_VERSION: &str = "0.11.2";

lazy_static::lazy_static! {
    /// Current datetime
    pub static ref CURRENT_DATETIME: String = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
}

/// The error type used by the crate.
pub type Error = failure::Error;

/// The corresponding result type used by the crate.
pub type Result<T> = std::result::Result<T, Error>;

pub use crate::model::Model;
