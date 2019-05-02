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

use crate::model::Collections;
use crate::read_utils::FileHandler;
use crate::Result;
use log::info;

/// Generates calendars
pub fn read_operday<H>(file_handler: &mut H, collections: &mut Collections) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    info!("Reading OPERDAYXXX.TMI");

    // collections.calendars = CollectionWithId::new(calendars)?;

    Ok(())
}

/// Generates networks, companies and lines
pub fn read_line<H>(file_handler: &mut H, collections: &mut Collections) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    info!("Reading LINEXXXXXX.TMI");

    // collections.networks = CollectionWithId::new(networks)?;
    // collections.companies = CollectionWithId::new(companies)?;
    // collections.lines = CollectionWithId::new(lines)?;

    Ok(())
}

/// Generates stop_points
pub fn read_usrstop_point<H>(file_handler: &mut H, collections: &mut Collections) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    info!("Reading USRSTOPXXX.TMI and POINTXXXXX.TMI");

    // collections.stop_points = CollectionWithId::new(stop_points)?;

    Ok(())
}

/// Generates stop_areas
pub fn read_usrstar<H>(file_handler: &mut H, collections: &mut Collections) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    info!("Reading USRSTARXXX.TMI");

    // collections.stop_areas = CollectionWithId::new(stop_areas)?;

    Ok(())
}

/// Generates vehicle_journeys, stop_times and routes
pub fn read_jopa_pujopass<H>(file_handler: &mut H, collections: &mut Collections) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    info!("Reading JOPAXXXXXX.TMI and PUJOPASSXX.TMI");

    // collections.routes = CollectionWithId::new(routes)?;
    // collections.vehicle_journeys = CollectionWithId::new(vehicle_journeys)?;
    // collections.stop_times = CollectionWithId::new(listop_timesnes)?;

    Ok(())
}

/// Generates comments on trips
pub fn read_notice_ntcassgn<H>(file_handler: &mut H, collections: &mut Collections) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    info!("Reading NOTICEXXXX.TMI and NTCASSGNMX.TMI");

    // collections.comments = CollectionWithId::new(comments)?;

    Ok(())
}

#[cfg(test)]
mod tests {}
