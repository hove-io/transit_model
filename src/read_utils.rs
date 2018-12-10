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

use collection::CollectionWithId;
use model::Collections;
use objects::{self, Contributor};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::path;
use utils::{add_prefix_to_collection, add_prefix_to_collection_with_id};
use Result;
extern crate serde_json;
use failure::ResultExt;
use std::path::{Path, PathBuf};

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

pub trait FileHandler<'a, R: 'a>
where
    R: std::io::Read,
{
    fn get_file(&'a mut self, name: &str) -> Result<R>;
}

pub fn open_file<P: AsRef<path::Path>>(
    path: P,
    name: &str,
) -> std::result::Result<File, failure::Context<String>> {
    let f = path.as_ref().join(name);
    File::open(&f).with_context(ctx_from_path!(&f))
}

pub struct PathFileHandler {
    base_path: PathBuf,
}

impl PathFileHandler {
    pub fn new(path: PathBuf) -> Self {
        PathFileHandler { base_path: path }
    }
}

impl<'a> FileHandler<'a, File> for PathFileHandler {
    fn get_file(&'a mut self, name: &str) -> Result<File> {
        let f = self.base_path.join(name);
        Ok(File::open(&f).with_context(ctx_from_path!(&f))?)
    }
}

/// ZipHandler is a wrapper around a ZipArchive
/// It provides a way to access the archive's file by their names
///
/// Unlike ZipArchive, it gives access to a file by it's name not regarding it's path in the ZipArchive
pub struct ZipHandler<R: std::io::Seek + std::io::Read> {
    archive: zip::ZipArchive<R>,
    index_by_name: BTreeMap<String, usize>,
}

impl<R> ZipHandler<R>
where
    R: std::io::Seek + std::io::Read,
{
    pub fn new(r: R) -> Result<Self> {
        let mut archive = zip::ZipArchive::new(r)?;
        Ok(ZipHandler {
            index_by_name: Self::files_by_name(&mut archive),
            archive: archive,
        })
    }

    fn files_by_name(archive: &mut zip::ZipArchive<R>) -> BTreeMap<String, usize> {
        let mut res = BTreeMap::new();
        for i in 0..archive.len() {
            let file = archive.by_index(i).unwrap();
            // we get the name of the file, not regarding it's patch in the ZipArchive
            let real_name = Path::new(file.name()).file_name().unwrap();
            let real_name: String = real_name.to_str().unwrap().into();
            res.insert(real_name, i);
        }
        res
    }
}

impl<'a, R> FileHandler<'a, zip::read::ZipFile<'a>> for ZipHandler<R>
where
    R: std::io::Seek + std::io::Read,
{
    fn get_file(&'a mut self, name: &str) -> Result<zip::read::ZipFile<'a>> {
        // self.index_by_name
        //     .get(name)
        //     .map(|i| self.archive.by_index(i.clone()).unwrap())
        //     .ok_or(format_err!("impossible to find file {}", name))
        match self.index_by_name.get(name) {
            None => Err(format_err!("impossible to find file {}", name)),
            Some(i) => Ok(self.archive.by_index(*i).unwrap()),
        }
    }
}
