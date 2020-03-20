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
    read_utils, validity_period, AddPrefix, Result,
};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};
use tempfile::TempDir;
use transit_model_collection::CollectionWithId;

/// Imports a `Model` from the KV1 files in the `path` directory.
///
/// The `config_path` argument allows you to give a path to a file
/// containing a json representing the contributor and dataset used
/// for this KV1. If not given, default values will be created.
///
/// The `prefix` argument is a string that will be prepended to every
/// identifiers, allowing to namespace the dataset. By default, no
/// prefix will be added to the identifiers.
pub fn read_from_path<P: AsRef<Path>, Q: AsRef<Path>>(
    path: P,
    config_path: Option<Q>,
    prefix: Option<String>,
) -> Result<Model> {
    let mut collections = Collections::default();

    read::read_operday(&path, &mut collections)?;

    let (contributor, mut dataset, feed_infos) = read_utils::read_config(config_path)?;
    validity_period::set_dataset_validity_period(&mut dataset, &collections.calendars)?;

    collections.contributors = CollectionWithId::new(vec![contributor])?;
    collections.datasets = CollectionWithId::new(vec![dataset])?;
    collections.feed_infos = feed_infos;

    read::read_usrstop_point(&path, &mut collections)?;
    read::read_notice(&path, &mut collections)?;
    read::read_jopa_pujopass_line(&path, &mut collections)?;

    //add prefixes
    if let Some(prefix) = prefix {
        collections.add_prefix_with_sep(prefix.as_str(), ":");
    }

    collections.calendar_deduplication();
    Model::new(collections)
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
pub fn read_from_zip<P: AsRef<Path>, Q: AsRef<Path>>(
    path: P,
    config_path: Option<Q>,
    prefix: Option<String>,
) -> Result<Model> {
    let file = File::open(path.as_ref())?;
    let mut archive = zip::ZipArchive::new(file)?;
    let unzipped_folder = TempDir::new()?;
    for file_index in 0..archive.len() {
        let mut file = archive.by_index(file_index)?;
        if file.is_file() {
            let unziped_filepath = unzipped_folder.as_ref().join(file.sanitized_name());
            let mut unziped_file = File::create(unziped_filepath)?;
            let mut buffer = [0u8; 8];
            loop {
                let read_bytes = file.read(&mut buffer)?;
                if read_bytes != 0 {
                    unziped_file.write_all(&buffer[0..read_bytes])?;
                } else {
                    break;
                }
            }
        }
    }
    read_from_path(unzipped_folder, config_path, prefix)
}
