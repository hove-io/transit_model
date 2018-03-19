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

mod read;

use std::path;
use {Collections, PtObjects};
use Result;
use common_format::manage_calendars;
use collection::add_prefix;

pub fn read<P: AsRef<path::Path>>(
    path: P,
    config_path: Option<P>,
    prefix: Option<String>,
) -> Result<PtObjects> {
    let mut collections = Collections::default();

    let (contributors, datasets) = read::read_config(config_path)?;
    collections.contributors = contributors;
    collections.datasets = datasets;

    let path = path.as_ref();
    let (networks, companies) = read::read_agency(path)?;
    collections.networks = networks;
    collections.companies = companies;
    let (stop_areas, stop_points) = read::read_stops(path)?;
    collections.stop_areas = stop_areas;
    collections.stop_points = stop_points;
    manage_calendars(&mut collections, path)?;
    read::read_routes(path, &mut collections)?;

    //add prefixes
    if let Some(prefix) = prefix {
        let prefix = prefix + ":";
        add_prefix(&mut collections.networks, &prefix)?;
        add_prefix(&mut collections.companies, &prefix)?;
        add_prefix(&mut collections.stop_points, &prefix)?;
        add_prefix(&mut collections.stop_areas, &prefix)?;
        add_prefix(&mut collections.routes, &prefix)?;
        add_prefix(&mut collections.lines, &prefix)?;
        add_prefix(&mut collections.contributors, &prefix)?;
        add_prefix(&mut collections.datasets, &prefix)?;
    }

    Ok(PtObjects::new(collections)?)
}
