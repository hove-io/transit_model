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

//! [Netex](http://netex-cen.eu/) format management.

mod read;

use self::read::NetexReader;
use crate::collection::CollectionWithId;
use crate::model::Model;
use crate::read_utils;
use crate::Result;
use failure::bail;
use log::info;
use std::fs;
use std::path::Path;
use zip;

/// Imports a `Model` from one or several [Netex](http://netex-cen.eu/) files.
/// The `path` can be a single file, a directory or a zip file.
/// Refers to the [Netex Github repo](https://github.com/NeTEx-CEN/NeTEx/)
/// for details.
///
/// The `config_path` argument allows you to give a path to a file
/// containing a json representing the contributor and dataset used
/// for this Netex file. If not given, default values will be created.
///
/// The `prefix` argument is a string that will be prepended to every
/// identifiers, allowing to namespace the dataset. By default, no
/// prefix will be added to the identifiers.
pub fn read<P>(path: P, config_path: Option<P>, prefix: Option<String>) -> Result<Model>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    info!("Reading Netex data from {:?}", path);
    println!("Reading Netex data from {:?}", path);
    let mut netex_reader = NetexReader::default();
    if path.is_file() {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("zip") => {
                let zip_file = fs::File::open(path)?;
                let mut zip = zip::ZipArchive::new(zip_file)?;
                for i in 0..zip.len() {
                    let file = zip.by_index(i)?;
                    match file.sanitized_name().extension() {
                        Some(ext) if ext == "xml" => {
                            netex_reader.read_netex_file(file)?;
                        }
                        _ => {
                            info!(
                                "Netex read : skipping file in ZIP : {:?}",
                                file.sanitized_name()
                            );
                        }
                    }
                }
            }
            Some("xml") => netex_reader.read_netex_file(fs::File::open(path)?)?,
            _ => bail!("Provided netex file should be xml or zip : {:?}", path),
        };
    } else {
        for entry in fs::read_dir(path)? {
            let file = entry?;
            if file.path().extension().map_or(false, |ext| ext == "xml") {
                let file = fs::File::open(file.path())?;
                netex_reader.read_netex_file(file)?;
            } else {
                info!(
                    "Netex read : skipping file in directory : {:?}",
                    file.file_name()
                );
            }
        }
    };

    let (contributor, mut dataset) = read_utils::read_config(config_path)?;
    let vp = read_utils::get_validity_period(&netex_reader.collections.calendars);
    let vp = match vp {
        None => bail!("No valid calendar in Netex Data"),
        Some(vp) => vp,
    };
    dataset.start_date = vp.start_date;
    dataset.end_date = vp.end_date;
    dataset.system = Some("Netex".to_string());

    netex_reader.collections.contributors = CollectionWithId::new(vec![contributor])?;
    netex_reader.collections.datasets = CollectionWithId::new(vec![dataset])?;
    //add prefixes
    if let Some(prefix) = prefix {
        read_utils::add_prefix(prefix, &mut netex_reader.collections)?;
    }

    Ok(Model::new(netex_reader.collections)?)
}
