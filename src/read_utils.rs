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

use crate::collection::{CollectionWithId, Id};
use crate::model::Collections;
use crate::objects::{self, Contributor};
use crate::utils::{add_prefix_to_collection, add_prefix_to_collection_with_id};
use crate::Result;
use failure::{format_err, ResultExt};
use log::info;
use serde_derive::Deserialize;
use serde_json;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::path;
use std::path::{Path, PathBuf};
use std::result::Result as StdResult;

#[derive(Deserialize, Debug)]
struct ConfigDataset {
    dataset_id: String,
}

#[derive(Deserialize, Debug)]
struct Config {
    contributor: objects::Contributor,
    dataset: ConfigDataset,
}

pub fn read_config<P: AsRef<path::Path>>(
    config_path: Option<P>,
) -> Result<(objects::Contributor, objects::Dataset)> {
    let contributor;
    let dataset;
    if let Some(config_path) = config_path {
        let json_config_file = File::open(config_path)?;
        let config: Config = serde_json::from_reader(json_config_file)?;
        info!("Reading dataset and contributor from config: {:?}", config);

        contributor = config.contributor;
        dataset = objects::Dataset::new(config.dataset.dataset_id, contributor.id.clone());
    } else {
        contributor = Contributor::default();
        dataset = objects::Dataset::default();
    }
    Ok((contributor, dataset))
}

pub fn add_prefix(prefix: String, collections: &mut Collections) -> Result<()> {
    let prefix = prefix + ":";
    info!("Adding prefix: \"{}\"", &prefix);
    add_prefix_to_collection_with_id(&mut collections.commercial_modes, &prefix)?;
    add_prefix_to_collection_with_id(&mut collections.networks, &prefix)?;
    add_prefix_to_collection_with_id(&mut collections.companies, &prefix)?;
    add_prefix_to_collection_with_id(&mut collections.stop_points, &prefix)?;
    add_prefix_to_collection_with_id(&mut collections.stop_areas, &prefix)?;
    add_prefix_to_collection(&mut collections.transfers, &prefix);
    add_prefix_to_collection_with_id(&mut collections.routes, &prefix)?;
    add_prefix_to_collection_with_id(&mut collections.lines, &prefix)?;
    add_prefix_to_collection_with_id(&mut collections.contributors, &prefix)?;
    add_prefix_to_collection_with_id(&mut collections.datasets, &prefix)?;
    add_prefix_to_collection_with_id(&mut collections.vehicle_journeys, &prefix)?;
    add_prefix_to_collection_with_id(&mut collections.trip_properties, &prefix)?;
    add_prefix_to_collection_with_id(&mut collections.equipments, &prefix)?;
    add_prefix_to_collection_with_id(&mut collections.comments, &prefix)?;
    add_prefix_to_collection_with_id(&mut collections.geometries, &prefix)?;
    add_prefix_to_collection_with_id(&mut collections.calendars, &prefix)?;

    Ok(())
}

pub fn get_validity_period(
    calendars: &CollectionWithId<objects::Calendar>,
) -> Option<objects::ValidityPeriod> {
    let dates = calendars.values().fold(BTreeSet::new(), |acc, c| {
        acc.union(&c.dates).cloned().collect()
    });

    if dates.is_empty() {
        return None;
    }

    Some(objects::ValidityPeriod {
        start_date: *dates.iter().next().unwrap(),
        end_date: *dates.iter().next_back().unwrap(),
    })
}

pub trait FileHandler
where
    Self: std::marker::Sized,
{
    type Reader: std::io::Read;

    fn is_file(self, name: &str) -> bool;
    fn get_file_if_exists(self, name: &str) -> Result<(Option<Self::Reader>, PathBuf)>;

    fn get_file(self, name: &str) -> Result<(Self::Reader, PathBuf)> {
        let (reader, path) = self.get_file_if_exists(name)?;
        Ok((
            reader.ok_or_else(|| format_err!("file {:?} not found", path))?,
            path,
        ))
    }
}

