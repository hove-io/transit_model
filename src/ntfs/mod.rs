// Copyright (C) 2017 Hove and/or its affiliates.
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by the
// Free Software Foundation, version 3.

// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.

// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>

//! [NTFS](https://github.com/hove-io/ntfs-specification/blob/master/ntfs_fr.md)
//! format management.

mod read;
mod write;

use crate::{
    calendars::{manage_calendars, write_calendar_dates},
    file_handler::{FileHandler, PathFileHandler, ZipHandler},
    model::{Collections, Model},
    objects::*,
    serde_utils::*,
    utils::*,
    Result,
};
use anyhow::{anyhow, Context};
use chrono::{DateTime, FixedOffset};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use std::{io, path};
use tempfile::tempdir;
use tracing::info;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct StopTime {
    stop_id: String,
    trip_id: String,
    stop_sequence: u32,
    arrival_time: Option<Time>,
    departure_time: Option<Time>,
    start_pickup_drop_off_window: Option<Time>,
    end_pickup_drop_off_window: Option<Time>,
    #[serde(default)]
    boarding_duration: u16,
    #[serde(default)]
    alighting_duration: u16,
    #[serde(default)]
    pickup_type: u8,
    #[serde(default)]
    drop_off_type: u8,
    #[serde(skip_serializing)]
    datetime_estimated: Option<u8>,
    local_zone_id: Option<u16>,
    stop_headsign: Option<String>,
    stop_time_id: Option<String>,
    #[serde(rename = "stop_time_precision")]
    precision: Option<StopTimePrecision>,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone, PartialEq)]
enum StopLocationType {
    #[default]
    #[serde(rename = "0")]
    StopPoint,
    #[serde(rename = "1")]
    StopArea,
    #[serde(rename = "2")]
    GeographicArea,
    #[serde(rename = "3")]
    EntranceExit,
    #[serde(rename = "4")]
    PathwayInterconnectionNode,
    #[serde(rename = "5")]
    BoardingArea,
}

impl From<StopLocationType> for StopType {
    fn from(stop_location_type: StopLocationType) -> StopType {
        match stop_location_type {
            StopLocationType::StopPoint => StopType::Point,
            StopLocationType::StopArea => StopType::Zone,
            StopLocationType::GeographicArea => StopType::Zone,
            StopLocationType::EntranceExit => StopType::StopEntrance,
            StopLocationType::PathwayInterconnectionNode => StopType::GenericNode,
            StopLocationType::BoardingArea => StopType::BoardingArea,
        }
    }
}

