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
//! Some utilities for input dataset to the library.

use crate::{
    file_handler::FileHandler,
    objects::{self, Contributor},
    Result,
};
use anyhow::{anyhow, bail, Context};
use serde::Deserialize;
use skip_error::SkipError;
use std::collections::BTreeMap;
use std::fs::File;
use std::path;
use tracing::info;
use typed_index_collection::{CollectionWithId, Id};

#[derive(Deserialize, Debug)]
struct ConfigDataset {
    dataset_id: String,
}

#[derive(Deserialize, Debug)]
struct Config {
    contributor: objects::Contributor,
    dataset: ConfigDataset,
    feed_infos: Option<BTreeMap<String, String>>,
}

/// Read a JSON configuration file to facilitate the creation of:
/// - a Contributor
/// - a Dataset
/// - a list of key/value which will be used in 'feed_infos.txt'
/// Below is an example of this file
/// ```text
/// {
///     "contributor": {
///         "contributor_id": "contributor_id",
///         "contributor_name": "Contributor Name",
///         "contributor_license": "AGPIT",
///         "contributor_website": "http://www.datasource-website.com"
///     },
///     "dataset": {
///         "dataset_id": "dataset-id"
///     },
///     "feed_infos": {
///         "feed_publisher_name": "The Great Data Publisher",
///         "feed_license": "AGPIT",
///         "feed_license_url": "http://www.datasource-website.com",
///         "tartare_platform": "dev",
///         "tartare_contributor_id": "contributor_id"
///     }
/// }
/// ```
pub fn read_config<P: AsRef<path::Path>>(
    config_path: Option<P>,
) -> Result<(
    objects::Contributor,
    objects::Dataset,
    BTreeMap<String, String>,
)> {
    let contributor;
    let dataset;
    let mut feed_infos = BTreeMap::default();

    if let Some(config_path) = config_path {
        let config_path = config_path.as_ref();
        info!("Reading dataset and contributor from {:?}", config_path);
        let json_config_file = File::open(config_path)?;
        let config: Config = serde_json::from_reader(json_config_file)?;

        contributor = config.contributor;
        dataset = objects::Dataset::new(config.dataset.dataset_id, contributor.id.clone());
        if let Some(config_feed_infos) = config.feed_infos {
            feed_infos = config_feed_infos;
        }
    } else {
        contributor = Contributor::default();
        dataset = objects::Dataset::default();
    }

    Ok((contributor, dataset, feed_infos))
}

/// Read a vector of objects from a zip in a file_handler
pub(crate) fn _read_objects<H, O>(
    file_handler: &mut H,
    file_name: &str,
    required_file: bool,
) -> Result<Vec<O>>
where
    for<'a> &'a mut H: FileHandler,
    O: for<'de> serde::Deserialize<'de>,
{
    let (reader, path) = file_handler.get_file_if_exists(file_name)?;
    let file_name = path.file_name();
    let basename = file_name.map_or(path.to_string_lossy(), |b| b.to_string_lossy());

    match (reader, required_file) {
        (None, false) => {
            info!("Skipping {}", basename);
            Ok(vec![])
        }
        (None, true) => {
            bail!("file {:?} not found", path)
        }
        (Some(reader), _) => {
            info!("Reading {}", basename);
            let mut rdr = csv::ReaderBuilder::new()
                .flexible(true)
                .trim(csv::Trim::All)
                .from_reader(reader);
            Ok(rdr
                .deserialize()
                .collect::<Result<_, _>>()
                .with_context(|| format!("Error reading {:?}", path))?)
        }
    }
}

#[cfg(not(feature = "parser"))]
pub(crate) fn read_objects<H, O>(
    file_handler: &mut H,
    file_name: &str,
    required_file: bool,
) -> Result<Vec<O>>
where
    for<'a> &'a mut H: FileHandler,
    O: for<'de> serde::Deserialize<'de>,
{
    _read_objects(file_handler, file_name, required_file)
}

#[cfg(feature = "parser")]
/// See function _read_objects
pub fn read_objects<H, O>(
    file_handler: &mut H,
    file_name: &str,
    required_file: bool,
) -> Result<Vec<O>>
where
    for<'a> &'a mut H: FileHandler,
    O: for<'de> serde::Deserialize<'de>,
{
    _read_objects(file_handler, file_name, required_file)
}

