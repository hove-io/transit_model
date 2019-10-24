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

//! KV1 format management.

mod read;

use crate::{
    model::{Collections, Model},
    read_utils, AddPrefix, Result,
};
use std::path::Path;
use transit_model_collection::CollectionWithId;

fn read<H>(
    file_handler: &mut H,
    config_path: Option<impl AsRef<Path>>,
    prefix: Option<String>,
) -> Result<Model>
where
    for<'a> &'a mut H: read_utils::FileHandler,
{
    let mut collections = Collections::default();

    read::read_operday(file_handler, &mut collections)?;

    let (contributor, mut dataset, feed_infos) = read_utils::read_config(config_path)?;
    read_utils::set_dataset_validity_period(&mut dataset, &collections.calendars)?;

    collections.contributors = CollectionWithId::new(vec![contributor])?;
    collections.datasets = CollectionWithId::new(vec![dataset])?;
    collections.feed_infos = feed_infos;

    read::read_usrstop_point(file_handler, &mut collections)?;
    read::read_notice(file_handler, &mut collections)?;
    read::read_jopa_pujopass_line(file_handler, &mut collections)?;

    //add prefixes
    if let Some(prefix) = prefix {
        collections.add_prefix_with_sep(prefix.as_str(), ":");
    }

    collections.enhance_with_co2();
    Ok(Model::new(collections)?)
}

/// Imports a `Model` from the KV1 files in the `path` directory.
///
/// The `config_path` argument allows you to give a path to a file
/// containing a json representing the contributor and dataset used
/// for this KV1. If not given, default values will be created.
///
/// The `prefix` argument is a string that will be prepended to every
/// identifiers, allowing to namespace the dataset. By default, no
/// prefix will be added to the identifiers.
pub fn read_from_path<P: AsRef<Path>>(
    p: P,
    config_path: Option<P>,
    prefix: Option<String>,
) -> Result<Model> {
    let mut file_handle = read_utils::PathFileHandler::new(p.as_ref().to_path_buf());
    read(&mut file_handle, config_path, prefix)
}

/// Imports a `Model` from a zip file containing the KV1.
///
/// The `config_path` argument allows you to give a path to a file
/// containing a json representing the contributor and dataset used
/// for this KV1. If not given, default values will be created.
///
/// The `prefix` argument is a string that will be prepended to every
/// identifiers, allowing to namespace the dataset. By default, no
/// prefix will be added to the identifiers.
pub fn read_from_zip<P: AsRef<Path>>(
    path: P,
    config_path: Option<P>,
    prefix: Option<String>,
) -> Result<Model> {
    let mut file_handler = read_utils::ZipHandler::new(path)?;
    read(&mut file_handler, config_path, prefix)
}