impl From<StopType> for StopLocationType {
    fn from(stop_type: StopType) -> StopLocationType {
        match stop_type {
            StopType::Point => StopLocationType::StopPoint,
            StopType::Zone => StopLocationType::StopArea,
            StopType::StopEntrance => StopLocationType::EntranceExit,
            StopType::GenericNode => StopLocationType::PathwayInterconnectionNode,
            StopType::BoardingArea => StopLocationType::BoardingArea,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Stop {
    #[serde(rename = "stop_id")]
    id: String,
    #[serde(rename = "stop_name")]
    name: String,
    #[serde(rename = "stop_code")]
    code: Option<String>,
    #[serde(
        default = "default_visible",
        deserialize_with = "de_from_u8",
        serialize_with = "ser_from_bool"
    )]
    visible: bool,
    fare_zone_id: Option<String>,
    #[serde(rename = "stop_lon")]
    lon: String,
    #[serde(rename = "stop_lat")]
    lat: String,
    #[serde(default, deserialize_with = "de_with_empty_default")]
    location_type: StopLocationType,
    parent_station: Option<String>,
    #[serde(rename = "stop_timezone")]
    timezone: Option<Tz>,
    geometry_id: Option<String>,
    equipment_id: Option<String>,
    level_id: Option<String>,
    platform_code: Option<String>,
    address_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct CommentLink {
    object_id: String,
    object_type: ObjectType,
    comment_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct BookingRuleLink {
    object_id: String,
    object_type: ObjectType,
    booking_rule_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Code {
    object_type: ObjectType,
    object_id: String,
    object_system: String,
    object_code: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ObjectProperty {
    object_type: ObjectType,
    object_id: String,
    object_property_name: String,
    object_property_value: String,
}

fn default_visible() -> bool {
    true
}

/// Checks if minimum FaresV2 collections are defined and not empty (ticket_use_restrictions and ticket_prices are optional)
/// See https://github.com/hove-io/ntfs-specification/blob/master/ntfs_fare_extension.md
fn has_fares_v2(collections: &Collections) -> bool {
    !collections.tickets.is_empty()
        && !collections.ticket_uses.is_empty()
        && !collections.ticket_use_perimeters.is_empty()
}

/// Checks if minimum FaresV1 collections are defined and not empty (fares_v1 is optional)
/// `prices.csv` and `od_fares.csv` are mandatory but od_fares.csv can be empty.
/// See https://github.com/hove-io/ntfs-specification/blob/master/ntfs_fare_extension_fr_deprecated.md
fn has_fares_v1(collections: &Collections) -> bool {
    !collections.prices_v1.is_empty()
}

/// Imports a `Model` from the
/// [NTFS](https://github.com/hove-io/ntfs-specification/blob/master/ntfs_fr.md)
/// files in the given directory.
pub fn from_dir<P: AsRef<path::Path>>(p: P) -> Result<Model> {
    let mut file_handle = PathFileHandler::new(p.as_ref().to_path_buf());
    read_file_handler(&mut file_handle)
}
/// Imports a `Model` from a zip file containing the
/// [NTFS](https://github.com/hove-io/ntfs-specification/blob/master/ntfs_fr.md).
pub fn from_zip<P: AsRef<path::Path>>(p: P) -> Result<Model> {
    let reader = std::fs::File::open(p.as_ref())?;
    let mut file_handler = ZipHandler::new(reader, p)?;
    read_file_handler(&mut file_handler)
}

/// Imports `Collections` from a zip file containing the
/// [NTFS](https://github.com/hove-io/ntfs-specification/blob/master/ntfs_fr.md).
pub fn collections_from_zip<P: AsRef<path::Path>>(p: P) -> Result<Collections> {
    let reader = std::fs::File::open(p.as_ref())?;
    let mut file_handler = ZipHandler::new(reader, p)?;
    read_collections_file_handler(&mut file_handler)
}

/// Imports `Collections` from the
/// [NTFS](https://github.com/hove-io/ntfs-specification/blob/master/ntfs_fr.md)
/// files in the given directory.
pub fn collections_from_dir<P: AsRef<path::Path>>(p: P) -> Result<Collections> {
    let mut file_handle = PathFileHandler::new(p.as_ref().to_path_buf());
    read_collections_file_handler(&mut file_handle)
}

/// Imports a `Model` from an object implementing `Read` and `Seek` and containing a zip file with a
/// [NTFS](https://github.com/hove-io/ntfs-specification/blob/master/ntfs_fr.md).
///
/// This method makes it possible to read from a variety of sources like read a NTFS
/// from the network.
///
/// ```ignore
/// let url = "http://some_url/ntfs.zip";
/// let resp = reqwest::blocking::get(url)?; // or async call
/// let data = std::io::Cursor::new(resp.bytes()?.to_vec());
/// let model = transit_model::ntfs::from_zip_reader(data, &url)?;
/// # Ok::<(), transit_model::Error>(())
/// ```
///
/// The `source_name` is needed to have nicer error messages.
pub fn from_zip_reader<R>(reader: R, source_name: &str) -> Result<Model>
where
    R: std::io::Seek + std::io::Read,
{
    let mut file_handler = ZipHandler::new(reader, source_name)?;
    read_file_handler(&mut file_handler)
}

/// Imports a `Model` from the
/// [NTFS](https://github.com/hove-io/ntfs-specification/blob/master/ntfs_fr.md)
/// files in the given directory.
/// This method will try to detect if the input is a zipped archive or not.
/// If the default file type mechanism is not enough, you can use
/// [from_zip] or [from_dir].
pub fn read<P: AsRef<path::Path>>(path: P) -> Result<Model> {
    let p = path.as_ref();
    if p.is_file() {
        // if it's a file, we consider it to be a zip (and an error will be returned if it is not)
        Ok(from_zip(p).with_context(|| format!("impossible to read zipped ntfs {p:?}"))?)
    } else if p.is_dir() {
        Ok(from_dir(p).with_context(|| format!("impossible to read ntfs directory from {p:?}"))?)
    } else {
        Err(anyhow!(
            "file {:?} is neither a file nor a directory, cannot read a ntfs from it",
            p
        ))
    }
}

/// Imports `Collections` from the
/// [NTFS](https://github.com/hove-io/ntfs-specification/blob/master/ntfs_fr.md)
/// files in the given directory.
/// This method will try to detect if the input is a zipped archive or not.
/// If the default file type mechanism is not enough, you can use
/// [from_zip] or [from_dir].
pub fn read_collections<P: AsRef<path::Path>>(path: P) -> Result<Collections> {
    let p = path.as_ref();
    if p.is_file() {
        // if it's a file, we consider it to be a zip (and an error will be returned if it is not)
        Ok(collections_from_zip(p)
            .with_context(|| format!("impossible to read zipped ntfs {p:?}"))?)
    } else if p.is_dir() {
        Ok(collections_from_dir(p)
            .with_context(|| format!("impossible to read ntfs directory from {p:?}"))?)
    } else {
        Err(anyhow!(
            "file {:?} is neither a file nor a directory, cannot read a ntfs from it",
            p
        ))
    }
}

/// Controls which NTFS sub-collections are loaded by [`read_collections_partial`].
///
/// By default ([`NtfsSelector::none`]), nothing is selected.
/// Call [`NtfsSelector::all`] to select everything (equivalent to [`read_collections`]).
///
/// # Example
/// ```no_run
/// use transit_model::ntfs::{read_collections_partial, NtfsSelector};
///
/// let collections = read_collections_partial(
///     "/path/to/ntfs",
///     NtfsSelector::none()
///         .with_vehicle_journeys()
///         .with_stop_points()
///         .with_geometries()
///         .with_stop_times(),
/// )?;
/// # Ok::<(), transit_model::Error>(())
/// ```
#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub struct NtfsSelector {
    // --- Direct collection loads ---
    pub contributors: bool,
    pub datasets: bool,
    pub commercial_modes: bool,
    pub networks: bool,
    pub lines: bool,
    pub routes: bool,
    /// `trips.txt`
    pub vehicle_journeys: bool,
    pub frequencies: bool,
    pub physical_modes: bool,
    pub companies: bool,
    pub equipments: bool,
    pub trip_properties: bool,
    pub transfers: bool,
    pub admin_stations: bool,
    /// Loads all fares-v2 files: `tickets.txt`, `ticket_uses.txt`,
    /// `ticket_prices.txt`, `ticket_use_perimeters.txt`,
    /// `ticket_use_restrictions.txt`.
    pub fares_v2: bool,
    pub levels: bool,
    /// Loads all grid files: `grid_calendars.txt`, `grid_exception_dates.txt`,
    /// `grid_periods.txt`, `grid_rel_calendar_line.txt`.
    pub grid: bool,
    pub addresses: bool,
    pub administrative_regions: bool,

    // --- manage_* calls ---
    pub calendars: bool,
    pub geometries: bool,
    pub feed_infos: bool,
    /// Loads stop points and stop areas (`stops.txt`).
    pub stop_points: bool,
    pub pathways: bool,
    /// Loads stop times into each vehicle journey.
    ///
    /// Only executed when `vehicle_journeys` and `stop_points` are also selected;
    /// if either dependency is missing this flag is silently ignored.
    pub stop_times: bool,
    pub codes: bool,
    pub comments: bool,
    pub booking_rules: bool,
    pub object_properties: bool,
    pub fares_v1: bool,
    /// Derives company assignments for vehicle journeys.
    ///
    /// Requires `vehicle_journeys` and `companies` to also be selected.
    pub companies_on_vj: bool,
    pub occupancies: bool,
    pub object_locks: bool,
}

#[allow(missing_docs)]
impl NtfsSelector {
    /// Selects nothing — the starting point for partial loads.
    pub fn none() -> Self {
        Self {
            contributors: false,
            datasets: false,
            commercial_modes: false,
            networks: false,
            lines: false,
            routes: false,
            vehicle_journeys: false,
            frequencies: false,
            physical_modes: false,
            companies: false,
            equipments: false,
            trip_properties: false,
            transfers: false,
            admin_stations: false,
            fares_v2: false,
            levels: false,
            grid: false,
            addresses: false,
            administrative_regions: false,
            calendars: false,
            geometries: false,
            feed_infos: false,
            stop_points: false,
            pathways: false,
            stop_times: false,
            codes: false,
            comments: false,
            booking_rules: false,
            object_properties: false,
            fares_v1: false,
            companies_on_vj: false,
            occupancies: false,
            object_locks: false,
        }
    }

    /// Selects everything — equivalent to calling [`read_collections`].
    pub fn all() -> Self {
        Self {
            contributors: true,
            datasets: true,
            commercial_modes: true,
            networks: true,
            lines: true,
            routes: true,
            vehicle_journeys: true,
            frequencies: true,
            physical_modes: true,
            companies: true,
            equipments: true,
            trip_properties: true,
            transfers: true,
            admin_stations: true,
            fares_v2: true,
            levels: true,
            grid: true,
            addresses: true,
            administrative_regions: true,
            calendars: true,
            geometries: true,
            feed_infos: true,
            stop_points: true,
            pathways: true,
            stop_times: true,
            codes: true,
            comments: true,
            booking_rules: true,
            object_properties: true,
            fares_v1: true,
            companies_on_vj: true,
            occupancies: true,
            object_locks: true,
        }
    }

    pub fn with_contributors(mut self) -> Self {
        self.contributors = true;
        self
    }
    pub fn with_datasets(mut self) -> Self {
        self.datasets = true;
        self
    }
    pub fn with_commercial_modes(mut self) -> Self {
        self.commercial_modes = true;
        self
    }
    pub fn with_networks(mut self) -> Self {
        self.networks = true;
        self
    }
    pub fn with_lines(mut self) -> Self {
        self.lines = true;
        self
    }
    pub fn with_routes(mut self) -> Self {
        self.routes = true;
        self
    }
    pub fn with_vehicle_journeys(mut self) -> Self {
        self.vehicle_journeys = true;
        self
    }
    pub fn with_frequencies(mut self) -> Self {
        self.frequencies = true;
        self
    }
    pub fn with_physical_modes(mut self) -> Self {
        self.physical_modes = true;
        self
    }
    pub fn with_companies(mut self) -> Self {
        self.companies = true;
        self
    }
    pub fn with_equipments(mut self) -> Self {
        self.equipments = true;
        self
    }
    pub fn with_trip_properties(mut self) -> Self {
        self.trip_properties = true;
        self
    }
    pub fn with_transfers(mut self) -> Self {
        self.transfers = true;
        self
    }
    pub fn with_admin_stations(mut self) -> Self {
        self.admin_stations = true;
        self
    }
    pub fn with_fares_v2(mut self) -> Self {
        self.fares_v2 = true;
        self
    }
    pub fn with_levels(mut self) -> Self {
        self.levels = true;
        self
    }
    pub fn with_grid(mut self) -> Self {
        self.grid = true;
        self
    }
    pub fn with_addresses(mut self) -> Self {
        self.addresses = true;
        self
    }
    pub fn with_administrative_regions(mut self) -> Self {
        self.administrative_regions = true;
        self
    }
    pub fn with_calendars(mut self) -> Self {
        self.calendars = true;
        self
    }
    pub fn with_geometries(mut self) -> Self {
        self.geometries = true;
        self
    }
    pub fn with_feed_infos(mut self) -> Self {
        self.feed_infos = true;
        self
    }
    pub fn with_stop_points(mut self) -> Self {
        self.stop_points = true;
        self
    }
    pub fn with_pathways(mut self) -> Self {
        self.pathways = true;
        self
    }
    pub fn with_stop_times(mut self) -> Self {
        self.stop_times = true;
        self
    }
    pub fn with_codes(mut self) -> Self {
        self.codes = true;
        self
    }
    pub fn with_comments(mut self) -> Self {
        self.comments = true;
        self
    }
    pub fn with_booking_rules(mut self) -> Self {
        self.booking_rules = true;
        self
    }
    pub fn with_object_properties(mut self) -> Self {
        self.object_properties = true;
        self
    }
    pub fn with_fares_v1(mut self) -> Self {
        self.fares_v1 = true;
        self
    }
    pub fn with_companies_on_vj(mut self) -> Self {
        self.companies_on_vj = true;
        self
    }
    pub fn with_occupancies(mut self) -> Self {
        self.occupancies = true;
        self
    }
    pub fn with_object_locks(mut self) -> Self {
        self.object_locks = true;
        self
    }
}

/// Loads only the NTFS sub-collections requested by `selector`.
///
/// Files not selected are left at their [`Default::default`] value.
/// Use this when you need only a few collections and want to avoid the cost
/// of loading the full dataset.
///
/// # Example
/// ```no_run
/// use transit_model::ntfs::{read_collections_partial, NtfsSelector};
///
/// let collections = read_collections_partial(
///     "/path/to/ntfs",
///     NtfsSelector::none()
///         .with_vehicle_journeys()
///         .with_stop_points()
///         .with_geometries()
///         .with_stop_times(),
/// )?;
/// # Ok::<(), transit_model::Error>(())
/// ```
pub fn read_collections_partial<P: AsRef<path::Path>>(
    path: P,
    selector: NtfsSelector,
) -> Result<Collections> {
    let p = path.as_ref();
    if p.is_file() {
        let reader = std::fs::File::open(p)?;
        let mut file_handler = ZipHandler::new(reader, p)?;
        read_collections_partial_file_handler(&mut file_handler, &selector)
            .with_context(|| format!("impossible to read zipped ntfs {p:?}"))
    } else if p.is_dir() {
        let mut file_handler = PathFileHandler::new(p.to_path_buf());
        read_collections_partial_file_handler(&mut file_handler, &selector)
            .with_context(|| format!("impossible to read ntfs directory from {p:?}"))
    } else {
        Err(anyhow!(
            "file {:?} is neither a file nor a directory, cannot read a ntfs from it",
            p
        ))
    }
}

fn read_file_handler<H>(file_handler: &mut H) -> Result<Model>
where
    for<'a> &'a mut H: FileHandler,
{
    let collections = read_collections_partial_file_handler(file_handler, &NtfsSelector::all())?;
    info!("Indexing");
    let res = Model::new(collections)?;
    info!("Loading NTFS done");
    Ok(res)
}

fn read_collections_file_handler<H>(file_handler: &mut H) -> Result<Collections>
where
    for<'a> &'a mut H: FileHandler,
{
    read_collections_partial_file_handler(file_handler, &NtfsSelector::all())
}

fn read_collections_partial_file_handler<H>(
    file_handler: &mut H,
    s: &NtfsSelector,
) -> Result<Collections>
where
    for<'a> &'a mut H: FileHandler,
{
    info!("Loading NTFS from {:?}", file_handler.source_name());

    macro_rules! load {
        ($flag:expr, $expr:expr) => {
            if $flag {
                $expr?
            } else {
                Default::default()
            }
        };
    }

    let mut collections = Collections {
        contributors: load!(
            s.contributors,
            make_collection_with_id(file_handler, "contributors.txt")
        ),
        datasets: load!(
            s.datasets,
            make_collection_with_id(file_handler, "datasets.txt")
        ),
        commercial_modes: load!(
            s.commercial_modes,
            make_collection_with_id(file_handler, "commercial_modes.txt")
        ),
        networks: load!(
            s.networks,
            make_collection_with_id(file_handler, "networks.txt")
        ),
        lines: load!(s.lines, make_collection_with_id(file_handler, "lines.txt")),
        routes: load!(
            s.routes,
            make_collection_with_id(file_handler, "routes.txt")
        ),
        vehicle_journeys: load!(
            s.vehicle_journeys,
            make_collection_with_id(file_handler, "trips.txt")
        ),
        frequencies: load!(
            s.frequencies,
            make_opt_collection(file_handler, "frequencies.txt")
        ),
        physical_modes: load!(
            s.physical_modes,
            make_collection_with_id(file_handler, "physical_modes.txt")
        ),
        companies: load!(
            s.companies,
            make_collection_with_id(file_handler, "companies.txt")
        ),
        equipments: load!(
            s.equipments,
            make_opt_collection_with_id(file_handler, "equipments.txt")
        ),
        trip_properties: load!(
            s.trip_properties,
            make_opt_collection_with_id(file_handler, "trip_properties.txt")
        ),
        transfers: load!(
            s.transfers,
            make_opt_collection(file_handler, "transfers.txt")
        ),
        admin_stations: load!(
            s.admin_stations,
            make_opt_collection(file_handler, "admin_stations.txt")
        ),
        tickets: load!(
            s.fares_v2,
            make_opt_collection_with_id(file_handler, "tickets.txt")
        ),
        ticket_uses: load!(
            s.fares_v2,
            make_opt_collection_with_id(file_handler, "ticket_uses.txt")
        ),
        ticket_prices: load!(
            s.fares_v2,
            make_opt_collection(file_handler, "ticket_prices.txt")
        ),
        ticket_use_perimeters: load!(
            s.fares_v2,
            make_opt_collection(file_handler, "ticket_use_perimeters.txt")
        ),
        ticket_use_restrictions: load!(
            s.fares_v2,
            make_opt_collection(file_handler, "ticket_use_restrictions.txt")
        ),
        levels: load!(
            s.levels,
            make_opt_collection_with_id(file_handler, "levels.txt")
        ),
        grid_calendars: load!(
            s.grid,
            make_opt_collection_with_id(file_handler, "grid_calendars.txt")
        ),
        grid_exception_dates: load!(
            s.grid,
            make_opt_collection(file_handler, "grid_exception_dates.txt")
        ),
        grid_periods: load!(
            s.grid,
            make_opt_collection(file_handler, "grid_periods.txt")
        ),
        grid_rel_calendar_line: load!(
            s.grid,
            make_opt_collection(file_handler, "grid_rel_calendar_line.txt")
        ),
        addresses: load!(
            s.addresses,
            make_opt_collection_with_id(file_handler, "addresses.txt")
        ),
        administrative_regions: load!(
            s.administrative_regions,
            make_opt_collection_with_id(file_handler, "administrative_regions.txt")
        ),
        ..Default::default()
    };

    if s.calendars {
        manage_calendars(file_handler, &mut collections)?;
    }
    if s.geometries {
        read::manage_geometries(&mut collections, file_handler)?;
    }
    if s.feed_infos {
        read::manage_feed_infos(&mut collections, file_handler)?;
    }
    if s.stop_points {
        read::manage_stops(&mut collections, file_handler)?;
    }
    if s.pathways {
        read::manage_pathways(&mut collections, file_handler)?;
    }
    if s.stop_times && s.vehicle_journeys && s.stop_points {
        read::manage_stop_times(&mut collections, file_handler)?;
    }
    if s.codes {
        read::manage_codes(&mut collections, file_handler)?;
    }
    if s.comments {
        read::manage_comments(&mut collections, file_handler)?;
    }
    if s.booking_rules {
        read::manage_booking_rules(&mut collections, file_handler)?;
    }
    if s.object_properties {
        read::manage_object_properties(&mut collections, file_handler)?;
    }
    if s.fares_v1 {
        read::manage_fares_v1(&mut collections, file_handler)?;
    }
    if s.companies_on_vj && s.vehicle_journeys && s.companies {
        read::manage_companies_on_vj(&mut collections)?;
    }
    if s.occupancies {
        read::manage_occupancies(&mut collections, file_handler)?;
    }
    if s.object_locks {
        read::manage_object_locks(&mut collections, file_handler)?;
    }

    Ok(collections)
}

/// Exports a `Collections` to the
/// [NTFS](https://github.com/hove-io/ntfs-specification/blob/master/ntfs_fr.md)
/// files in the given directory.
pub fn write<P: AsRef<path::Path>>(
    collections: &Collections,
    path: P,
    current_datetime: DateTime<FixedOffset>,
) -> Result<()> {
    let path = path.as_ref();
    std::fs::create_dir_all(path)?;
    info!("Writing NTFS to {:?}", path);

    write::write_feed_infos(path, collections, current_datetime)?;
    write_collection_with_id(path, "contributors.txt", &collections.contributors)?;
    write_collection_with_id(path, "datasets.txt", &collections.datasets)?;
    write_collection_with_id(path, "networks.txt", &collections.networks)?;
    write_collection_with_id(path, "commercial_modes.txt", &collections.commercial_modes)?;
    write_collection_with_id(path, "companies.txt", &collections.companies)?;
    write_collection_with_id(path, "lines.txt", &collections.lines)?;
    write_collection_with_id(path, "physical_modes.txt", &collections.physical_modes)?;
    write_collection_with_id(path, "equipments.txt", &collections.equipments)?;
    write_collection_with_id(path, "routes.txt", &collections.routes)?;
    write_collection_with_id(path, "trip_properties.txt", &collections.trip_properties)?;
    write_collection_with_id(path, "geometries.txt", &collections.geometries)?;
    write_collection(path, "transfers.txt", &collections.transfers)?;
    write_collection(path, "admin_stations.txt", &collections.admin_stations)?;
    write_collection_with_id(path, "tickets.txt", &collections.tickets)?;
    write_collection_with_id(path, "ticket_uses.txt", &collections.ticket_uses)?;
    write_collection(path, "ticket_prices.txt", &collections.ticket_prices)?;
    write_collection(
        path,
        "ticket_use_perimeters.txt",
        &collections.ticket_use_perimeters,
    )?;
    write_collection(
        path,
        "ticket_use_restrictions.txt",
        &collections.ticket_use_restrictions,
    )?;
    write_collection_with_id(path, "grid_calendars.txt", &collections.grid_calendars)?;
    write_collection(
        path,
        "grid_exception_dates.txt",
        &collections.grid_exception_dates,
    )?;
    write_collection(path, "grid_periods.txt", &collections.grid_periods)?;
    write_collection(
        path,
        "grid_rel_calendar_line.txt",
        &collections.grid_rel_calendar_line,
    )?;
    write::write_vehicle_journeys_and_stop_times(
        path,
        &collections.vehicle_journeys,
        &collections.stop_points,
        &collections.stop_time_headsigns,
        &collections.stop_time_ids,
    )?;
    write_collection(path, "frequencies.txt", &collections.frequencies)?;
    write_calendar_dates(path, &collections.calendars)?;
    write::write_stops(
        path,
        &collections.stop_points,
        &collections.stop_areas,
        &collections.stop_locations,
    )?;
    write::write_comments(path, collections)?;
    write::write_booking_rules(path, collections)?;
    write::write_codes(path, collections)?;
    write::write_object_properties(path, collections)?;
    write::write_fares_v1(path, collections)?;
    write_collection_with_id(path, "pathways.txt", &collections.pathways)?;
    write_collection_with_id(path, "levels.txt", &collections.levels)?;
    write_collection_with_id(path, "addresses.txt", &collections.addresses)?;
    write_collection_with_id(
        path,
        "administrative_regions.txt",
        &collections.administrative_regions,
    )?;
    write_collection(path, "occupancies.txt", &collections.occupancies)?;
    write_collection(path, "object_locks.txt", &collections.object_locks)?;

    Ok(())
}

/// Exports only the sub-collections selected by `selector` to the given directory.
///
/// Files not selected are not written; the caller is responsible for providing
/// the missing files (e.g. by copying them unchanged from the input).
///
/// This is the write-side counterpart of [`read_collections_partial`]: use it
/// to overwrite only the files that were actually modified, leaving the rest of
/// the NTFS dataset untouched.
///
/// # Example
/// ```no_run
/// use transit_model::ntfs::{write_partial, NtfsSelector};
/// use chrono::DateTime;
///
/// # let collections = transit_model::ModelBuilder::default().build().into_collections();
/// # let current_datetime = DateTime::parse_from_rfc3339("2024-01-01T00:00:00+00:00").unwrap();
/// write_partial(
///     &collections,
///     "/path/to/output",
///     current_datetime,
///     &NtfsSelector::none()
///         .with_vehicle_journeys()   // writes trips.txt + stop_times.txt
///         .with_geometries(),        // writes geometries.txt
/// )?;
/// # Ok::<(), transit_model::Error>(())
/// ```
pub fn write_partial<P: AsRef<path::Path>>(
    collections: &Collections,
    path: P,
    current_datetime: DateTime<FixedOffset>,
    selector: &NtfsSelector,
) -> Result<()> {
    let path = path.as_ref();
    std::fs::create_dir_all(path)?;
    info!("Writing partial NTFS to {:?}", path);

    if selector.feed_infos {
        write::write_feed_infos(path, collections, current_datetime)?;
    }
    if selector.contributors {
        write_collection_with_id(path, "contributors.txt", &collections.contributors)?;
    }
    if selector.datasets {
        write_collection_with_id(path, "datasets.txt", &collections.datasets)?;
    }
    if selector.networks {
        write_collection_with_id(path, "networks.txt", &collections.networks)?;
    }
    if selector.commercial_modes {
        write_collection_with_id(path, "commercial_modes.txt", &collections.commercial_modes)?;
    }
    if selector.companies {
        write_collection_with_id(path, "companies.txt", &collections.companies)?;
    }
    if selector.lines {
        write_collection_with_id(path, "lines.txt", &collections.lines)?;
    }
    if selector.physical_modes {
        write_collection_with_id(path, "physical_modes.txt", &collections.physical_modes)?;
    }
    if selector.equipments {
        write_collection_with_id(path, "equipments.txt", &collections.equipments)?;
    }
    if selector.routes {
        write_collection_with_id(path, "routes.txt", &collections.routes)?;
    }
    if selector.trip_properties {
        write_collection_with_id(path, "trip_properties.txt", &collections.trip_properties)?;
    }
    if selector.geometries {
        write_collection_with_id(path, "geometries.txt", &collections.geometries)?;
    }
    if selector.transfers {
        write_collection(path, "transfers.txt", &collections.transfers)?;
    }
    if selector.admin_stations {
        write_collection(path, "admin_stations.txt", &collections.admin_stations)?;
    }
    if selector.fares_v2 {
        write_collection_with_id(path, "tickets.txt", &collections.tickets)?;
        write_collection_with_id(path, "ticket_uses.txt", &collections.ticket_uses)?;
        write_collection(path, "ticket_prices.txt", &collections.ticket_prices)?;
        write_collection(
            path,
            "ticket_use_perimeters.txt",
            &collections.ticket_use_perimeters,
        )?;
        write_collection(
            path,
            "ticket_use_restrictions.txt",
            &collections.ticket_use_restrictions,
        )?;
    }
    if selector.grid {
        write_collection_with_id(path, "grid_calendars.txt", &collections.grid_calendars)?;
        write_collection(
            path,
            "grid_exception_dates.txt",
            &collections.grid_exception_dates,
        )?;
        write_collection(path, "grid_periods.txt", &collections.grid_periods)?;
        write_collection(
            path,
            "grid_rel_calendar_line.txt",
            &collections.grid_rel_calendar_line,
        )?;
    }
    if selector.vehicle_journeys {
        // Writes both trips.txt and stop_times.txt.
        write::write_vehicle_journeys_and_stop_times(
            path,
            &collections.vehicle_journeys,
            &collections.stop_points,
            &collections.stop_time_headsigns,
            &collections.stop_time_ids,
        )?;
    }
    if selector.frequencies {
        write_collection(path, "frequencies.txt", &collections.frequencies)?;
    }
    if selector.calendars {
        write_calendar_dates(path, &collections.calendars)?;
    }
    if selector.stop_points {
        write::write_stops(
            path,
            &collections.stop_points,
            &collections.stop_areas,
            &collections.stop_locations,
        )?;
    }
    if selector.comments {
        write::write_comments(path, collections)?;
    }
    if selector.booking_rules {
        write::write_booking_rules(path, collections)?;
    }
    if selector.codes {
        write::write_codes(path, collections)?;
    }
    if selector.object_properties {
        write::write_object_properties(path, collections)?;
    }
    if selector.fares_v1 {
        write::write_fares_v1(path, collections)?;
    }
    if selector.pathways {
        write_collection_with_id(path, "pathways.txt", &collections.pathways)?;
    }
    if selector.levels {
        write_collection_with_id(path, "levels.txt", &collections.levels)?;
    }
    if selector.addresses {
        write_collection_with_id(path, "addresses.txt", &collections.addresses)?;
    }
    if selector.administrative_regions {
        write_collection_with_id(
            path,
            "administrative_regions.txt",
            &collections.administrative_regions,
        )?;
    }
    if selector.occupancies {
        write_collection(path, "occupancies.txt", &collections.occupancies)?;
    }
    if selector.object_locks {
        write_collection(path, "object_locks.txt", &collections.object_locks)?;
    }

    Ok(())
}

/// Exports a `Collections` to a
/// [NTFS](https://github.com/hove-io/ntfs-specification/blob/master/ntfs_fr.md)
/// ZIP archive at the given full path.
pub fn write_to_zip<P: AsRef<path::Path>>(
    collections: &Collections,
    path: P,
    current_datetime: DateTime<FixedOffset>,
) -> Result<()> {
    let path = path.as_ref();
    info!("Writing NTFS to ZIP File {:?}", path);
    let input_tmp_dir = tempdir()?;
    write(collections, input_tmp_dir.path(), current_datetime)?;
    zip_to(input_tmp_dir.path(), path)?;
    input_tmp_dir.close()?;
    Ok(())
}

/// Returns the NTFS file names that [`write_partial`] would produce for `s`.
///
/// Used by [`write_partial_update`] to know which files to skip when copying
/// the unchanged input files to the output.
fn selector_output_files(s: &NtfsSelector) -> Vec<&'static str> {
    let mut files: Vec<&'static str> = Vec::new();
    if s.feed_infos {
        files.push("feed_infos.txt");
    }
    if s.contributors {
        files.push("contributors.txt");
    }
    if s.datasets {
        files.push("datasets.txt");
    }
    if s.networks {
        files.push("networks.txt");
    }
    if s.commercial_modes {
        files.push("commercial_modes.txt");
    }
    if s.companies {
        files.push("companies.txt");
    }
    if s.lines {
        files.push("lines.txt");
    }
    if s.physical_modes {
        files.push("physical_modes.txt");
    }
    if s.equipments {
        files.push("equipments.txt");
    }
    if s.routes {
        files.push("routes.txt");
    }
    if s.trip_properties {
        files.push("trip_properties.txt");
    }
    if s.geometries {
        files.push("geometries.txt");
    }
    if s.transfers {
        files.push("transfers.txt");
    }
    if s.admin_stations {
        files.push("admin_stations.txt");
    }
    if s.fares_v2 {
        files.extend_from_slice(&[
            "tickets.txt",
            "ticket_uses.txt",
            "ticket_prices.txt",
            "ticket_use_perimeters.txt",
            "ticket_use_restrictions.txt",
        ]);
    }
    if s.grid {
        files.extend_from_slice(&[
            "grid_calendars.txt",
            "grid_exception_dates.txt",
            "grid_periods.txt",
            "grid_rel_calendar_line.txt",
        ]);
    }
    if s.vehicle_journeys {
        files.extend_from_slice(&["trips.txt", "stop_times.txt"]);
    }
    if s.frequencies {
        files.push("frequencies.txt");
    }
    if s.calendars {
        files.extend_from_slice(&["calendar.txt", "calendar_dates.txt"]);
    }
    if s.stop_points {
        files.push("stops.txt");
    }
    if s.comments {
        files.extend_from_slice(&["comments.txt", "comment_links.txt"]);
    }
    if s.booking_rules {
        files.extend_from_slice(&["booking_rules.txt", "booking_rule_links.txt"]);
    }
    if s.codes {
        files.push("object_codes.txt");
    }
    if s.object_properties {
        files.push("object_properties.txt");
    }
    if s.fares_v1 {
        // write_fares_v1 may produce v1 files (prices.csv / od_fares.csv / fares.csv)
        // or re-derive them from fares_v2 collections, so we skip all of them.
        files.extend_from_slice(&["prices.csv", "od_fares.csv", "fares.csv"]);
    }
    if s.pathways {
        files.push("pathways.txt");
    }
    if s.levels {
        files.push("levels.txt");
    }
    if s.addresses {
        files.push("addresses.txt");
    }
    if s.administrative_regions {
        files.push("administrative_regions.txt");
    }
    if s.occupancies {
        files.push("occupancies.txt");
    }
    if s.object_locks {
        files.push("object_locks.txt");
    }
    files
}

/// Copies all files from `input` (directory or ZIP) to `output_dir`, skipping
/// any file whose name appears in `skip`.
fn copy_input_except(input: &path::Path, output_dir: &path::Path, skip: &[&str]) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    if input.is_dir() {
        for entry in std::fs::read_dir(input)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let name = entry.file_name();
            if !skip.contains(&name.to_str().unwrap_or("")) {
                std::fs::copy(entry.path(), output_dir.join(&name))?;
            }
        }
    } else {
        // ZIP input: extract each file entry unless it is in `skip`.
        let reader = std::fs::File::open(input)?;
        let mut archive = zip::ZipArchive::new(reader)
            .with_context(|| format!("cannot open zip archive {:?}", input))?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            if !entry.is_file() {
                continue;
            }
            // Use only the leaf name to ignore any sub-directory prefix.
            let name = entry
                .enclosed_name()
                .and_then(|p| p.file_name().map(|n| n.to_owned()))
                .ok_or_else(|| anyhow!("invalid zip entry name: {:?}", entry.name()))?;
            if !skip.contains(&name.to_str().unwrap_or("")) {
                let mut buf = Vec::with_capacity(entry.size() as usize);
                io::Read::read_to_end(&mut entry, &mut buf)?;
                std::fs::write(output_dir.join(&name), buf)?;
            }
        }
    }
    Ok(())
}

