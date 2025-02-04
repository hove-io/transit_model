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

//! [GTFS](https://gtfs.org/reference/static) format management.

mod read;
mod write;

use crate::{
    calendars::{manage_calendars, write_calendar_dates},
    file_handler::{FileHandler, PathFileHandler, ZipHandler},
    model::{Collections, Model},
    objects::{self, Availability, Contributor, Dataset, Network, StopType, Time},
    parser::read_opt_collection,
    serde_utils::*,
    utils::*,
    validity_period, AddPrefix, PrefixConfiguration, Result,
};
use anyhow::{anyhow, Context};
use chrono_tz::Tz;
use derivative::Derivative;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fmt,
    path::Path,
};

use tracing::info;
use typed_index_collection::CollectionWithId;

#[cfg(all(feature = "gtfs", feature = "parser"))]
pub use read::{
    manage_frequencies, manage_pathways, manage_shapes, manage_stop_times, read_agency,
    read_routes, read_stops, read_transfers, EquipmentList,
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Agency {
    #[serde(rename = "agency_id")]
    id: Option<String>,
    #[serde(rename = "agency_name")]
    name: String,
    #[serde(rename = "agency_url")]
    url: String,
    #[serde(rename = "agency_timezone")]
    pub timezone: Tz,
    #[serde(rename = "agency_lang")]
    lang: Option<String>,
    #[serde(rename = "agency_phone")]
    phone: Option<String>,
    #[serde(rename = "agency_email")]
    email: Option<String>,
    #[serde(rename = "agency_fare_url")]
    fare_url: Option<String>,
    // Will not export attribute (and therefore csv column) if all values ​​are None
    #[serde(skip_serializing_if = "Option::is_none")]
    ticketing_deep_link_id: Option<String>,
}

impl<'a> From<&'a objects::Network> for Agency {
    fn from(obj: &objects::Network) -> Agency {
        Agency {
            id: Some(obj.id.clone()),
            name: obj.name.clone(),
            url: obj
                .url
                .clone()
                .unwrap_or_else(|| "http://www.navitia.io/".to_string()),
            timezone: obj.timezone.unwrap_or(chrono_tz::Europe::Paris),
            lang: obj.lang.clone(),
            phone: obj.phone.clone(),
            email: None,
            fare_url: obj.fare_url.clone(),
            ticketing_deep_link_id: None,
        }
    }
}

#[derive(Derivative, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[derivative(Default)]
enum StopLocationType {
    #[derivative(Default)]
    #[serde(rename = "0")]
    StopPoint,
    #[serde(rename = "1")]
    StopArea,
    #[serde(rename = "2")]
    StopEntrance,
    #[serde(rename = "3")]
    GenericNode,
    #[serde(rename = "4")]
    BoardingArea,
}

impl From<StopLocationType> for StopType {
    fn from(stop_location_type: StopLocationType) -> StopType {
        match stop_location_type {
            StopLocationType::StopPoint => StopType::Point,
            StopLocationType::StopArea => StopType::Zone,
            StopLocationType::StopEntrance => StopType::StopEntrance,
            StopLocationType::GenericNode => StopType::GenericNode,
            StopLocationType::BoardingArea => StopType::BoardingArea,
        }
    }
}

impl From<StopType> for StopLocationType {
    fn from(stop_type: StopType) -> StopLocationType {
        match stop_type {
            StopType::Point => StopLocationType::StopPoint,
            StopType::Zone => StopLocationType::StopArea,
            StopType::StopEntrance => StopLocationType::StopEntrance,
            StopType::GenericNode => StopLocationType::GenericNode,
            StopType::BoardingArea => StopLocationType::BoardingArea,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Stop {
    #[serde(rename = "stop_id", deserialize_with = "de_without_slashes")]
    id: String,
    #[serde(rename = "stop_code")]
    code: Option<String>,
    #[serde(rename = "stop_name")]
    name: String,
    #[serde(
        default,
        rename = "stop_desc",
        deserialize_with = "de_option_empty_string"
    )]
    desc: Option<String>,
    #[serde(rename = "stop_lon")]
    lon: String,
    #[serde(rename = "stop_lat")]
    lat: String,
    #[serde(rename = "zone_id")]
    fare_zone_id: Option<String>,
    #[serde(rename = "stop_url")]
    url: Option<String>,
    #[serde(default, deserialize_with = "de_with_empty_default")]
    location_type: StopLocationType,
    #[serde(default, deserialize_with = "de_option_without_slashes")]
    parent_station: Option<String>,
    #[serde(rename = "stop_timezone")]
    timezone: Option<Tz>,
    level_id: Option<String>,
    #[serde(deserialize_with = "de_with_empty_default", default)]
    wheelchair_boarding: Availability,
    platform_code: Option<String>,
}

#[derive(Derivative)]
#[derivative(Default)]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum DirectionType {
    #[derivative(Default)]
    #[serde(rename = "0")]
    Forward = 0,
    #[serde(rename = "1")]
    Backward = 1,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Trip {
    route_id: String,
    service_id: String,
    #[serde(rename = "trip_id")]
    id: String,
    #[serde(rename = "trip_headsign")]
    headsign: Option<String>,
    #[serde(rename = "trip_short_name")]
    short_name: Option<String>,
    #[serde(
        default,
        deserialize_with = "de_with_empty_default",
        rename = "direction_id"
    )]
    direction: DirectionType,
    block_id: Option<String>,
    #[serde(default, deserialize_with = "de_option_without_slashes")]
    shape_id: Option<String>,
    #[serde(deserialize_with = "de_with_empty_default", default)]
    wheelchair_accessible: Availability,
    #[serde(deserialize_with = "de_with_empty_default", default)]
    bikes_allowed: Availability,
}

