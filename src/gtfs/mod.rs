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

//! [GTFS](http://gtfs.org/) format management.

mod read;

use collection::CollectionWithId;
use common_format::manage_calendars;
use gtfs::read::EquipmentList;
use model::{Collections, Model};
use objects::Comment;
use std::path::Path;
use read_utils::add_prefix;
use Result;


/// Imports a `Model` from the [GTFS](http://gtfs.org/) files in the
/// `path` directory.
///
/// The `config_path` argument allows you to give a path to a file
/// containing a json representing the contributor and dataset used
/// for this GTFS. If not given, default values will be created.
///
/// The `prefix` argument is a string that will be prepended to every
/// identifiers, allowing to namespace the dataset. By default, no
/// prefix will be added to the identifiers.
pub fn read<P>(path: P, config_path: Option<P>, prefix: Option<String>) -> Result<Model>
where
    P: AsRef<Path>,
{
    let mut collections = Collections::default();
    let mut equipments = EquipmentList::default();
    let mut comments: CollectionWithId<Comment> = CollectionWithId::default();

    let path = path.as_ref();

    manage_calendars(&mut collections, path)?;

    let (contributors, mut datasets) = read::read_config(config_path)?;
    read::set_dataset_validity_period(&mut datasets, &collections.calendars)?;

    collections.contributors = contributors;
    collections.datasets = datasets;

    let (networks, companies) = read::read_agency(path)?;
    collections.networks = networks;
    collections.companies = companies;
    let (stop_areas, stop_points) = read::read_stops(path, &mut comments, &mut equipments)?;
    collections.transfers = read::read_transfers(path, &stop_points)?;
    collections.stop_areas = stop_areas;
    collections.stop_points = stop_points;

    read::read_routes(path, &mut collections)?;
    collections.equipments = CollectionWithId::new(equipments.into_equipments())?;
    collections.comments = comments;
    read::manage_stop_times(&mut collections, path)?;

    //add prefixes
    if let Some(prefix) = prefix {
        add_prefix(prefix, &mut collections)?;
    }

    Ok(Model::new(collections)?)
}
