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

extern crate csv;
#[macro_use]
extern crate derivative;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate get_corresponding_derive;
#[macro_use]
extern crate log;
extern crate serde;
#[macro_use]
extern crate serde_derive;

extern crate chrono;
#[macro_use]
pub(crate) mod utils;
pub mod collection;
pub(crate) mod common_format;
pub mod gtfs;
pub mod model;
pub mod ntfs;
pub mod objects;
pub mod relations;

pub type Error = failure::Error;
pub type Result<T> = std::result::Result<T, Error>;

pub use model::Model;