fn default_true_bool() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct StopTime {
    trip_id: String,
    arrival_time: Option<Time>,
    departure_time: Option<Time>,
    start_pickup_drop_off_window: Option<Time>,
    end_pickup_drop_off_window: Option<Time>,
    #[serde(deserialize_with = "de_without_slashes")]
    stop_id: String,
    stop_sequence: u32,
    #[serde(deserialize_with = "de_with_empty_default", default)]
    pickup_type: u8,
    #[serde(deserialize_with = "de_with_empty_default", default)]
    drop_off_type: u8,
    local_zone_id: Option<u16>,
    stop_headsign: Option<String>,
    #[serde(
        deserialize_with = "de_from_u8_with_true_default",
        serialize_with = "ser_from_bool",
        default = "default_true_bool"
    )]
    timepoint: bool,
    pickup_booking_rule_id: Option<String>,
    drop_off_booking_rule_id: Option<String>,
}

#[derive(Derivative, Serialize)]
#[derivative(Default)]
enum BookingType {
    #[derivative(Default)]
    #[serde(rename = "0")]
    RealTime,
}

#[derive(Derivative, Serialize)]
#[derivative(Default)]
struct BookingRule {
    #[serde(rename = "booking_rule_id")]
    id: String,
    booking_type: BookingType,
    message: Option<String>,
    phone_number: Option<String>,
    info_url: Option<String>,
    booking_url: Option<String>,
}

