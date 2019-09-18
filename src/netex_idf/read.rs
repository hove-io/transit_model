// Copyright 2017 Kisio Digital and/or its affiliates.
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

use crate::{
    model::{Collections, Model},
    AddPrefix, Result,
};
use std::path::Path;
use transit_model_collection::CollectionWithId;

/// Read Netex IDF format into a Navitia Transit Model
pub fn read<P>(_netex_idf_path: P, config_path: Option<P>, prefix: Option<String>) -> Result<Model>
where
    P: AsRef<Path>,
{
    let mut collections = Collections::default();
    let (contributor, dataset, feed_infos) = crate::read_utils::read_config(config_path)?;
    collections.contributors = CollectionWithId::from(contributor);
    collections.datasets = CollectionWithId::from(dataset);
    collections.feed_infos = feed_infos;

    // read netex files

    if let Some(prefix) = prefix {
        collections.add_prefix_with_sep(prefix.as_str(), ":");
    }
    collections.sanitize()?;
    Model::new(collections)
}
