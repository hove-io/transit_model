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

//! [Netex](http://netex-cen.eu/) format management.

mod read;

use model::{Collections, Model};
use std::path::Path;
use utils::{add_prefix_to_collection, add_prefix_to_collection_with_id};
use std::fs;
use Result;
extern crate tempdir;
use self::tempdir::TempDir;



fn add_prefix(prefix: String, collections: &mut Collections) -> Result<()> {
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

    Ok(())
}

/// Imports a `Model` from the [Netex](http://netex-cen.eu/) files in the
/// `path` directory. Refers to the [Netex Github repo](https://github.com/NeTEx-CEN/NeTEx/)
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
    
    let mut collections = Collections::default();
    for entry in fs::read_dir(path)? {
        let file = entry?;
        if file.path().extension().unwrap() == "xml" {
            read::read_netex_file(&mut collections, file.path().as_path());
        }
    }

    let (contributors, datasets) = read::read_config(config_path)?;
    collections.contributors = contributors;
    collections.datasets = datasets;

    //add prefixes
    if let Some(prefix) = prefix {
        add_prefix(prefix, &mut collections)?;
    }

    Ok(Model::new(collections)?)
}

/// This function is a shortcut to call the read function on all files of a zip archive. 
pub fn read_from_zip<P>(zip_file: P, config_path: Option<P>, prefix: Option<String>) -> Result<Model>
where
    P: AsRef<Path>,
{
    info!("Reading Netex data from ZIP file {:?}", zip_file.as_ref());
    // let input_tmp_dir = TempDir::new("netex_input").unwrap();
    let input_tmp_dir = Path::new("fixtures/netex/RATP_Line7bis-extract-2009-NeTEx/input_tmp");
    // ::utils::unzip_to(zip_file.as_ref(), input_tmp_dir.path());
    ::utils::unzip_to(zip_file.as_ref(), input_tmp_dir);
    // let config_path = match config_path {
    //     None => None,
    //     Some(c) => Some(c.as_ref().clone())
    // };
    // let config_path = config_path.map(|c| c.as_ref().to_owned().as_ref()).clone(); 
    read(input_tmp_dir, None, prefix)
}