/// Applies a partial NTFS update: copies unchanged files from `input` and
/// overwrites only the files produced by `write_partial` for `selector`.
///
/// Handles all four input/output combinations (directory or ZIP):
///
/// | input | output | strategy |
/// |-------|--------|----------|
/// | dir   | dir    | `fs::copy` unchanged + `write_partial` |
/// | dir   | zip    | copy to temp dir + `write_partial` + `zip_to` |
/// | zip   | dir    | extract unchanged + `write_partial` |
/// | zip   | zip    | extract unchanged to temp dir + `write_partial` + `zip_to` |
///
/// # Example
/// ```no_run
/// use transit_model::ntfs::{write_partial_update, NtfsSelector};
/// use chrono::DateTime;
///
/// # let collections = transit_model::ModelBuilder::default().build().into_collections();
/// # let current_datetime = DateTime::parse_from_rfc3339("2024-01-01T00:00:00+00:00").unwrap();
/// write_partial_update(
///     "/path/to/input",
///     "/path/to/output",
///     &collections,
///     current_datetime,
///     &NtfsSelector::none()
///         .with_vehicle_journeys()
///         .with_geometries(),
/// )?;
/// # Ok::<(), transit_model::Error>(())
/// ```
pub fn write_partial_update<P, Q>(
    input: P,
    output: Q,
    collections: &Collections,
    current_datetime: DateTime<FixedOffset>,
    selector: &NtfsSelector,
) -> Result<()>
where
    P: AsRef<path::Path>,
    Q: AsRef<path::Path>,
{
    let input = input.as_ref();
    let output = output.as_ref();
    let owned = selector_output_files(selector);

    let output_is_zip = output
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"));

    if output_is_zip {
        let tmp = tempdir()?;
        copy_input_except(input, tmp.path(), &owned)?;
        write_partial(collections, tmp.path(), current_datetime, selector)?;
        zip_to(tmp.path(), output)?;
        tmp.close()?;
    } else {
        copy_input_except(input, output, &owned)?;
        write_partial(collections, output, current_datetime, selector)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Collections;
    use super::*;
    use super::{read, write};
    use crate::calendars::{manage_calendars, write_calendar_dates};
    use crate::objects;
    use crate::{file_handler::PathFileHandler, test_utils::*};
    use geo::line_string;
    use pretty_assertions::assert_eq;
    use std::{
        collections::{BTreeMap, BTreeSet, HashMap},
        fmt::Debug,
    };
    use typed_index_collection::{Collection, CollectionWithId, Id};

    fn test_serialize_deserialize_collection_with_id<T>(objects: Vec<T>)
    where
        T: Id<T> + PartialEq + Debug + serde::Serialize,
        for<'de> T: serde::Deserialize<'de>,
    {
        let collection = CollectionWithId::new(objects).unwrap();
        test_in_tmp_dir(|path| {
            write_collection_with_id(path, "file.txt", &collection).unwrap();
            let mut handler = PathFileHandler::new(path.to_path_buf());
            let des_collection = make_collection_with_id(&mut handler, "file.txt").unwrap();
            assert_eq!(collection, des_collection);
        });
    }

    fn test_serialize_deserialize_collection<T>(objects: Vec<T>)
    where
        T: PartialEq + Debug + serde::Serialize,
        for<'de> T: serde::Deserialize<'de>,
    {
        let collection = Collection::new(objects);
        test_in_tmp_dir(|path| {
            write_collection(path, "file.txt", &collection).unwrap();
            let mut handler = PathFileHandler::new(path.to_path_buf());
            let des_collection = make_opt_collection(&mut handler, "file.txt").unwrap();
            assert_eq!(collection, des_collection);
        });
    }

    fn btree_set_from_vec<T: Ord>(input: Vec<T>) -> BTreeSet<T> {
        input.into_iter().collect()
    }

    #[test]
    fn feed_infos_serialization_deserialization() {
        let mut feed_infos = BTreeMap::default();
        feed_infos.insert("tartare_platform".to_string(), "dev".to_string());
        feed_infos.insert("feed_publisher_name".to_string(), "Nicaragua".to_string());

        let dataset = Dataset {
            id: "Foo:0".to_string(),
            contributor_id: "Foo".to_string(),
            start_date: chrono::NaiveDate::from_ymd_opt(2018, 1, 30).unwrap(),
            end_date: chrono::NaiveDate::from_ymd_opt(2018, 1, 31).unwrap(),
            dataset_type: Some(DatasetType::Theorical),
            extrapolation: false,
            desc: Some("description".to_string()),
            system: Some("GTFS V2".to_string()),
        };

        let mut collections = Collections {
            datasets: CollectionWithId::from(dataset),
            feed_infos,
            ..Default::default()
        };

        test_in_tmp_dir(|path| {
            write::write_feed_infos(path, &collections, get_test_datetime()).unwrap();
            let mut handler = PathFileHandler::new(path.to_path_buf());
            read::manage_feed_infos(&mut collections, &mut handler).unwrap();
            assert_eq!(
                vec![
                    ("feed_creation_date".to_string(), "20190403".to_string()),
                    (
                        "feed_creation_datetime".to_string(),
                        "2019-04-03T17:19:00+00:00".to_string()
                    ),
                    ("feed_creation_time".to_string(), "17:19:00".to_string()),
                    ("feed_end_date".to_string(), "20180131".to_string()),
                    ("feed_publisher_name".to_string(), "Nicaragua".to_string()),
                    ("feed_start_date".to_string(), "20180130".to_string()),
                    ("ntfs_version".to_string(), "0.21.0".to_string()),
                    ("tartare_platform".to_string(), "dev".to_string()),
                ],
                collections
                    .feed_infos
                    .into_iter()
                    .collect::<Vec<(String, String)>>()
            );
        });
    }

    #[test]
    fn networks_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            Network {
                id: "OIF:101".to_string(),
                name: "SAVAC".to_string(),
                url: Some("http://www.vianavigo.com".to_string()),
                timezone: Some(chrono_tz::Europe::Paris),
                lang: Some("fr".to_string()),
                phone: Some("0123456789".to_string()),
                address: Some("somewhere".to_string()),
                fare_url: Some("http://www.vianavigo.com/tickets".to_string()),
                sort_order: Some(1),
                codes: KeysValues::default(),
            },
            Network {
                id: "OIF:102".to_string(),
                name: "SAVAC".to_string(),
                url: None,
                timezone: None,
                lang: None,
                phone: None,
                address: None,
                fare_url: None,
                sort_order: None,
                codes: KeysValues::default(),
            },
        ]);
    }

    #[test]
    fn commercial_modes_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            CommercialMode {
                id: "boarding_landing".to_string(),
                name: "Boarding - Landing".to_string(),
            },
            CommercialMode {
                id: "bus".to_string(),
                name: "Bus".to_string(),
            },
        ]);
    }

    #[test]
    fn companies_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            Company {
                id: "OIF:101".to_string(),
                name: "Foo".to_string(),
                address: Some("foo address".to_string()),
                url: Some("http://www.foo.fr/".to_string()),
                mail: Some("contact@foo.fr".to_string()),
                phone: Some("0123456789".to_string()),
                codes: BTreeSet::new(),
                ..Default::default()
            },
            Company {
                id: "OIF:102".to_string(),
                name: "Bar".to_string(),
                address: None,
                url: None,
                mail: None,
                phone: None,
                codes: BTreeSet::new(),
                ..Default::default()
            },
        ]);
    }

    #[test]
    fn lines_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            Line {
                id: "OIF:002002002:BDEOIF829".to_string(),
                name: "DEF".to_string(),
                code: Some("DEF".to_string()),
                codes: KeysValues::default(),
                object_properties: PropertiesMap::default(),
                comment_links: LinksT::default(),
                booking_rule_links: LinksT::default(),
                forward_name: Some("Hôtels - Hôtels".to_string()),
                backward_name: Some("Hôtels - Hôtels".to_string()),
                color: Some(Rgb {
                    red: 155,
                    green: 12,
                    blue: 89,
                }),
                text_color: Some(Rgb {
                    red: 10,
                    green: 0,
                    blue: 45,
                }),
                sort_order: Some(1342),
                network_id: "OIF:829".to_string(),
                commercial_mode_id: "bus".to_string(),
                geometry_id: Some("Geometry:Line:Relation:6883353".to_string()),
                opening_time: Some(Time::new(9, 0, 0)),
                closing_time: Some(Time::new(18, 0, 0)),
            },
            Line {
                id: "OIF:002002003:3OIF829".to_string(),
                name: "3".to_string(),
                code: None,
                codes: KeysValues::default(),
                object_properties: PropertiesMap::default(),
                comment_links: LinksT::default(),
                booking_rule_links: LinksT::default(),
                forward_name: None,
                backward_name: None,
                color: None,
                text_color: None,
                sort_order: None,
                network_id: "OIF:829".to_string(),
                commercial_mode_id: "bus".to_string(),
                geometry_id: None,
                opening_time: None,
                closing_time: None,
            },
        ]);
    }

    #[test]
    fn physical_modes_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            PhysicalMode {
                id: "Bus".to_string(),
                name: "Bus".to_string(),
                co2_emission: Some(6.2),
            },
            PhysicalMode {
                id: "Funicular".to_string(),
                name: "Funicular".to_string(),
                co2_emission: None,
            },
            PhysicalMode {
                id: "SuspendedCableCar".to_string(),
                name: "Suspended Cable Car".to_string(),
                co2_emission: None,
            },
        ]);
    }

    #[test]
    fn routes_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            Route {
                id: "IF:002002002:BDE".to_string(),
                name: "Hôtels - Hôtels".to_string(),
                direction_type: Some("forward".to_string()),
                codes: KeysValues::default(),
                object_properties: PropertiesMap::default(),
                comment_links: LinksT::default(),
                line_id: "OIF:002002002:BDEOIF829".to_string(),
                geometry_id: Some("Geometry:Line:Relation:6883353".to_string()),
                destination_id: Some("OIF,OIF:SA:4:126".to_string()),
            },
            Route {
                id: "OIF:002002002:CEN".to_string(),
                name: "Hôtels - Hôtels".to_string(),
                direction_type: None,
                codes: KeysValues::default(),
                object_properties: PropertiesMap::default(),
                comment_links: LinksT::default(),
                line_id: "OIF:002002002:BDEOIF829".to_string(),
                geometry_id: None,
                destination_id: None,
            },
        ]);
    }

    #[test]
    fn vehicle_journeys_and_stop_times_serialization_deserialization() {
        let stop_points = CollectionWithId::new(vec![
            StopPoint {
                id: "OIF:SP:36:2085".to_string(),
                name: "Gare de Saint-Cyr l'École".to_string(),
                visible: true,
                coord: Coord {
                    lon: 2.073_034,
                    lat: 48.799_115,
                },
                stop_area_id: "OIF:SA:8739322".to_string(),
                timezone: Some(chrono_tz::Europe::Paris),
                fare_zone_id: Some("1".to_string()),
                stop_type: StopType::Point,
                ..Default::default()
            },
            StopPoint {
                id: "OIF:SP:36:2127".to_string(),
                name: "Division Leclerc".to_string(),
                visible: true,
                coord: Coord {
                    lon: 2.073_407,
                    lat: 48.800_598,
                },
                stop_area_id: "OIF:SA:2:1468".to_string(),
                timezone: Some(chrono_tz::Europe::Paris),
                stop_type: StopType::Point,
                ..Default::default()
            },
        ])
        .unwrap();
        let vehicle_journeys = CollectionWithId::new(vec![
            VehicleJourney {
                id: "OIF:87604986-1_11595-1".to_string(),
                codes: KeysValues::default(),
                object_properties: PropertiesMap::default(),
                comment_links: LinksT::default(),
                booking_rule_links: LinksT::default(),
                route_id: "OIF:078078001:1".to_string(),
                physical_mode_id: "Bus".to_string(),
                dataset_id: "OIF:0".to_string(),
                service_id: "2".to_string(),
                headsign: Some("2005".to_string()),
                short_name: Some("42".to_string()),
                block_id: Some("PLOI".to_string()),
                company_id: "OIF:743".to_string(),
                trip_property_id: Some("0".to_string()),
                geometry_id: Some("Geometry:Line:Relation:6883353".to_string()),
                stop_times: vec![
                    objects::StopTime {
                        stop_point_idx: stop_points.get_idx("OIF:SP:36:2085").unwrap(),
                        sequence: 0,
                        arrival_time: Some(Time::new(14, 40, 0)),
                        departure_time: Some(Time::new(14, 40, 0)),
                        start_pickup_drop_off_window: None,
                        end_pickup_drop_off_window: None,
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 0,
                        drop_off_type: 1,
                        local_zone_id: None,
                        precision: Some(StopTimePrecision::Exact),
                    },
                    objects::StopTime {
                        stop_point_idx: stop_points.get_idx("OIF:SP:36:2127").unwrap(),
                        sequence: 1,
                        arrival_time: Some(Time::new(14, 42, 0)),
                        departure_time: Some(Time::new(14, 42, 0)),
                        start_pickup_drop_off_window: None,
                        end_pickup_drop_off_window: None,
                        boarding_duration: 0,
                        alighting_duration: 0,
                        pickup_type: 0,
                        drop_off_type: 0,
                        local_zone_id: None,
                        precision: Some(StopTimePrecision::Exact),
                    },
                ],
                journey_pattern_id: Some(String::from("OIF:JP:1")),
            },
            VehicleJourney {
                id: "OIF:90014407-1_425283-1".to_string(),
                codes: KeysValues::default(),
                object_properties: PropertiesMap::default(),
                comment_links: LinksT::default(),
                booking_rule_links: LinksT::default(),
                route_id: "OIF:800:TER".to_string(),
                physical_mode_id: "Bus".to_string(),
                dataset_id: "OIF:0".to_string(),
                service_id: "2".to_string(),
                headsign: None,
                short_name: Some("43".to_string()),
                block_id: None,
                company_id: "OIF:743".to_string(),
                trip_property_id: None,
                geometry_id: None,
                stop_times: vec![],
                journey_pattern_id: Some(String::from("OIF:JP:1")),
            },
        ])
        .unwrap();

        let mut headsigns = HashMap::new();
        headsigns.insert(
            ("OIF:87604986-1_11595-1".to_string(), 1),
            "somewhere".to_string(),
        );
        let mut stop_time_ids = HashMap::new();
        stop_time_ids.insert(
            ("OIF:87604986-1_11595-1".to_string(), 0),
            "StopTime:OIF:87604986-1_11595-1:0".to_string(),
        );

        test_in_tmp_dir(|path| {
            write::write_vehicle_journeys_and_stop_times(
                path,
                &vehicle_journeys,
                &stop_points,
                &headsigns,
                &stop_time_ids,
            )
            .unwrap();

            let mut handler = PathFileHandler::new(path.to_path_buf());
            let mut collections = Collections {
                vehicle_journeys: make_collection_with_id(&mut handler, "trips.txt").unwrap(),
                stop_points,
                ..Default::default()
            };

            read::manage_stop_times(&mut collections, &mut handler).unwrap();
            assert_eq!(vehicle_journeys, collections.vehicle_journeys);
            assert_eq!(collections.stop_time_headsigns, headsigns);
            assert_eq!(collections.stop_time_ids, stop_time_ids);
        });
    }

    #[test]
    fn contributors_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            Contributor {
                id: "Foo".to_string(),
                name: "Foo".to_string(),
                license: Some("ODbL".to_string()),
                website: Some("http://www.foo.com".to_string()),
            },
            Contributor {
                id: "Bar".to_string(),
                name: "Bar".to_string(),
                license: None,
                website: None,
            },
        ]);
    }

    #[test]
    fn datasets_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            Dataset {
                id: "Foo:0".to_string(),
                contributor_id: "Foo".to_string(),
                start_date: chrono::NaiveDate::from_ymd_opt(2018, 1, 30).unwrap(),
                end_date: chrono::NaiveDate::from_ymd_opt(2018, 1, 31).unwrap(),
                dataset_type: Some(DatasetType::Theorical),
                extrapolation: false,
                desc: Some("description".to_string()),
                system: Some("GTFS V2".to_string()),
            },
            Dataset {
                id: "Bar:0".to_string(),
                contributor_id: "Bar".to_string(),
                start_date: chrono::NaiveDate::from_ymd_opt(2018, 1, 30).unwrap(),
                end_date: chrono::NaiveDate::from_ymd_opt(2018, 1, 31).unwrap(),
                dataset_type: None,
                extrapolation: false,
                desc: None,
                system: None,
            },
        ]);
    }

    #[test]
    fn equipments_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![Equipment {
            id: "1".to_string(),
            wheelchair_boarding: Availability::Available,
            sheltered: Availability::InformationNotAvailable,
            elevator: Availability::Available,
            escalator: Availability::Available,
            bike_accepted: Availability::Available,
            bike_depot: Availability::Available,
            visual_announcement: Availability::Available,
            audible_announcement: Availability::Available,
            appropriate_escort: Availability::Available,
            appropriate_signage: Availability::Available,
        }]);
    }

    #[test]
    fn transfers_serialization_deserialization() {
        let transfers = vec![
            Transfer {
                from_stop_id: "st_1".to_string(),
                to_stop_id: "st_1".to_string(),
                min_transfer_time: Some(20),
                real_min_transfer_time: Some(30),
                equipment_id: Some("eq_1".to_string()),
            },
            Transfer {
                from_stop_id: "st_1".to_string(),
                to_stop_id: "st_2".to_string(),
                min_transfer_time: None,
                real_min_transfer_time: None,
                equipment_id: Some("eq_1".to_string()),
            },
        ];
        let expected_transfers = vec![
            Transfer {
                from_stop_id: "st_1".to_string(),
                to_stop_id: "st_1".to_string(),
                min_transfer_time: Some(20),
                real_min_transfer_time: Some(30),
                equipment_id: Some("eq_1".to_string()),
            },
            Transfer {
                from_stop_id: "st_1".to_string(),
                to_stop_id: "st_2".to_string(),
                min_transfer_time: Some(0),
                real_min_transfer_time: Some(0),
                equipment_id: Some("eq_1".to_string()),
            },
        ];
        let collection = Collection::new(transfers);
        let expected_collection = Collection::new(expected_transfers);
        test_in_tmp_dir(|path| {
            write_collection(path, "file.txt", &collection).unwrap();
            let mut handler = PathFileHandler::new(path.to_path_buf());
            let des_collection = make_opt_collection(&mut handler, "file.txt").unwrap();
            assert_eq!(expected_collection, des_collection);
        });
    }

    #[test]
    fn calendar_serialization_deserialization() {
        let mut dates1 = ::std::collections::BTreeSet::new();
        dates1.insert(chrono::NaiveDate::from_ymd_opt(2018, 5, 5).unwrap());
        dates1.insert(chrono::NaiveDate::from_ymd_opt(2018, 5, 6).unwrap());

        let mut dates2 = ::std::collections::BTreeSet::new();
        dates2.insert(chrono::NaiveDate::from_ymd_opt(2018, 6, 1).unwrap());

        let calendars = CollectionWithId::new(vec![
            Calendar {
                id: "0".to_string(),
                dates: dates1,
            },
            Calendar {
                id: "1".to_string(),
                dates: dates2,
            },
        ])
        .unwrap();

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            write_calendar_dates(path, &calendars).unwrap();

            let mut collections = Collections::default();
            manage_calendars(&mut handler, &mut collections).unwrap();

            assert_eq!(calendars, collections.calendars);
        });
    }

    #[test]
    fn stops_serialization_deserialization() {
        let stop_points = CollectionWithId::new(vec![
            StopPoint {
                id: "sp_1".to_string(),
                name: "sp_name_1".to_string(),
                visible: true,
                coord: Coord {
                    lon: 2.073_034,
                    lat: 48.799_115,
                },
                timezone: Some(chrono_tz::Europe::Paris),
                geometry_id: Some("geometry_1".to_string()),
                equipment_id: Some("equipment_1".to_string()),
                stop_area_id: "sa_1".to_string(),
                fare_zone_id: Some("1".to_string()),
                stop_type: StopType::Point,
                ..Default::default()
            },
            // stop point with no parent station
            StopPoint {
                id: "sa_2".to_string(),
                name: "sa_name_2".to_string(),
                visible: true,
                coord: Coord {
                    lon: 2.173_034,
                    lat: 47.899_115,
                },
                stop_area_id: "Navitia:sa_2".to_string(),
                stop_type: StopType::Point,
                ..Default::default()
            },
        ])
        .unwrap();

        let stop_areas = CollectionWithId::new(vec![
            StopArea {
                id: "Navitia:sa_2".to_string(),
                name: "sa_name_2".to_string(),
                codes: KeysValues::default(),
                object_properties: PropertiesMap::default(),
                comment_links: LinksT::default(),
                visible: true,
                coord: Coord {
                    lon: 2.173_034,
                    lat: 47.899_115,
                },
                timezone: None,
                geometry_id: None,
                equipment_id: None,
                level_id: None,
                address_id: None,
            },
            StopArea {
                id: "sa_1".to_string(),
                name: "sa_name_1".to_string(),
                codes: KeysValues::default(),
                object_properties: PropertiesMap::default(),
                comment_links: LinksT::default(),
                visible: true,
                coord: Coord {
                    lon: 2.073_034,
                    lat: 48.799_115,
                },
                timezone: Some(chrono_tz::Europe::Paris),
                geometry_id: Some("geometry_3".to_string()),
                equipment_id: Some("equipment_1".to_string()),
                level_id: Some("level2".to_string()),
                address_id: None,
            },
        ])
        .unwrap();

        let stop_locations: CollectionWithId<StopLocation> = CollectionWithId::default();

        test_in_tmp_dir(|path| {
            write::write_stops(path, &stop_points, &stop_areas, &stop_locations).unwrap();

            let mut collections = Collections::default();
            let mut handler = PathFileHandler::new(path.to_path_buf());
            read::manage_stops(&mut collections, &mut handler).unwrap();

            assert_eq!(stop_points, collections.stop_points);
            assert_eq!(stop_areas, collections.stop_areas);
        });
    }

    #[test]
    fn comments_codes_object_properties_serialization_deserialization() {
        let mut ser_collections = Collections::default();
        let comments = CollectionWithId::new(vec![
            Comment {
                id: "c:1".to_string(),
                comment_type: CommentType::Information,
                label: Some("label:".to_string()),
                name: "value:1".to_string(),
                url: Some("http://www.foo.bar".to_string()),
            },
            Comment {
                id: "c:2".to_string(),
                comment_type: CommentType::OnDemandTransport,
                label: Some("label:2".to_string()),
                name: "value:3".to_string(),
                url: Some("http://www.foo.bar".to_string()),
            },
            Comment {
                id: "c:3".to_string(),
                comment_type: CommentType::Information,
                label: None,
                name: "value:1".to_string(),
                url: None,
            },
        ])
        .unwrap();

        let booking_rules = CollectionWithId::new(vec![
            BookingRule {
                id: "odt:1".to_string(),
                name: Some("name:1".to_string()),
                booking_type: BookingType::RealTime,
                info_url: Some("https://reservation1".to_string()),
                phone: Some("01 02 03 04 01".to_string()),
                message: Some("lundi au vendredi de 9h à 18h".to_string()),
                booking_url: Some("https://deeplink1".to_string()),
                ..Default::default()
            },
            BookingRule {
                id: "odt:2".to_string(),
                name: None,
                booking_type: BookingType::SameDayWithPriorNotice,
                prior_notice_duration_min: Some(30),
                prior_notice_duration_max: Some(120),
                info_url: Some("https://reservation2".to_string()),
                phone: Some("01 02 03 04 02".to_string()),
                message: Some("lundi au samedi de 8h à 15h".to_string()),
                booking_url: Some("https://deeplink2".to_string()),
                ..Default::default()
            },
            BookingRule {
                id: "odt:3".to_string(),
                name: Some("name:3".to_string()),
                booking_type: BookingType::UpToPreviousDays,
                prior_notice_last_day: Some(2),
                prior_notice_last_time: Some(Time::new(18, 0, 0)),
                info_url: Some("https://reservation3".to_string()),
                phone: Some("01 02 03 04 03".to_string()),
                message: Some("lundi au mardi de 9h à 10h".to_string()),
                booking_url: Some("https://deeplink3".to_string()),
                ..Default::default()
            },
        ])
        .unwrap();

        let stop_points = CollectionWithId::from(StopPoint {
            id: "sp_1".to_string(),
            name: "sp_name_1".to_string(),
            codes: btree_set_from_vec(vec![(
                "object_system:1".to_string(),
                "object_code:1".to_string(),
            )]),
            object_properties: properties_map![(
                "prop_name:1".to_string(),
                "prop_value:1".to_string()
            )],
            comment_links: btree_set_from_vec(vec!["c:1".to_string()]),
            visible: true,
            coord: Coord {
                lon: 2.073_034,
                lat: 48.799_115,
            },
            stop_area_id: "sa_1".to_string(),
            stop_type: StopType::Point,
            ..Default::default()
        });

        let stop_areas = CollectionWithId::from(StopArea {
            id: "sa_1".to_string(),
            name: "sa_name_1".to_string(),
            codes: btree_set_from_vec(vec![(
                "object_system:2".to_string(),
                "object_code:2".to_string(),
            )]),
            object_properties: properties_map![(
                "prop_name:2".to_string(),
                "prop_value:2".to_string()
            )],
            comment_links: btree_set_from_vec(vec!["c:2".to_string()]),
            visible: true,
            coord: Coord {
                lon: 2.073_034,
                lat: 48.799_115,
            },
            timezone: None,
            geometry_id: None,
            equipment_id: None,
            level_id: Some("level1".to_string()),
            address_id: None,
        });

        let stop_locations: CollectionWithId<StopLocation> = CollectionWithId::default();

        let lines = CollectionWithId::from(Line {
            id: "OIF:002002003:3OIF829".to_string(),
            name: "3".to_string(),
            code: None,
            codes: btree_set_from_vec(vec![(
                "object_system:3".to_string(),
                "object_code:3".to_string(),
            )]),
            object_properties: properties_map![(
                "prop_name:3".to_string(),
                "prop_value:3".to_string()
            )],
            comment_links: btree_set_from_vec(vec!["c:1".to_string()]),
            booking_rule_links: btree_set_from_vec(vec!["odt:1".to_string()]),
            forward_name: None,
            backward_name: None,
            color: None,
            text_color: None,
            sort_order: None,
            network_id: "OIF:829".to_string(),
            commercial_mode_id: "bus".to_string(),
            geometry_id: None,
            opening_time: None,
            closing_time: None,
        });

        let routes = CollectionWithId::from(Route {
            id: "OIF:002002002:CEN".to_string(),
            name: "Hôtels - Hôtels".to_string(),
            direction_type: None,
            codes: btree_set_from_vec(vec![
                ("object_system:4".to_string(), "object_code:4".to_string()),
                ("object_system:5".to_string(), "object_code:5".to_string()),
            ]),
            object_properties: properties_map![(
                "prop_name:4".to_string(),
                "prop_value:4".to_string()
            )],
            comment_links: btree_set_from_vec(vec!["c:3".to_string()]),
            line_id: "OIF:002002002:BDEOIF829".to_string(),
            geometry_id: None,
            destination_id: None,
        });

        let vehicle_journeys = CollectionWithId::from(VehicleJourney {
            id: "VJ:1".to_string(),
            codes: btree_set_from_vec(vec![(
                "object_system:6".to_string(),
                "object_code:6".to_string(),
            )]),
            object_properties: properties_map![(
                "prop_name:6".to_string(),
                "prop_value:6".to_string()
            )],
            comment_links: LinksT::default(),
            booking_rule_links: btree_set_from_vec(vec!["odt:2".to_string()]),
            route_id: "OIF:800:TER".to_string(),
            physical_mode_id: "Bus".to_string(),
            dataset_id: "OIF:0".to_string(),
            service_id: "2".to_string(),
            headsign: None,
            short_name: Some("42".to_string()),
            block_id: None,
            company_id: "OIF:743".to_string(),
            trip_property_id: None,
            geometry_id: None,
            stop_times: vec![objects::StopTime {
                stop_point_idx: stop_points.get_idx("sp_1").unwrap(),
                sequence: 0,
                arrival_time: Some(Time::new(9, 0, 0)),
                departure_time: Some(Time::new(9, 2, 0)),
                start_pickup_drop_off_window: None,
                end_pickup_drop_off_window: None,
                boarding_duration: 2,
                alighting_duration: 3,
                pickup_type: 1,
                drop_off_type: 2,
                local_zone_id: None,
                precision: None,
            }],
            journey_pattern_id: None,
        });

        let networks = CollectionWithId::from(Network {
            id: "OIF:102".to_string(),
            name: "SAVAC".to_string(),
            url: None,
            timezone: None,
            lang: None,
            phone: None,
            address: None,
            fare_url: None,
            sort_order: None,
            codes: KeysValues::default(),
        });

        let mut stop_time_ids = HashMap::new();
        stop_time_ids.insert((("VJ:1").to_string(), 0), "StopTime:VJ:1:0".to_string());
        let mut stop_time_comments = HashMap::new();
        stop_time_comments.insert(("VJ:1".to_string(), 0), "c:2".to_string());

        ser_collections.comments = comments;
        ser_collections.booking_rules = booking_rules;
        ser_collections.stop_areas = stop_areas;
        ser_collections.stop_points = stop_points;
        ser_collections.stop_locations = stop_locations;
        ser_collections.lines = lines;
        ser_collections.routes = routes;
        ser_collections.vehicle_journeys = vehicle_journeys;
        ser_collections.networks = networks;
        ser_collections.stop_time_ids = stop_time_ids;
        ser_collections.stop_time_comments = stop_time_comments;

        test_in_tmp_dir(|path| {
            write_collection_with_id(path, "lines.txt", &ser_collections.lines).unwrap();
            write::write_stops(
                path,
                &ser_collections.stop_points,
                &ser_collections.stop_areas,
                &ser_collections.stop_locations,
            )
            .unwrap();
            write_collection_with_id(path, "routes.txt", &ser_collections.routes).unwrap();
            write_collection_with_id(path, "networks.txt", &ser_collections.networks).unwrap();
            write::write_vehicle_journeys_and_stop_times(
                path,
                &ser_collections.vehicle_journeys,
                &ser_collections.stop_points,
                &ser_collections.stop_time_headsigns,
                &ser_collections.stop_time_ids,
            )
            .unwrap();
            write::write_comments(path, &ser_collections).unwrap();
            write::write_booking_rules(path, &ser_collections).unwrap();
            write::write_codes(path, &ser_collections).unwrap();
            write::write_object_properties(path, &ser_collections).unwrap();
            let mut handler = PathFileHandler::new(path.to_path_buf());

            let mut des_collections = Collections {
                lines: make_collection_with_id(&mut handler, "lines.txt").unwrap(),
                routes: make_collection_with_id(&mut handler, "routes.txt").unwrap(),
                vehicle_journeys: make_collection_with_id(&mut handler, "trips.txt").unwrap(),
                networks: make_collection_with_id(&mut handler, "networks.txt").unwrap(),
                ..Default::default()
            };
            read::manage_stops(&mut des_collections, &mut handler).unwrap();
            read::manage_stop_times(&mut des_collections, &mut handler).unwrap();
            read::manage_comments(&mut des_collections, &mut handler).unwrap();
            read::manage_booking_rules(&mut des_collections, &mut handler).unwrap();
            read::manage_codes(&mut des_collections, &mut handler).unwrap();
            read::manage_object_properties(&mut des_collections, &mut handler).unwrap();

            assert_eq!(ser_collections.comments, des_collections.comments);
            assert_eq!(ser_collections.booking_rules, des_collections.booking_rules);

            // test comment links
            assert_eq!(
                ser_collections
                    .lines
                    .get("OIF:002002003:3OIF829")
                    .unwrap()
                    .comment_links,
                des_collections
                    .lines
                    .get("OIF:002002003:3OIF829")
                    .unwrap()
                    .comment_links
            );

            assert_eq!(
                ser_collections
                    .stop_points
                    .get("sp_1")
                    .unwrap()
                    .comment_links,
                des_collections
                    .stop_points
                    .get("sp_1")
                    .unwrap()
                    .comment_links
            );

            assert_eq!(
                ser_collections
                    .stop_points
                    .get("sp_1")
                    .unwrap()
                    .comment_links,
                des_collections
                    .stop_points
                    .get("sp_1")
                    .unwrap()
                    .comment_links
            );

            assert_eq!(
                ser_collections
                    .stop_areas
                    .get("sa_1")
                    .unwrap()
                    .comment_links,
                des_collections
                    .stop_areas
                    .get("sa_1")
                    .unwrap()
                    .comment_links
            );

            assert_eq!(
                ser_collections
                    .routes
                    .get("OIF:002002002:CEN")
                    .unwrap()
                    .comment_links,
                des_collections
                    .routes
                    .get("OIF:002002002:CEN")
                    .unwrap()
                    .comment_links
            );

            assert_eq!(
                ser_collections
                    .vehicle_journeys
                    .get("VJ:1")
                    .unwrap()
                    .comment_links,
                des_collections
                    .vehicle_journeys
                    .get("VJ:1")
                    .unwrap()
                    .comment_links
            );

            assert_eq!(
                ser_collections.stop_time_comments,
                des_collections.stop_time_comments
            );

            // test booking rule links
            assert_eq!(
                ser_collections
                    .lines
                    .get("OIF:002002003:3OIF829")
                    .unwrap()
                    .booking_rule_links,
                des_collections
                    .lines
                    .get("OIF:002002003:3OIF829")
                    .unwrap()
                    .booking_rule_links
            );

            assert_eq!(
                ser_collections
                    .vehicle_journeys
                    .get("VJ:1")
                    .unwrap()
                    .booking_rule_links,
                des_collections
                    .vehicle_journeys
                    .get("VJ:1")
                    .unwrap()
                    .booking_rule_links
            );

            // test codes
            assert_eq!(
                ser_collections
                    .lines
                    .get("OIF:002002003:3OIF829")
                    .unwrap()
                    .codes,
                des_collections
                    .lines
                    .get("OIF:002002003:3OIF829")
                    .unwrap()
                    .codes
            );

            assert_eq!(
                ser_collections.stop_points.get("sp_1").unwrap().codes,
                des_collections.stop_points.get("sp_1").unwrap().codes
            );

            assert_eq!(
                ser_collections.stop_points.get("sp_1").unwrap().codes,
                des_collections.stop_points.get("sp_1").unwrap().codes
            );

            assert_eq!(
                ser_collections.stop_areas.get("sa_1").unwrap().codes,
                des_collections.stop_areas.get("sa_1").unwrap().codes
            );

            assert_eq!(
                ser_collections
                    .routes
                    .get("OIF:002002002:CEN")
                    .unwrap()
                    .codes,
                des_collections
                    .routes
                    .get("OIF:002002002:CEN")
                    .unwrap()
                    .codes
            );

            assert_eq!(
                ser_collections.vehicle_journeys.get("VJ:1").unwrap().codes,
                des_collections.vehicle_journeys.get("VJ:1").unwrap().codes
            );

            assert_eq!(
                ser_collections.networks.get("OIF:102").unwrap().codes,
                des_collections.networks.get("OIF:102").unwrap().codes
            );
        });
    }

    #[test]
    fn trip_properties_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            TripProperty {
                id: "1".to_string(),
                wheelchair_accessible: Availability::Available,
                bike_accepted: Availability::NotAvailable,
                air_conditioned: Availability::InformationNotAvailable,
                visual_announcement: Availability::Available,
                audible_announcement: Availability::Available,
                appropriate_escort: Availability::Available,
                appropriate_signage: Availability::Available,
                school_vehicle_type: TransportType::Regular,
            },
            TripProperty {
                id: "2".to_string(),
                wheelchair_accessible: Availability::Available,
                bike_accepted: Availability::NotAvailable,
                air_conditioned: Availability::InformationNotAvailable,
                visual_announcement: Availability::Available,
                audible_announcement: Availability::Available,
                appropriate_escort: Availability::Available,
                appropriate_signage: Availability::Available,
                school_vehicle_type: TransportType::RegularAndSchool,
            },
        ]);
    }

    #[test]
    fn geometries_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            Geometry {
                id: "geo-id-1".to_string(),
                geometry:
                    line_string![(x: 2.541_951, y: 49.013_402), (x: 2.571_294, y: 49.004_725)]
                        .into(),
            },
            Geometry {
                id: "geo-id-2".to_string(),
                geometry:
                    line_string![(x: 2.548_309, y: 49.009_182), (x: 2.549_309, y: 49.009_253)]
                        .into(),
            },
        ]);
    }

    #[test]
    fn admin_stations_serialization_deserialization() {
        test_serialize_deserialize_collection(vec![
            AdminStation {
                admin_id: "admin:1".to_string(),
                admin_name: "Paris 12".to_string(),
                stop_id: "OIF:SA:8768600".to_string(),
            },
            AdminStation {
                admin_id: "admin:1".to_string(),
                admin_name: "Paris 12".to_string(),
                stop_id: "OIF:SA:8768666".to_string(),
            },
            AdminStation {
                admin_id: "admin:2".to_string(),
                admin_name: "Paris Nord".to_string(),
                stop_id: "OIF:SA:8727100".to_string(),
            },
        ]);
    }

    #[test]
    fn prices_v1_serialization_deserialization() {
        test_serialize_deserialize_collection(vec![
            PriceV1 {
                id: "PV1-01".to_string(),
                start_date: chrono::NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(),
                end_date: chrono::NaiveDate::from_ymd_opt(2019, 12, 31).unwrap(),
                price: 190,
                name: "Ticket PV1-01".to_string(),
                ignored: "".to_string(),
                comment: "Comment on PV1-01".to_string(),
                currency_type: Some("centime".to_string()),
            },
            PriceV1 {
                id: "PV1-02".to_string(),
                start_date: chrono::NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(),
                end_date: chrono::NaiveDate::from_ymd_opt(2019, 12, 31).unwrap(),
                price: 280,
                name: "Ticket PV1-02".to_string(),
                ignored: "".to_string(),
                comment: "".to_string(),
                currency_type: None,
            },
        ]);
    }

    #[test]
    fn od_fares_v1_serialization_deserialization() {
        test_serialize_deserialize_collection(vec![
            OdFareV1 {
                origin_stop_area_id: "stop_area:0:SA:8727114".to_string(),
                origin_name: Some("EPINAY-S/SEINE".to_string()),
                origin_mode: "stop".to_string(),
                destination_stop_area_id: "stop_area:0:SA:8727116".to_string(),
                destination_name: Some("PIERREFITTE-ST.".to_string()),
                destination_mode: "stop".to_string(),
                ticket_id: "29".to_string(),
            },
            OdFareV1 {
                origin_stop_area_id: "stop_area:0:SA:8773006".to_string(),
                origin_name: None,
                origin_mode: "zone".to_string(),
                destination_stop_area_id: "stop_area:0:SA:8775812".to_string(),
                destination_name: None,
                destination_mode: "stop".to_string(),
                ticket_id: "99-93".to_string(),
            },
        ]);
    }

    #[test]
    fn fares_v1_serialization_deserialization() {
        test_serialize_deserialize_collection(vec![
            FareV1 {
                before_change: "*".to_string(),
                after_change: "mode=physical_mode:Bus".to_string(),
                start_trip: "duration<90".to_string(),
                end_trip: "".to_string(),
                global_condition: "".to_string(),
                ticket_id: "".to_string(),
            },
            FareV1 {
                before_change: "*".to_string(),
                after_change: "network=network:0:56".to_string(),
                start_trip: "zone=1".to_string(),
                end_trip: "zone=1".to_string(),
                global_condition: "exclusive".to_string(),
                ticket_id: "tickett".to_string(),
            },
        ]);
    }

    #[test]
    fn tickets_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            Ticket {
                id: "PF1:Ticket1".to_string(),
                name: "Ticket name 1".to_string(),
                comment: Some("Some comment on ticket".to_string()),
            },
            Ticket {
                id: "PF2:Ticket2".to_string(),
                name: "Ticket name 1".to_string(),
                comment: None,
            },
        ]);
    }

    #[test]
    fn ticket_uses_serialization_deserialization() {
        test_serialize_deserialize_collection_with_id(vec![
            TicketUse {
                id: "PF1:TicketUse1".to_string(),
                ticket_id: "PF1:Ticket1".to_string(),
                max_transfers: Some(1),
                boarding_time_limit: Some(60),
                alighting_time_limit: Some(60),
            },
            TicketUse {
                id: "PF2:TicketUse2".to_string(),
                ticket_id: "PF2:Ticket2".to_string(),
                max_transfers: None,
                boarding_time_limit: None,
                alighting_time_limit: None,
            },
        ]);
    }

    #[test]
    fn ticket_prices_serialization_deserialization() {
        use rust_decimal_macros::dec;
        test_serialize_deserialize_collection(vec![
            TicketPrice {
                ticket_id: "PF1:Ticket1".to_string(),
                price: dec!(150.0),
                currency: "EUR".to_string(),
                ticket_validity_start: chrono::NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(),
                ticket_validity_end: chrono::NaiveDate::from_ymd_opt(2019, 12, 31).unwrap(),
            },
            TicketPrice {
                ticket_id: "PF2:Ticket2".to_string(),
                price: dec!(900.0),
                currency: "GHS".to_string(),
                ticket_validity_start: chrono::NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(),
                ticket_validity_end: chrono::NaiveDate::from_ymd_opt(2019, 12, 31).unwrap(),
            },
        ]);
    }

    #[test]
    fn ticket_use_perimeters_serialization_deserialization() {
        test_serialize_deserialize_collection(vec![
            TicketUsePerimeter {
                ticket_use_id: "PF1:TicketUse1".to_string(),
                object_type: ObjectType::Network,
                object_id: "PF1:Network1".to_string(),
                perimeter_action: PerimeterAction::Included,
            },
            TicketUsePerimeter {
                ticket_use_id: "PF1:TicketUse1".to_string(),
                object_type: ObjectType::Line,
                object_id: "PF2:Line2".to_string(),
                perimeter_action: PerimeterAction::Excluded,
            },
        ]);
    }

    #[test]
    fn ticket_use_restrictions_serialization_deserialization() {
        test_serialize_deserialize_collection(vec![
            TicketUseRestriction {
                ticket_use_id: "PF1:TicketUse1".to_string(),
                restriction_type: RestrictionType::OriginDestination,
                use_origin: "PF1:SA1".to_string(),
                use_destination: "PF1:SA2".to_string(),
            },
            TicketUseRestriction {
                ticket_use_id: "PF2:TicketUse2".to_string(),
                restriction_type: RestrictionType::Zone,
                use_origin: "PF2:ZO1".to_string(),
                use_destination: "PF2:ZO2".to_string(),
            },
        ]);
    }

    // ── NtfsSelector / read_collections_partial tests ─────────────────────────

    const PARTIAL_FIXTURE: &str = "tests/fixtures/restrict-validity-period/input";

    #[test]
    fn partial_load_nothing_gives_empty_collections() {
        let collections = read_collections_partial(PARTIAL_FIXTURE, NtfsSelector::none()).unwrap();
        assert!(
            collections.vehicle_journeys.is_empty(),
            "vehicle_journeys should be empty"
        );
        assert!(
            collections.stop_points.is_empty(),
            "stop_points should be empty"
        );
        assert!(
            collections.geometries.is_empty(),
            "geometries should be empty"
        );
        assert!(collections.lines.is_empty(), "lines should be empty");
    }

    #[test]
    fn partial_load_vehicle_journeys_only() {
        let collections = read_collections_partial(
            PARTIAL_FIXTURE,
            NtfsSelector::none().with_vehicle_journeys(),
        )
        .unwrap();
        assert!(
            !collections.vehicle_journeys.is_empty(),
            "vehicle_journeys should be populated"
        );
        assert!(
            collections.stop_points.is_empty(),
            "stop_points should remain empty"
        );
        assert!(
            collections.geometries.is_empty(),
            "geometries should remain empty"
        );
    }

    #[test]
    fn partial_load_geometries_and_trips() {
        let collections = read_collections_partial(
            PARTIAL_FIXTURE,
            NtfsSelector::none()
                .with_vehicle_journeys()
                .with_geometries(),
        )
        .unwrap();
        assert!(
            !collections.vehicle_journeys.is_empty(),
            "vehicle_journeys should be populated"
        );
        assert!(
            !collections.geometries.is_empty(),
            "geometries should be populated"
        );
        assert!(collections.lines.is_empty(), "lines should remain empty");
        assert!(
            collections.stop_points.is_empty(),
            "stop_points should remain empty"
        );
    }

    #[test]
    fn partial_load_stop_times_without_deps_is_silently_ignored() {
        // Selecting stop_times without vehicle_journeys or stop_points:
        // the flag is silently ignored (no error), and vehicle_journeys remains empty.
        let collections =
            read_collections_partial(PARTIAL_FIXTURE, NtfsSelector::none().with_stop_times())
                .unwrap();
        assert!(
            collections.vehicle_journeys.is_empty(),
            "vehicle_journeys should be empty when not selected"
        );
    }

    #[test]
    fn partial_load_all_matches_full_load() {
        let full = read_collections(PARTIAL_FIXTURE).unwrap();
        let partial = read_collections_partial(PARTIAL_FIXTURE, NtfsSelector::all()).unwrap();

        assert_eq!(
            full.vehicle_journeys.len(),
            partial.vehicle_journeys.len(),
            "vehicle_journeys count mismatch"
        );
        assert_eq!(
            full.stop_points.len(),
            partial.stop_points.len(),
            "stop_points count mismatch"
        );
        assert_eq!(
            full.geometries.len(),
            partial.geometries.len(),
            "geometries count mismatch"
        );
        assert_eq!(
            full.lines.len(),
            partial.lines.len(),
            "lines count mismatch"
        );
        assert_eq!(
            full.networks.len(),
            partial.networks.len(),
            "networks count mismatch"
        );
        assert_eq!(
            full.contributors.len(),
            partial.contributors.len(),
            "contributors count mismatch"
        );
    }
}
