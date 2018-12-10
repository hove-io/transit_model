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
mod write;

use collection::CollectionWithId;
use common_format;
use common_format::{manage_calendars, Availability};
use gtfs::read::EquipmentList;
use model::{Collections, Model};
use objects;
use objects::Time;
use read_utils;
use read_utils::{add_prefix, open_file, ZipHandler};
use std::fs::File;
use std::path::Path;
use utils::*;
use Result;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Agency {
    #[serde(rename = "agency_id")]
    id: Option<String>,
    #[serde(rename = "agency_name")]
    name: String,
    #[serde(rename = "agency_url")]
    url: String,
    #[serde(rename = "agency_timezone")]
    timezone: String,
    #[serde(rename = "agency_lang")]
    lang: Option<String>,
    #[serde(rename = "agency_phone")]
    phone: Option<String>,
    #[serde(rename = "agency_email")]
    email: Option<String>,
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
            timezone: obj
                .timezone
                .clone()
                .unwrap_or_else(|| "Europe/Paris".to_string()),
            lang: obj.lang.clone(),
            phone: obj.phone.clone(),
            email: None,
        }
    }
}

#[derivative(Default)]
#[derive(Derivative, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
enum StopLocationType {
    #[derivative(Default)]
    #[serde(rename = "0")]
    StopPoint,
    #[serde(rename = "1")]
    StopArea,
    #[serde(rename = "2")]
    StopEntrance,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Stop {
    #[serde(rename = "stop_id", deserialize_with = "de_without_slashes")]
    id: String,
    #[serde(rename = "stop_code")]
    code: Option<String>,
    #[serde(rename = "stop_name")]
    name: String,
    #[serde(default, rename = "stop_desc")]
    desc: String,
    #[serde(
        rename = "stop_lon",
        deserialize_with = "de_location_trim_with_default"
    )]
    lon: f64,
    #[serde(
        rename = "stop_lat",
        deserialize_with = "de_location_trim_with_default"
    )]
    lat: f64,
    #[serde(rename = "zone_id")]
    fare_zone_id: Option<String>,
    #[serde(rename = "stop_url")]
    url: Option<String>,
    #[serde(default, deserialize_with = "de_with_empty_default")]
    location_type: StopLocationType,
    #[serde(default, deserialize_with = "de_option_without_slashes")]
    parent_station: Option<String>,
    #[serde(rename = "stop_timezone")]
    timezone: Option<String>,
    #[serde(deserialize_with = "de_with_empty_default", default)]
    wheelchair_boarding: Availability,
}

#[derive(Derivative)]
#[derivative(Default)]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
enum DirectionType {
    #[derivative(Default)]
    #[serde(rename = "0")]
    Forward,
    #[serde(rename = "1")]
    Backward,
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
    arrival_time: Time,
    departure_time: Time,
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
}

#[derive(Serialize, Deserialize, Debug, Derivative, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, PartialEq)]
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
    let mut comments: CollectionWithId<objects::Comment> = CollectionWithId::default();

    //TODO
    // manage_calendars(
    //     open_file(&path, "calendar.txt").ok(),
    //     open_file(&path, "calendar_dates.txt").ok(),
    //     &mut collections,
    // )?;

    let (contributors, mut datasets) = read::read_config(config_path)?;
    read::set_dataset_validity_period(&mut datasets, &collections.calendars)?;

    collections.contributors = contributors;
    collections.datasets = datasets;

    let (networks, companies) = read::read_agency(open_file(&path, "agency.txt")?)?;
    collections.networks = networks;
    collections.companies = companies;
    let (stop_areas, stop_points) = read::read_stops(
        open_file(&path, "stops.txt")?,
        &mut comments,
        &mut equipments,
    )?;
    collections.transfers =
        read::read_transfers(open_file(&path, "transfers.txt").ok(), &stop_points)?;
    collections.stop_areas = stop_areas;
    collections.stop_points = stop_points;

    read::manage_shapes(&mut collections, open_file(&path, "shapes.txt").ok())?;

    read::read_routes(
        open_file(&path, "routes.txt")?,
        open_file(&path, "trips.txt")?,
        &mut collections,
    )?;
    collections.equipments = CollectionWithId::new(equipments.into_equipments())?;
    collections.comments = comments;
    read::manage_stop_times(&mut collections, open_file(&path, "stop_times.txt")?)?;
    read::manage_frequencies(&mut collections, open_file(&path, "frequencies.txt").ok())?;

    //add prefixes
    if let Some(prefix) = prefix {
        add_prefix(prefix, &mut collections)?;
    }

    Ok(Model::new(collections)?)
}

