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
    objects::{self, Contributor},
    Result,
};
use failure::{format_err, ResultExt};
use log::info;
use serde::Deserialize;
use serde_json;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::path;
use std::path::{Path, PathBuf};
use std::result::Result as StdResult;
use transit_model_collection::{CollectionWithId, Id};

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

pub fn set_dataset_validity_period(
    dataset: &mut objects::Dataset,
    calendars: &CollectionWithId<objects::Calendar>,
) -> Result<()> {
    let validity_period = get_validity_period(calendars);

    if let Some(vp) = validity_period {
        dataset.start_date = vp.start_date;
        dataset.end_date = vp.end_date;
    }

    Ok(())
}

pub trait FileHandler
where
    Self: std::marker::Sized,
{
    type Reader: std::io::Read;

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
pub struct PathFileHandler<P: AsRef<Path>> {
    base_path: P,
}

impl<P: AsRef<Path>> PathFileHandler<P> {
    pub fn new(path: P) -> Self {
        PathFileHandler { base_path: path }
    }
}

impl<'a, P: AsRef<Path>> FileHandler for &'a mut PathFileHandler<P> {
    type Reader = File;
    fn get_file_if_exists(self, name: &str) -> Result<(Option<Self::Reader>, PathBuf)> {
        let f = self.base_path.as_ref().join(name);
        if f.exists() {
            Ok((Some(File::open(&f).with_context(ctx_from_path!(&f))?), f))
        } else {
            Ok((None, f))
        }
    }
}

/// ZipHandler is a wrapper around a ZipArchive
/// It provides a way to access the archive's file by their names
///
/// Unlike ZipArchive, it gives access to a file by its name not regarding its path in the ZipArchive
/// It thus cannot be correct if there are 2 files with the same name in the archive,
/// but for transport data if will make it possible to handle a zip with a sub directory
pub struct ZipHandler {
    archive: zip::ZipArchive<File>,
    archive_path: PathBuf,
    index_by_name: BTreeMap<String, usize>,
}

impl ZipHandler {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path.as_ref())?;
        let mut archive = zip::ZipArchive::new(file)?;
        Ok(ZipHandler {
            index_by_name: Self::files_by_name(&mut archive),
            archive,
            archive_path: path.as_ref().to_path_buf(),
        })
    }

    fn files_by_name(archive: &mut zip::ZipArchive<File>) -> BTreeMap<String, usize> {
        (0..archive.len())
            .filter_map(|i| {
                let file = archive.by_index(i).ok()?;
                // we get the name of the file, not regarding its path in the ZipArchive
                let real_name = Path::new(file.name()).file_name()?;
                let real_name: String = real_name.to_str()?.into();
                Some((real_name, i))
            })
            .collect()
    }
}

impl<'a> FileHandler for &'a mut ZipHandler {
    type Reader = zip::read::ZipFile<'a>;
    fn get_file_if_exists(self, name: &str) -> Result<(Option<Self::Reader>, PathBuf)> {
        let p = self.archive_path.join(name);
        match self.index_by_name.get(name) {
            None => Ok((None, p)),
            Some(i) => Ok((Some(self.archive.by_index(*i)?), p)),
        }
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
