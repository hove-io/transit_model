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
    model::{Collections, Model},
    transxchange::naptan,
    Result,
};
use log::info;
use minidom::Element;
use std::{fs::File, io::Read, path::Path};
use zip::ZipArchive;

fn read_transxchange(_transxchange: &Element, _collections: &mut Collections) -> Result<()> {
    unimplemented!()
}

fn read_transxchange_archive<P>(transxchange_path: P, collections: &mut Collections) -> Result<()>
where
    P: AsRef<Path>,
{
    let zip_file = File::open(transxchange_path)?;
    let mut zip_archive = ZipArchive::new(zip_file)?;
    for index in 0..zip_archive.len() {
        let mut zip_file = zip_archive.by_index(index)?;
        match zip_file.sanitized_name().extension() {
            Some(ext) if ext == "xml" => {
                info!("reading TransXChange file {:?}", zip_file.sanitized_name());
                let mut file_content = String::new();
                zip_file.read_to_string(&mut file_content)?;
                let root: Element = file_content.parse()?;
                read_transxchange(&root, collections)?;
            }
            _ => {
                info!("skipping file in zip: {:?}", zip_file.sanitized_name());
            }
        }
    }
    Ok(())
}

/// Read TransXChange format into a Navitia Transit Model
pub fn read<P>(transxchange_path: P, naptan_path: P) -> Result<Model>
where
    P: AsRef<Path>,
{
    let mut collections = Collections::default();
    naptan::read_naptan(naptan_path, &mut collections)?;
    read_transxchange_archive(transxchange_path, &mut collections)?;
    Model::new(collections)
}
