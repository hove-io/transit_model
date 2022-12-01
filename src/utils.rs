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

use crate::{file_handler::FileHandler, parser::read_objects};
use anyhow::Context;
use skip_error::skip_error_and_warn;
use std::{
    fs,
    io::{Read, Write},
    path,
};
use tracing::{debug, info};
use typed_index_collection::{Collection, CollectionWithId, Id};
use walkdir::WalkDir;

pub fn zip_to<P, R>(source_path: P, zip_file: R) -> crate::Result<()>
where
    P: AsRef<path::Path>,
    R: AsRef<path::Path>,
{
    let source_path = source_path.as_ref();
    let file = fs::File::create(zip_file.as_ref())?;
    let mut zip = zip::ZipWriter::new(file);
    let options =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut buffer = Vec::new();
    for entry in WalkDir::new(source_path) {
        let path = entry?.path().to_owned();
        if path.is_file() {
            let name = path.strip_prefix(path::Path::new(source_path))?.to_owned();
            if let Some(name) = name.to_str() {
                debug!("adding {:?} as {:?} ...", path, name);
                zip.start_file(name, options)?;
                let mut f = fs::File::open(path)?;

                f.read_to_end(&mut buffer)?;
                zip.write_all(&buffer)?;
                buffer.clear();
            }
        }
    }
    zip.finish()?;
    Ok(())
}

pub(crate) fn make_collection_with_id<T, H>(
    file_handler: &mut H,
    file: &str,
) -> crate::Result<CollectionWithId<T>>
where
    for<'de> T: Id<T> + serde::Deserialize<'de>,
    for<'a> &'a mut H: FileHandler,
{
    let mut collection = CollectionWithId::<T>::default();
    for object in read_objects::<_, T>(file_handler, file, true)? {
        skip_error_and_warn!(collection.push(object));
    }
    Ok(collection)
}

pub(crate) fn make_opt_collection<T, H>(
    file_handler: &mut H,
    file: &str,
) -> crate::Result<Collection<T>>
where
    for<'de> T: serde::Deserialize<'de>,
    for<'a> &'a mut H: FileHandler,
{
    let vec = read_objects::<_, T>(file_handler, file, false)?;
    Ok(Collection::new(vec))
}

pub(crate) fn make_opt_collection_with_id<T, H>(
    file_handler: &mut H,
    file: &str,
) -> crate::Result<CollectionWithId<T>>
where
    for<'de> T: Id<T> + serde::Deserialize<'de>,
    for<'a> &'a mut H: FileHandler,
{
    let mut collection = CollectionWithId::<T>::default();
    for object in read_objects::<_, T>(file_handler, file, false)? {
        skip_error_and_warn!(collection.push(object));
    }
    Ok(collection)
}

pub fn write_collection_with_id<T>(
    path: &path::Path,
    file: &str,
    collection: &CollectionWithId<T>,
) -> crate::Result<()>
where
    T: Id<T> + serde::Serialize,
{
    if collection.is_empty() {
        return Ok(());
    }
    info!("Writing {}", file);
    let path = path.join(file);
    let mut wtr =
        csv::Writer::from_path(&path).with_context(|| format!("Error reading {:?}", path))?;
    for obj in collection.values() {
        wtr.serialize(obj)
            .with_context(|| format!("Error reading {:?}", path))?;
    }
    wtr.flush()
        .with_context(|| format!("Error reading {:?}", path))?;

    Ok(())
}

pub fn write_collection<T>(
    path: &path::Path,
    file: &str,
    collection: &Collection<T>,
) -> crate::Result<()>
where
    T: serde::Serialize,
{
    if collection.is_empty() {
        return Ok(());
    }
    info!("Writing {}", file);
    let path = path.join(file);
    let mut wtr =
        csv::Writer::from_path(&path).with_context(|| format!("Error reading {:?}", path))?;
    for obj in collection.values() {
        wtr.serialize(obj)
            .with_context(|| format!("Error reading {:?}", path))?;
    }
    wtr.flush()
        .with_context(|| format!("Error reading {:?}", path))?;

    Ok(())
}