impl<'a> From<&'a objects::BookingRule> for BookingRule {
    fn from(obj: &objects::BookingRule) -> BookingRule {
        BookingRule {
            id: obj.id.clone(),
            message: obj.message.clone(),
            phone_number: obj.phone.clone(),
            info_url: obj.info_url.clone(),
            booking_url: obj.booking_url.clone(),
            ..Default::default()
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Derivative, PartialEq, Clone)]
#[derivative(Default)]
enum TransferType {
    #[derivative(Default)]
    #[serde(rename = "0")]
    Recommended,
    #[serde(rename = "1")]
    Timed,
    #[serde(rename = "2")]
    WithTransferTime,
    #[serde(rename = "3")]
    NotPossible,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct Transfer {
    #[serde(deserialize_with = "de_without_slashes")]
    from_stop_id: String,
    #[serde(deserialize_with = "de_without_slashes")]
    to_stop_id: String,
    #[serde(deserialize_with = "de_with_empty_default")]
    transfer_type: TransferType,
    min_transfer_time: Option<u32>,
}

impl<'a> From<&'a objects::Transfer> for Transfer {
    fn from(obj: &objects::Transfer) -> Transfer {
        Transfer {
            from_stop_id: obj.from_stop_id.clone(),
            to_stop_id: obj.to_stop_id.clone(),
            transfer_type: TransferType::WithTransferTime,
            min_transfer_time: obj.min_transfer_time,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Shape {
    #[serde(rename = "shape_id", deserialize_with = "de_without_slashes")]
    id: String,
    #[serde(rename = "shape_pt_lat")]
    lat: f64,
    #[serde(rename = "shape_pt_lon")]
    lon: f64,
    #[serde(rename = "shape_pt_sequence")]
    sequence: u32,
}

#[derive(Serialize, Debug)]
struct TicketingDeepLink {
    #[serde(rename = "ticketing_deep_link_id")]
    id: String,
    web_url: Option<String>,
    android_intent_uri: Option<String>,
    ios_universal_link_url: Option<String>,
}

type TicketingDeepLinks = HashMap<String, TicketingDeepLink>;

///parameters consolidation
#[derive(Default)]
pub struct Configuration {
    /// The Contributor providing the Dataset
    pub contributor: Contributor,
    /// Describe the Dataset being parsed
    pub dataset: Dataset,
    /// Additional key-values for the 'feed_infos.txt'
    pub feed_infos: BTreeMap<String, String>,
    /// used to prefix objects
    pub prefix_conf: Option<PrefixConfiguration>,
    /// stop time precision management
    pub on_demand_transport: bool,
    /// on demand transport comment template
    pub on_demand_transport_comment: Option<String>,
    /// If true, each GTFS `Route` will generate a different `Line`.
    /// Else we group the routes by `agency_id` and `route_short_name`
    /// (or `route_long_name` if the short name is empty) and create a `Line` for each group.
    pub read_as_line: bool,
}

fn read_file_handler<H>(file_handler: &mut H, configuration: Configuration) -> Result<Model>
where
    for<'a> &'a mut H: FileHandler,
{
    let collections = read_file_handler_to_collections(file_handler, configuration)?;
    Model::new(collections)
}

fn read_file_handler_to_collections<H>(
    file_handler: &mut H,
    configuration: Configuration,
) -> Result<Collections>
where
    for<'a> &'a mut H: FileHandler,
{
    let mut collections = Collections::default();
    let mut equipments = read::EquipmentList::default();

    let Configuration {
        contributor,
        mut dataset,
        feed_infos,
        prefix_conf,
        on_demand_transport,
        on_demand_transport_comment,
        read_as_line,
    } = configuration;

    manage_calendars(file_handler, &mut collections)?;
    validity_period::compute_dataset_validity_period(&mut dataset, &collections.calendars)?;

    collections.contributors = CollectionWithId::from(contributor);
    collections.datasets = CollectionWithId::from(dataset);
    collections.feed_infos = feed_infos;

    let (networks, companies) = read::read_agency(file_handler)?;
    collections.networks = networks;
    collections.companies = companies;
    let (stop_areas, stop_points, stop_locations) =
        read::read_stops(file_handler, &mut collections.comments, &mut equipments)?;
    collections.transfers = read::read_transfers(file_handler, &stop_points, &stop_areas)?;
    collections.stop_areas = stop_areas;
    collections.stop_points = stop_points;
    collections.stop_locations = stop_locations;

    read::manage_shapes(&mut collections, file_handler)?;

    read::read_routes(file_handler, &mut collections, read_as_line)?;
    collections.equipments = CollectionWithId::new(equipments.into_equipments())?;
    read::manage_stop_times(
        &mut collections,
        file_handler,
        on_demand_transport,
        on_demand_transport_comment,
    )?;
    read::manage_frequencies(&mut collections, file_handler)?;
    read::manage_pathways(&mut collections, file_handler)?;
    collections.levels = read_opt_collection(file_handler, "levels.txt")?;

    //add prefixes
    if let Some(prefix_conf) = prefix_conf {
        collections.prefix(&prefix_conf);
    }

    collections.calendar_deduplication();
    Ok(collections)
}

/// Imports a `Model` from the [GTFS](https://gtfs.org/reference/static)
/// files in the `path` directory.
///
/// The `Configuration` is used to control various parameters during the import.
pub fn from_dir<P: AsRef<Path>>(p: P) -> Result<Model> {
    Reader::default().parse_dir(p)
}

/// Imports a `Model` from a zip file containing the
/// [GTFS](https://gtfs.org/reference/static).
pub fn from_zip<P: AsRef<Path>>(p: P) -> Result<Model> {
    Reader::default().parse_zip(p)
}

/// Imports a `Model` from an object implementing `Read` and `Seek` and containing the
/// [GTFS](https://gtfs.org/reference/static).
///
/// This method makes it possible to read from a variety of sources like read a GTFS
/// from the network.
///
/// ```ignore
/// let url = "http://some_url/gtfs.zip";
/// let resp = reqwest::blocking::get(url)?; // or async call
/// let data = std::io::Cursor::new(resp.bytes()?.to_vec());
/// let model = transit_model::gtfs::from_zip_reader(data, &url)?;
/// # Ok::<(), transit_model::Error>(())
/// ```
///
/// The `source_name` is needed to have nicer error messages.
pub fn from_zip_reader<R>(reader: R, source_name: &str) -> Result<Model>
where
    R: std::io::Seek + std::io::Read,
{
    Reader::default().parse_zip_reader(reader, source_name)
}

/// Imports a `Model` from the
/// [GTFS](https://gtfs.org/reference/static).
/// files in the given directory.
/// This method will try to detect if the input is a zipped archive or not.
/// If the default file type mechanism is not enough, you can use
/// [from_zip] or [from_dir].
pub fn read<P: AsRef<Path>>(p: P) -> Result<Model> {
    Reader::default().parse(p)
}

/// Structure to configure the GTFS reading
#[derive(Default)]
pub struct Reader {
    configuration: Configuration,
}

impl Reader {
    /// Build a Reader with a custom configuration
    pub fn new(configuration: Configuration) -> Self {
        Self { configuration }
    }

    /// Imports a `Model` from the
    /// [GTFS](https://gtfs.org/reference/static).
    /// files in the given directory.
    /// This method will try to detect if the input is a zipped archive or not.
    /// If the default file type mechanism is not enough, you can use
    /// [Reader::parse_zip] or [Reader::parse_dir].
    pub fn parse(self, path: impl AsRef<Path>) -> Result<Model> {
        let p = path.as_ref();
        if p.is_file() {
            // if it's a file, we consider it to be a zip (and an error will be returned if it is not)
            Ok(self
                .parse_zip(p)
                .with_context(|| format!("impossible to read zipped gtfs {:?}", p))?)
        } else if p.is_dir() {
            Ok(self
                .parse_dir(p)
                .with_context(|| format!("impossible to read gtfs directory from {:?}", p))?)
        } else {
            Err(anyhow!(
                "file {:?} is neither a file nor a directory, cannot read a gtfs from it",
                p
            ))
        }
    }
    /// Imports `Collections` from the
    /// [GTFS](https://gtfs.org/reference/static).
    /// files in the given directory.
    /// This method will try to detect if the input is a zipped archive or not.
    pub fn parse_collections(self, path: impl AsRef<Path>) -> Result<Collections> {
        let p = path.as_ref();
        if p.is_file() {
            // if it's a file, we consider it to be a zip (and an error will be returned if it is not)
            Ok(self
                .parse_zip_collections(p)
                .with_context(|| format!("impossible to read zipped gtfs {:?}", p))?)
        } else if p.is_dir() {
            Ok(self
                .parse_dir_collections(p)
                .with_context(|| format!("impossible to read gtfs directory from {:?}", p))?)
        } else {
            Err(anyhow!(
                "file {:?} is neither a file nor a directory, cannot read a gtfs from it",
                p
            ))
        }
    }

    /// Imports a `Model` from a zip file containing the
    /// [GTFS](https://gtfs.org/reference/static).
    pub fn parse_zip(self, path: impl AsRef<Path>) -> Result<Model> {
        let collections = self.parse_zip_collections(path)?;
        Model::new(collections)
    }

    /// Imports a `Model` from the [GTFS](https://gtfs.org/reference/static)
    /// files in the `path` directory.
    pub fn parse_dir(self, path: impl AsRef<Path>) -> Result<Model> {
        let collections = self.parse_dir_collections(path)?;
        Model::new(collections)
    }

    /// Imports `Collections` from the [GTFS](https://gtfs.org/reference/static)
    /// files in the `path` directory.
    fn parse_dir_collections(self, path: impl AsRef<Path>) -> Result<Collections> {
        let mut file_handler = PathFileHandler::new(path.as_ref().to_path_buf());
        read_file_handler_to_collections(&mut file_handler, self.configuration)
    }

    /// Imports `Collections` from a zip file containing the
    /// [GTFS](https://gtfs.org/reference/static).
    fn parse_zip_collections(self, path: impl AsRef<Path>) -> Result<Collections> {
        let reader = std::fs::File::open(path.as_ref())?;
        let mut file_handler = ZipHandler::new(reader, path)?;
        read_file_handler_to_collections(&mut file_handler, self.configuration)
    }

    /// Imports a `Model` from an object implementing `Read` and `Seek` and containing the
    /// [GTFS](https://gtfs.org/reference/static).
    ///
    /// This method makes it possible to read from a variety of sources like read a GTFS
    /// from the network.
    ///
    /// ```ignore
    /// let url = "http://some_url/gtfs.zip";
    /// let resp = reqwest::blocking::get(url)?; // or async call
    /// let data = std::io::Cursor::new(resp.bytes()?.to_vec());
    /// let model = transit_model::gtfs::Reader::default().parse_zip_reader(data, &url)?;
    /// # Ok::<(), transit_model::Error>(())
    /// ```
    ///
    /// The `source_name` is needed to have nicer error messages.
    pub fn parse_zip_reader<R>(self, reader: R, source_name: &str) -> Result<Model>
    where
        R: std::io::Seek + std::io::Read,
    {
        let mut file_handler = ZipHandler::new(reader, source_name)?;
        read_file_handler(&mut file_handler, self.configuration)
    }
}

#[derive(PartialOrd, Ord, Debug, Clone, Eq, PartialEq, Hash)]
enum RouteType {
    Tramway,
    Metro,
    Train,
    Bus,
    Ferry,
    CableCar,
    SuspendedCableCar,
    Funicular,
    Coach,
    Air,
    Taxi,
    UnknownMode,
}
impl fmt::Display for RouteType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Route {
    #[serde(rename = "route_id")]
    id: String,
    agency_id: Option<String>,
    #[serde(rename = "route_short_name")]
    short_name: String,
    #[serde(rename = "route_long_name")]
    long_name: String,
    #[serde(rename = "route_desc")]
    desc: Option<String>,
    route_type: RouteType,
    #[serde(rename = "route_url")]
    url: Option<String>,
    #[serde(
        rename = "route_color",
        default,
        deserialize_with = "de_with_empty_or_invalid_default"
    )]
    color: Option<objects::Rgb>,
    #[serde(
        rename = "route_text_color",
        default,
        deserialize_with = "de_with_empty_or_invalid_default"
    )]
    text_color: Option<objects::Rgb>,
    #[serde(rename = "route_sort_order")]
    sort_order: Option<u32>,
}

/// Use to serialize extended route type
/// For more information, see \
/// https://developers.google.com/transit/gtfs/reference/extended-route-types"
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct ExtendedRoute {
    #[serde(rename = "route_id")]
    id: String,
    agency_id: Option<String>,
    #[serde(rename = "route_short_name")]
    short_name: String,
    #[serde(rename = "route_long_name")]
    long_name: String,
    #[serde(rename = "route_desc")]
    desc: Option<String>,
    #[serde(serialize_with = "ser_from_route_type_extended")]
    route_type: RouteType,
    #[serde(rename = "route_url")]
    url: Option<String>,
    #[serde(
        rename = "route_color",
        default,
        deserialize_with = "de_with_empty_or_invalid_default"
    )]
    color: Option<objects::Rgb>,
    #[serde(
        rename = "route_text_color",
        default,
        deserialize_with = "de_with_empty_or_invalid_default"
    )]
    text_color: Option<objects::Rgb>,
    #[serde(rename = "route_sort_order")]
    sort_order: Option<u32>,
}

