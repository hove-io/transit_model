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

//! Some utilities to use Minidom and returns [Result](crate::Result) when
//! parsing instead of [Option](Option)

mod try_attribute;
pub use try_attribute::TryAttribute;
mod try_only_child;
pub use try_only_child::TryOnlyChild;
mod writer;
pub use self::writer::ElementWriter;
