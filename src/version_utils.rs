// Copyright (C) 2017 Hove and/or its affiliates.
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
//! Some utility to get the transit_model version

/// Precise git version of transit_model
/// the version will be:
/// v{last github tag}-{commit number}-{commit hash}{"-modified" if some changes have been done since last commit}
pub const GIT_VERSION: &str =
    git_version::git_version!(args = ["--tags", "--dirty=-modified"], fallback = "unknown");

/// get the binary version and the transit_model version
pub fn binary_full_version(binary_version: &str) -> String {
    format!("{binary_version} (transit_model = {GIT_VERSION})")
}
