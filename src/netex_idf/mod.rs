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

//! Module to handle Netex IDF profile

mod calendars;
mod common;
mod lines;
mod modes;
mod offers;
mod read;
mod share;
mod stops;

pub(crate) const EUROPE_PARIS_TIMEZONE: &str = "Europe/Paris";

pub use read::read;