impl From<Route> for ExtendedRoute {
    fn from(route: Route) -> Self {
        Self {
            id: route.id,
            agency_id: route.agency_id,
            short_name: route.short_name,
            long_name: route.long_name,
            desc: route.desc,
            route_type: route.route_type,
            url: route.url,
            color: route.color,
            text_color: route.text_color,
            sort_order: route.sort_order,
        }
    }
}

fn to_gtfs_extended_value(route_type: &RouteType) -> String {
    match *route_type {
        RouteType::Tramway => "900".to_string(),
        RouteType::Metro => "400".to_string(),
        RouteType::Train => "100".to_string(),
        RouteType::Bus | RouteType::UnknownMode => "700".to_string(),
        RouteType::Ferry => "1200".to_string(),
        RouteType::Funicular => "1400".to_string(),
        RouteType::CableCar | RouteType::SuspendedCableCar => "1300".to_string(),
        RouteType::Coach => "200".to_string(),
        RouteType::Air => "1100".to_string(),
        RouteType::Taxi => "1500".to_string(),
    }
}

fn get_ticketing_deep_links(networks: &CollectionWithId<Network>) -> TicketingDeepLinks {
    networks
        .values()
        .filter_map(|n| n.fare_url.clone())
        .collect::<BTreeSet<_>>()
        .iter()
        .enumerate()
        .map(|(i, fare_url)| {
            (
                fare_url.clone(),
                TicketingDeepLink {
                    id: format!("ticketing_deep_link:{}", i + 1),
                    web_url: Some(fare_url.clone()),
                    android_intent_uri: Some(fare_url.clone()),
                    ios_universal_link_url: Some(fare_url.clone()),
                },
            )
        })
        .collect::<TicketingDeepLinks>()
}