/// PathFileHandler is used to read files for a directory
pub struct PathFileHandler {
    base_path: PathBuf,
}

impl PathFileHandler {
    pub fn new(path: PathBuf) -> Self {
        PathFileHandler { base_path: path }
    }
}

impl<'a> FileHandler for &'a mut PathFileHandler {
    type Reader = File;
    fn get_file_if_exists(self, name: &str) -> Result<(Option<Self::Reader>, PathBuf)> {
        let f = self.base_path.join(name);
        if f.exists() {
            Ok((Some(File::open(&f).with_context(ctx_from_path!(&f))?), f))
        } else {
            Ok((None, f))
        }
    }
    fn is_file(self, name: &str) -> bool {
        self.base_path.join(name).is_file()
    }
}

/// ZipHandler is a wrapper around a ZipArchive
/// It provides a way to access the archive's file by their names
///
/// Unlike ZipArchive, it gives access to a file by it's name not regarding it's path in the ZipArchive
/// It thus cannot be correct if there are 2 files with the same name in the archive,
/// but for transport data if will make it possible to handle a zip with a sub directory
pub struct ZipHandler<R: std::io::Seek + std::io::Read> {
    archive: zip::ZipArchive<R>,
    archive_path: PathBuf,
    index_by_name: BTreeMap<String, usize>,
}

impl<R> ZipHandler<R>
where
    R: std::io::Seek + std::io::Read,
{
    pub fn new<P: AsRef<Path>>(r: R, path: P) -> Result<Self> {
        let mut archive = zip::ZipArchive::new(r)?;
        Ok(ZipHandler {
            index_by_name: Self::files_by_name(&mut archive),
            archive: archive,
            archive_path: path.as_ref().to_path_buf(),
        })
    }

    fn files_by_name(archive: &mut zip::ZipArchive<R>) -> BTreeMap<String, usize> {
        (0..archive.len())
            .filter_map(|i| {
                let file = archive.by_index(i).ok()?;
                // we get the name of the file, not regarding it's patch in the ZipArchive
                let real_name = Path::new(file.name()).file_name()?;
                let real_name: String = real_name.to_str()?.into();
                Some((real_name, i))
            })
            .collect()
    }
}

impl<'a, R> FileHandler for &'a mut ZipHandler<R>
where
    R: std::io::Seek + std::io::Read,
{
    type Reader = zip::read::ZipFile<'a>;
    fn get_file_if_exists(self, name: &str) -> Result<(Option<Self::Reader>, PathBuf)> {
        let p = self.archive_path.join(name);
        match self.index_by_name.get(name) {
            None => Ok((None, p)),
            Some(i) => Ok((Some(self.archive.by_index(*i)?), p)),
        }
    }
    fn is_file(self, name: &str) -> bool {
        self.index_by_name.get(name).is_some()
    }
}

/// Read a vector of objects from a zip in a file_handler
pub fn read_objects<H, O>(file_handler: &mut H, file_name: &str) -> Result<Vec<O>>
where
    for<'a> &'a mut H: FileHandler,
    O: for<'de> serde::Deserialize<'de>,
{
    let (reader, path) = file_handler.get_file(file_name)?;

    let mut rdr = csv::Reader::from_reader(reader);
    Ok(rdr
        .deserialize()
        .collect::<StdResult<_, _>>()
        .with_context(ctx_from_path!(path))?)
}

/// Read a CollectionId from a zip in a file_handler
pub fn read_collection<H, O>(file_handler: &mut H, file_name: &str) -> Result<CollectionWithId<O>>
where
    for<'a> &'a mut H: FileHandler,
    O: for<'de> serde::Deserialize<'de> + Id<O>,
{
    let vec = read_objects(file_handler, file_name)?;
    CollectionWithId::new(vec)
}

/// Read an URL and get a cursor on the hosted file
pub fn read_url(url: &str) -> Result<std::io::Cursor<Vec<u8>>> {
    let mut res = reqwest::get(url)?;
    let mut body = Vec::new();
    res.read_to_end(&mut body)?;
    Ok(std::io::Cursor::new(body))
}