/// Read a vector of objects from a zip in a file_handler ignoring error
pub(crate) fn _read_objects_loose<H, O>(
    file_handler: &mut H,
    file_name: &str,
    required_file: bool,
) -> Result<Vec<O>>
where
    for<'a> &'a mut H: FileHandler,
    O: for<'de> serde::Deserialize<'de>,
{
    let (reader, path) = file_handler.get_file_if_exists(file_name)?;
    let file_name = path.file_name();
    let basename = file_name.map_or(path.to_string_lossy(), |b| b.to_string_lossy());

    match (reader, required_file) {
        (None, false) => {
            info!("Skipping {}", basename);
            Ok(vec![])
        }
        (None, true) => {
            bail!("file {:?} not found", path)
        }
        (Some(reader), _) => {
            info!("Reading {}", basename);
            let mut rdr = csv::ReaderBuilder::new()
                .flexible(true)
                .trim(csv::Trim::All)
                .from_reader(reader);
            let objects = rdr
                .deserialize()
                .map(|object| object.with_context(|| format!("Error reading {:?}", path)))
                .skip_error_and_warn()
                .collect();
            Ok(objects)
        }
    }
}

#[cfg(not(feature = "parser"))]
pub(crate) fn read_objects_loose<H, O>(
    file_handler: &mut H,
    file_name: &str,
    required_file: bool,
) -> Result<Vec<O>>
where
    for<'a> &'a mut H: FileHandler,
    O: for<'de> serde::Deserialize<'de>,
{
    _read_objects_loose(file_handler, file_name, required_file)
}

#[cfg(feature = "parser")]
/// See function _read_objects_loose
pub fn read_objects_loose<H, O>(
    file_handler: &mut H,
    file_name: &str,
    required_file: bool,
) -> Result<Vec<O>>
where
    for<'a> &'a mut H: FileHandler,
    O: for<'de> serde::Deserialize<'de>,
{
    _read_objects_loose(file_handler, file_name, required_file)
}

/// Read a CollectionId from a required file in a file_handler
pub(crate) fn _read_collection<H, O>(
    file_handler: &mut H,
    file_name: &str,
) -> Result<CollectionWithId<O>>
where
    for<'a> &'a mut H: FileHandler,
    O: for<'de> serde::Deserialize<'de> + Id<O>,
{
    let vec = read_objects(file_handler, file_name, true)?;
    CollectionWithId::new(vec).map_err(|e| anyhow!("{}", e))
}

#[cfg(not(feature = "parser"))]
pub(crate) fn read_collection<H, O>(
    file_handler: &mut H,
    file_name: &str,
) -> Result<CollectionWithId<O>>
where
    for<'a> &'a mut H: FileHandler,
    O: for<'de> serde::Deserialize<'de> + Id<O>,
{
    _read_collection(file_handler, file_name)
}

#[cfg(feature = "parser")]
/// See function _read_collection
pub fn read_collection<H, O>(file_handler: &mut H, file_name: &str) -> Result<CollectionWithId<O>>
where
    for<'a> &'a mut H: FileHandler,
    O: for<'de> serde::Deserialize<'de> + Id<O>,
{
    _read_collection(file_handler, file_name)
}

/// Read a CollectionId from a optional file in a file_handler
pub(crate) fn _read_opt_collection<H, O>(
    file_handler: &mut H,
    file_name: &str,
) -> Result<CollectionWithId<O>>
where
    for<'a> &'a mut H: FileHandler,
    O: for<'de> serde::Deserialize<'de> + Id<O>,
{
    let vec = read_objects(file_handler, file_name, false)?;
    CollectionWithId::new(vec).map_err(|e| anyhow!("{}", e))
}

#[cfg(not(feature = "parser"))]
pub(crate) fn read_opt_collection<H, O>(
    file_handler: &mut H,
    file_name: &str,
) -> Result<CollectionWithId<O>>
where
    for<'a> &'a mut H: FileHandler,
    O: for<'de> serde::Deserialize<'de> + Id<O>,
{
    _read_opt_collection(file_handler, file_name)
}

#[cfg(feature = "parser")]
/// See function _read_opt_collection
pub fn read_opt_collection<H, O>(
    file_handler: &mut H,
    file_name: &str,
) -> Result<CollectionWithId<O>>
where
    for<'a> &'a mut H: FileHandler,
    O: for<'de> serde::Deserialize<'de> + Id<O>,
{
    _read_opt_collection(file_handler, file_name)
}
