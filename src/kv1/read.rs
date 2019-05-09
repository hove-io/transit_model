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

/// Generates stop_points
pub fn read_usrstop_point<H>(file_handler: &mut H, collections: &mut Collections) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    info!("Reading USRSTOPXXX.TMI and POINTXXXXX.TMI");

    // read POINTXXXXX.TMI
    // Generate HashMap<PointCode, (LocationX_EW, LocationY_NS)>

    // Read USRSTOPXXX.TMI and use the HashMap above to get the stop position
    // use proj crate to convert coordinates EPSG:28992 to EPSG:4326

    // collections.stop_points = CollectionWithId::new(stop_points)?;

    Ok(())
}

/// Generates stop_areas
pub fn read_usrstar<H>(file_handler: &mut H, collections: &mut Collections) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    info!("Reading USRSTARXXX.TMI");
    // filter collections.stop_points by sp.parent == UserStopAreaCode to calculate barycenter
    // collections.stop_areas = CollectionWithId::new(stop_areas)?;

    Ok(())
}

/// Generates networks, companies, stop_times, vehicle_journeys, routes and lines
pub fn read_jopa_pujopass_line<H>(file_handler: &mut H, collections: &mut Collections) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    info!("Reading JOPAXXXXXX.TMI, PUJOPASSXX.TMI and LINEXXXXXX.TMI");

    // collections.networks = CollectionWithId::new(networks)?;
    // collections.companies = CollectionWithId::new(companies)?;

    // Check that UserStopCode exists in collections.stop_points?
    // collections.stop_times = CollectionWithId::new(stop_times)?;

    // physical_mode_id = TransportationType in LINEXXXX.TMI where JOPAXXXXXX.(LinePlanningNumber, Direction) == LINEXXXXXX.(LinePlanningNumber, Direction)
    // needs collections.calendars
    // collections.vehicle_journeys = CollectionWithId::new(vehicle_journeys)?;

    // needs vehicles_journeys -> stop_times -> stop_points + stop_areas
    // collections.routes = CollectionWithId::new(routes)?;

    // need routes + stop_areas
    // collections.lines = CollectionWithId::new(lines)?;

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