pub fn read2<'a, H, R>(
    file_handler: &'a mut H,
    config_path: Option<impl AsRef<Path>>,
    prefix: Option<String>,
) -> Result<Model>
where
    H: 'a + read_utils::FileHandler<'a, R>,
    R: 'a + std::io::Read,
{
    let mut collections = Collections::default();
    let mut equipments = EquipmentList::default();
    let mut comments: CollectionWithId<objects::Comment> = CollectionWithId::default();

    manage_calendars(file_handler, &mut collections)?;

    // let (contributors, mut datasets) = read::read_config(config_path)?;
    // read::set_dataset_validity_period(&mut datasets, &collections.calendars)?;

    // collections.contributors = contributors;
    // collections.datasets = datasets;

    // let (networks, companies) = read::read_agency(file_handler.get_file("agency.txt")?)?;
    // collections.networks = networks;
    // collections.companies = companies;
    // let (stop_areas, stop_points) = read::read_stops(
    //     file_handler.get_file("stops.txt")?,
    //     &mut comments,
    //     &mut equipments,
    // )?;
    // collections.transfers =
    //     read::read_transfers(file_handler.get_file("transfers.txt").ok(), &stop_points)?;
    // collections.stop_areas = stop_areas;
    // collections.stop_points = stop_points;

    // read::manage_shapes(&mut collections, file_handler.get_file("shapes.txt").ok())?;

    // read::read_routes(
    //     file_handler.get_file("routes.txt")?,
    //     file_handler.get_file("trips.txt")?,
    //     &mut collections,
    // )?;
    // collections.equipments = CollectionWithId::new(equipments.into_equipments())?;
    // collections.comments = comments;
    // read::manage_stop_times(&mut collections, file_handler.get_file("stop_times.txt")?)?;
    // read::manage_frequencies(&mut collections, file_handler.get_file("frequencies.txt").ok())?;

    // //add prefixes
    // if let Some(prefix) = prefix {
    //     add_prefix(prefix, &mut collections)?;
    // }

    Ok(Model::new(collections)?)
}


///TODO
pub fn read_from_path<P: AsRef<Path>>(
    p: P,
    config_path: Option<P>,
    prefix: Option<String>,
) -> Result<Model> {
    let mut file_handle = read_utils::PathFileHandler::new(p.as_ref().to_path_buf());
    // let reader = File::open(p)?;
    read2(&mut file_handle, config_path, prefix)
}

// pub fn from_zip<P: AsRef<Path>>(
//     p: P,
//     config_path: Option<P>,
//     prefix: Option<String>,
// ) -> Result<Model> {
//     let reader = File::open(p)?;
//     read_from_reader(reader, config_path, prefix)
// }

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum RouteType {
    #[allow(non_camel_case_types)]
    Tramway_LightRail,
    Metro,
    Rail,
    Bus,
    Ferry,
    CableCar,
    #[allow(non_camel_case_types)]
    Gondola_SuspendedCableCar,
    Funicular,
    Other(u16),
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

/// Exports a `Model` to [GTFS](http://gtfs.org/) files
/// in the given directory.
/// see [NTFS to GTFS conversion](https://github.com/CanalTP/navitia_model/blob/master/src/documentation/ntfs2gtfs.md)
pub fn write<P: AsRef<Path>>(model: &Model, path: P) -> Result<()> {
    let path = path.as_ref();
    info!("Writing GTFS to {:?}", path);

    write::write_transfers(path, &model.transfers)?;
    write::write_agencies(path, &model.networks)?;
    common_format::write_calendar_dates(path, &model.calendars)?;
    write::write_stops(
        path,
        &model.stop_points,
        &model.stop_areas,
        &model.comments,
        &model.equipments,
    )?;
    write::write_trips(
        path,
        &model.vehicle_journeys,
        &model.stop_points,
        &model.routes,
        &model.trip_properties,
    )?;
    write::write_routes(path, &model)?;
    write::write_stop_extensions(path, &model.stop_points, &model.stop_areas)?;
    write::write_stop_times(
        path,
        &model.vehicle_journeys,
        &model.stop_points,
        &model.stop_time_headsigns,
    )?;
    write::write_shapes(path, &model.geometries)?;

    Ok(())
}