fn ser_from_route_type_extended<S>(r: &RouteType, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&to_gtfs_extended_value(r))
}

/// Exports a `Model` to [GTFS](https://gtfs.org/reference/static) files
/// in the given directory.
/// see [NTFS to GTFS conversion](https://github.com/hove-io/transit_model/blob/master/src/documentation/ntfs2gtfs.md)
pub fn write<P: AsRef<Path>>(model: Model, path: P, extend_route_type: bool) -> Result<()> {
    let path = path.as_ref();
    std::fs::create_dir_all(path)?;
    info!("Writing GTFS to {:?}", path);

    let ticketing_deep_links = get_ticketing_deep_links(&model.networks);
    write::write_transfers(path, &model.transfers)?;
    write::write_ticketing_deep_links(path, &ticketing_deep_links)?;
    write::write_agencies(path, &model.networks, &ticketing_deep_links)?;
    write_calendar_dates(path, &model.calendars)?;
    write::write_stops(
        path,
        &model.stop_points,
        &model.stop_areas,
        &model.stop_locations,
        &model.comments,
        &model.equipments,
    )?;
    write::write_trips(path, &model)?;
    write::write_routes(path, &model, extend_route_type)?;
    write::write_stop_extensions(path, &model.stop_points, &model.stop_areas)?;
    write::write_stop_times(
        path,
        &model.vehicle_journeys,
        &model.stop_points,
        &model.stop_time_headsigns,
    )?;
    write::write_booking_rules(path, &model.booking_rules)?;
    write::write_shapes(path, &model.geometries)?;
    write_collection_with_id(path, "pathways.txt", &model.pathways)?;
    write_collection_with_id(path, "levels.txt", &model.levels)?;

    Ok(())
}

/// Exports a `Model` to [GTFS](https://gtfs.org/reference/static) files
/// in the given ZIP archive.
/// see [NTFS to GTFS conversion](https://github.com/hove-io/transit_model/blob/master/src/documentation/ntfs2gtfs.md)
pub fn write_to_zip<P: AsRef<std::path::Path>>(
    model: Model,
    path: P,
    extend_route_type: bool,
) -> Result<()> {
    let path = path.as_ref();
    info!("Writing GTFS to ZIP File {:?}", path);
    let input_tmp_dir = tempfile::tempdir()?;
    write(model, input_tmp_dir.path(), extend_route_type)?;
    zip_to(input_tmp_dir.path(), path)?;
    input_tmp_dir.close()?;
    Ok(())
}
