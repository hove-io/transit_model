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

use crate::{file_handler::FileHandler, Result};
use anyhow::{anyhow, bail, Context};
use skip_error::SkipError;
use tracing::info;
use typed_index_collection::{CollectionWithId, Id};

/// Read a vector of objects from a zip in a file_handler
pub fn read_objects<H, O>(
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

/// Read a vector of objects from a zip in a file_handler ignoring error
pub fn read_objects_loose<H, O>(
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
/// Read a CollectionId from a zip in a file_handler
pub fn read_collection<H, O>(file_handler: &mut H, file_name: &str) -> Result<CollectionWithId<O>>
where
    for<'a> &'a mut H: FileHandler,
    O: for<'de> serde::Deserialize<'de> + Id<O>,
{
    let vec = read_objects(file_handler, file_name, true)?;
    CollectionWithId::new(vec).map_err(|e| anyhow!("{}", e))
}

/// Read a CollectionId from a optional file in a file_handler
pub fn read_opt_collection<H, O>(
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
