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
    collection::{CollectionWithId, Id},
    common_format::{Availability, CalendarDate},
    model::Collections,
    objects::*,
    read_utils::FileHandler,
    Result,
};
use chrono::NaiveDate;
use csv;
use failure::{bail, format_err, ResultExt};
use geo::algorithm::centroid::Centroid;
use geo_types::MultiPoint as GeoMultiPoint;
use lazy_static::lazy_static;
use log::info;
use proj::Proj;
use serde_derive::Deserialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::result::Result as StdResult;

/// Deserialize kv1 string date (Y-m-d) to NaiveDate
fn de_from_date_string<'de, D>(deserializer: D) -> StdResult<Date, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let s = String::deserialize(deserializer)?;

    NaiveDate::parse_from_str(&s, "%Y-%m-%d").map_err(serde::de::Error::custom)
}

#[derive(Deserialize, Debug)]
struct OPerDay {
    #[serde(rename = "[OrganizationalUnitCode]")]
    org_unit_code: String,
    #[serde(rename = "[ScheduleCode]")]
    schedule_code: String,
    #[serde(rename = "[ScheduleTypeCode]")]
    schedule_type_code: String,
    #[serde(rename = "[ValidDate]", deserialize_with = "de_from_date_string")]
    valid_date: Date,
}

#[derive(Deserialize, Debug)]
struct Kv1Line {
    #[serde(rename = "[DataOwnerCode]")]
    data_owner_code: String,
    #[serde(rename = "[LinePlanningNumber]")]
    id: String,
    #[serde(rename = "[TransportType]")]
    transport_type: String,
}
impl_id!(Kv1Line);

#[derive(Deserialize, Debug, Hash, Eq, PartialEq)]
enum Accessibility {
    #[serde(rename = "ACCESSIBLE")]
    Accessible,
    #[serde(rename = "NOTACCESSIBLE")]
    NotAccessible,
    #[serde(rename = "UNKNOWN")]
    Unknown,
}

#[derive(Deserialize, Debug)]
struct PujoPass {
    #[serde(rename = "[OrganizationalUnitCode]")]
    organizational_unit_code: String,
    #[serde(rename = "[ScheduleCode]")]
    schedule_code: String,
    #[serde(rename = "[ScheduleTypeCode]")]
    schedule_type_code: String,
    #[serde(rename = "[LinePlanningNumber]")]
    line_planning_number: String,
    #[serde(rename = "[JourneyPatternCode]")]
    journey_pattern_code: String,
    #[serde(rename = "[JourneyNumber]")]
    journey_number: String,
    #[serde(rename = "[TargetArrivalTime]")]
    arrival_time: Time,
    #[serde(rename = "[TargetDepartureTime]")]
    departure_time: Time,
    #[serde(rename = "[UserStopCode]")]
    user_stop_code: String,
    #[serde(rename = "[StopOrder]")]
    stop_order: u32,
    #[serde(rename = "[WheelChairAccessible]")]
    wheelchair_accessible: Accessibility,
}

#[derive(Deserialize, Debug)]
struct Jopa {
    #[serde(rename = "[LinePlanningNumber]")]
    line_planning_number: String,
    #[serde(rename = "[Direction]")]
    direction: String,
    #[serde(rename = "[DataOwnerCode]")]
    data_owner_code: String,
    #[serde(rename = "[JourneyPatternCode]")]
    journey_pattern_code: String,
}

impl Jopa {
    fn route_id(&self) -> String {
        format!("{}:{}", self.line_planning_number, self.direction)
    }
}

lazy_static! {
    static ref MODES: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("BUS", "Bus");
        m.insert("TRAIN", "Train");
        m.insert("METRO", "Metro");
        m.insert("TRAM", "Tramway");
        m.insert("BOAT", "Ferry");
        m
    };
}

#[derive(Deserialize, Debug)]
struct Point {
    #[serde(rename = "[PointCode]")]
    code: String,
    #[serde(rename = "[LocationX_EW]")]
    lon: f64,
    #[serde(rename = "[LocationY_NS]")]
    lat: f64,
    #[serde(rename = "[PointType]")]
    category: String,
}

#[derive(Deserialize, Debug)]
struct UsrStopArea {
    #[serde(rename = "[UserStopAreaCode]")]
    id: String,
    #[serde(rename = "[Name]")]
    name: String,
}

#[derive(Deserialize, Debug)]
struct UsrStop {
    #[serde(rename = "[Name]")]
    name: String,
    #[serde(rename = "[UserStopAreaCode]")]
    parent_station: String,
    #[serde(rename = "[UserstopCode]")]
    point_code: String,
}

type PujoJopaMap = HashMap<(String, String), Vec<PujoPass>>;
type JopaMap<'a> = BTreeMap<(String, String), &'a Jopa>;

/// Generates calendars
pub fn read_operday<H>(file_handler: &mut H, collections: &mut Collections) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    let file = "OPERDAYXXX.TMI";
    let (reader, path) = file_handler.get_file(file)?;
    info!("Reading {}", file);

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(b'|')
        .trim(csv::Trim::All)
        .from_reader(reader);

    for opd in rdr.deserialize() {
        let opd: OPerDay = opd.with_context(ctx_from_path!(path))?;

        let calendar_date: CalendarDate = CalendarDate {
            service_id: format!(
                "{}:{}:{}",
                opd.org_unit_code, opd.schedule_code, opd.schedule_type_code
            ),
            date: opd.valid_date,
            exception_type: ExceptionType::Add,
        };

        let is_inserted = collections
            .calendars
            .get_mut(&calendar_date.service_id)
            .map(|mut calendar| {
                calendar.dates.insert(calendar_date.date);
            });

        is_inserted.unwrap_or_else(|| {
            let mut dates = BTreeSet::new();
            dates.insert(calendar_date.date);
            collections
                .calendars
                .push(Calendar {
                    id: calendar_date.service_id,
                    dates,
                })
                .unwrap();
        });
    }
    Ok(())
}

/// Generates physical and commercial modes
pub fn make_physical_and_commercial_modes(collections: &mut Collections) {
    let modes: BTreeSet<&str> = MODES.values().map(|m| *m).collect();
    for m in modes {
        collections
            .physical_modes
            .push(PhysicalMode {
                id: m.to_string(),
                name: m.to_string(),
                co2_emission: None,
            })
            .unwrap();
        collections
            .commercial_modes
            .push(CommercialMode {
                id: m.to_string(),
                name: m.to_string(),
            })
            .unwrap();
    }
}

/// Read stop coordinates
fn read_point<H>(file_handler: &mut H) -> Result<BTreeMap<String, Coord>>
where
    for<'a> &'a mut H: FileHandler,
{
    let file = "POINTXXXXX.TMI";
    let (file_reader, path) = file_handler.get_file(file)?;
    info!("Reading {}", file);

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(b'|')
        .trim(csv::Trim::All)
        .from_reader(file_reader);

    let mut point_map = BTreeMap::new();
    let from = "EPSG:28992";
    // FIXME: String 'EPSG:4326' is failing at runtime (string below is equivalent but works)
    let to = "+proj=longlat +datum=WGS84 +no_defs"; // See https://epsg.io/4326
    let proj = match Proj::new_known_crs(&from, &to, None) {
        Some(p) => p,
        None => bail!("Proj cannot build a converter from {} to {}", from, to),
    };
    for point in rdr.deserialize() {
        let point: Point = point.with_context(ctx_from_path!(path))?;
        if point.category == "SP" {
            let coords = proj.convert((point.lon, point.lat).into())?;
            let coords = Coord {
                lon: coords.x(),
                lat: coords.y(),
            };
            point_map.insert(point.code, coords);
        }
    }
    Ok(point_map)
}

/// Read stop areas
fn read_usrstar<H>(file_handler: &mut H) -> Result<BTreeMap<String, UsrStopArea>>
where
    for<'a> &'a mut H: FileHandler,
{
    let file = "USRSTARXXX.TMI";
    let (file_reader, path) = file_handler.get_file(file)?;
    info!("Reading {}", file);

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(b'|')
        .trim(csv::Trim::All)
        .from_reader(file_reader);
    let mut usr_stop_area_map = BTreeMap::new();
    for usr_stop_area in rdr.deserialize() {
        let usr_stop_area: UsrStopArea = usr_stop_area.with_context(ctx_from_path!(path))?;
        usr_stop_area_map.insert(usr_stop_area.id.clone(), usr_stop_area);
    }
    Ok(usr_stop_area_map)
}

/// Generates stop_points
pub fn read_usrstop_point<H>(file_handler: &mut H, collections: &mut Collections) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    let point_map = read_point(file_handler)?;
    let usr_stop_area_map = read_usrstar(file_handler)?;

    let file = "USRSTOPXXX.TMI";
    let (file_reader, path) = file_handler.get_file(file)?;
    info!("Reading {}", file);

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(b'|')
        .trim(csv::Trim::All)
        .from_reader(file_reader);

    for usr_stop in rdr.deserialize() {
        let usr_stop: UsrStop = usr_stop.with_context(ctx_from_path!(path))?;
        let coord = match point_map.get(&usr_stop.point_code) {
            Some(c) => c.clone(),
            None => bail!("Point code {} does not exist.", usr_stop.point_code),
        };
        let stop_area_id = match usr_stop_area_map.get(&usr_stop.parent_station) {
            Some(stop_area) => stop_area.id.clone(),
            None => bail!(
                "Stop Area with id {} does not exist.",
                usr_stop.parent_station
            ),
        };
        let stop_point = StopPoint {
            id: usr_stop.point_code,
            name: usr_stop.name,
            codes: KeysValues::default(),
            object_properties: KeysValues::default(),
            comment_links: CommentLinksT::default(),
            visible: true,
            coord,
            stop_area_id,
            timezone: None,
            geometry_id: None,
            equipment_id: None,
            fare_zone_id: None,
            stop_type: StopType::Point,
        };
        collections.stop_points.push(stop_point)?;
    }

    for (_, usr_stop_area) in usr_stop_area_map {
        let stop_points = &collections.stop_points;
        let coord = stop_points
            .values()
            .filter(|sp| sp.stop_area_id == usr_stop_area.id)
            .map(|sp| (sp.coord.lon, sp.coord.lat))
            .collect::<GeoMultiPoint<_>>()
            .centroid()
            .map(|c| Coord {lon: c.x(), lat: c.y()})
            .ok_or_else(||
                format_err!(
                    "Failed to calculate a barycenter of stop area {} because it doesn't refer to any corresponding stop point.",
                    usr_stop_area.id
                )
            )?;
        let stop_area = StopArea {
            id: usr_stop_area.id,
            name: usr_stop_area.name,
            codes: KeysValues::default(),
            object_properties: KeysValues::default(),
            comment_links: CommentLinksT::default(),
            visible: true,
            coord,
            timezone: None,
            geometry_id: None,
            equipment_id: None,
        };
        collections.stop_areas.push(stop_area)?;
    }

    Ok(())
}

fn read_jopa<H>(file_handler: &mut H) -> Result<Vec<Jopa>>
where
    for<'a> &'a mut H: FileHandler,
{
    let file = "JOPAXXXXXX.TMI";
    let (reader, path) = file_handler.get_file(file)?;
    info!("Reading {}", file);

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(b'|')
        .trim(csv::Trim::All)
        .from_reader(reader);

    let jopas = rdr
        .deserialize()
        .collect::<StdResult<_, _>>()
        .with_context(ctx_from_path!(path))?;
    Ok(jopas)
}

fn read_line<H>(file_handler: &mut H) -> Result<CollectionWithId<Kv1Line>>
where
    for<'a> &'a mut H: FileHandler,
{
    let file = "LINEXXXXXX.TMI";
    let (reader, path) = file_handler.get_file(file)?;
    info!("Reading {}", file);

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(b'|')
        .trim(csv::Trim::All)
        .from_reader(reader);
    let lines = rdr
        .deserialize()
        .collect::<StdResult<_, _>>()
        .with_context(ctx_from_path!(path))?;
    Ok(CollectionWithId::new(lines)?)
}

fn make_networks_and_companies<H>(
    collections: &mut Collections,
    lines: &CollectionWithId<Kv1Line>,
) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    let network_ids: HashSet<&str> = lines.values().map(|l| l.data_owner_code.as_ref()).collect();
    for n_id in network_ids {
        collections
            .networks
            .push(Network {
                id: n_id.to_string(),
                name: n_id.to_string(),
                url: None,
                codes: BTreeSet::new(),
                timezone: Some("Europe/Amsterdam".into()),
                lang: None,
                phone: None,
                address: None,
                sort_order: None,
            })
            .unwrap();

        collections
            .companies
            .push(Company {
                id: n_id.to_string(),
                name: n_id.to_string(),
                address: None,
                url: None,
                mail: None,
                phone: None,
            })
            .unwrap();
    }

    Ok(())
}

fn make_trip_properties(
    map_vj_accs: BTreeMap<String, HashSet<&Accessibility>>,
    collections: &mut Collections,
) -> Result<()> {
    let mut trip_properties: HashMap<Availability, TripProperty> = HashMap::new();
    let mut id_incr: u8 = 0;
    for (vj_id, acc) in map_vj_accs {
        let avaibility = {
            if acc.len() == 1 {
                match acc.iter().next() {
                    Some(&acc) if *acc == Accessibility::Accessible => Availability::Available,
                    Some(&acc) if *acc == Accessibility::NotAccessible => {
                        Availability::NotAvailable
                    }
                    _ => Availability::InformationNotAvailable,
                }
            } else {
                Availability::InformationNotAvailable
            }
        };
        let associated_trip_property = trip_properties.entry(avaibility).or_insert_with(|| {
            id_incr += 1;
            TripProperty {
                id: id_incr.to_string(),
                wheelchair_accessible: avaibility,
                bike_accepted: Availability::InformationNotAvailable,
                air_conditioned: Availability::InformationNotAvailable,
                visual_announcement: Availability::InformationNotAvailable,
                audible_announcement: Availability::InformationNotAvailable,
                appropriate_escort: Availability::InformationNotAvailable,
                appropriate_signage: Availability::InformationNotAvailable,
                school_vehicle_type: TransportType::Regular,
            }
        });

        let mut vj = collections.vehicle_journeys.get_mut(&vj_id).unwrap();
        vj.trip_property_id = Some(associated_trip_property.id.clone());
    }

    let mut trip_properties: Vec<_> = trip_properties.into_iter().map(|(_, tp)| tp).collect();
    trip_properties.sort_unstable_by(|tp1, tp2| tp1.id.cmp(&tp2.id));
    collections.trip_properties = CollectionWithId::new(trip_properties)?;

    Ok(())
}

fn read_pujopass<H>(file_handler: &mut H) -> Result<PujoJopaMap>
where
    for<'a> &'a mut H: FileHandler,
{
    let pujopass_file = "PUJOPASSXX.TMI";
    let (reader, path) = file_handler.get_file(pujopass_file)?;
    info!("Reading {}", pujopass_file);

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(b'|')
        .trim(csv::Trim::All)
        .from_reader(reader);

    let mut map: PujoJopaMap = HashMap::new();
    for pujopass in rdr.deserialize() {
        let pujopass: PujoPass = pujopass.with_context(ctx_from_path!(path))?;
        map.entry((
            pujopass.line_planning_number.clone(),
            pujopass.journey_number.clone(),
        ))
        .or_insert_with(|| vec![])
        .push(pujopass);
    }

    Ok(map)
}

fn make_vjs_and_stop_times<H>(
    file_handler: &mut H,
    collections: &mut Collections,
    jopas: &Vec<Jopa>,
    map_pujopass: &PujoJopaMap,
    lines: &CollectionWithId<Kv1Line>,
) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    let map_jopas: JopaMap = jopas
        .into_iter()
        .map(|obj| {
            (
                (
                    obj.line_planning_number.clone(),
                    obj.journey_pattern_code.clone(),
                ),
                obj,
            )
        })
        .collect();
    let mut id_vj: BTreeMap<String, VehicleJourney> = BTreeMap::new();
    let mut map_vj_accs: BTreeMap<String, HashSet<&Accessibility>> = BTreeMap::new();

    // there always is one dataset from config or a default one
    let dataset = collections.datasets.values().next().unwrap();
    for pujopass in map_pujopass.values().flat_map(|p| p) {
        let route_id = map_jopas
            .get(&(
                pujopass.line_planning_number.clone(),
                pujopass.journey_pattern_code.clone(),
            ))
            .map(|j| j.route_id())
            .ok_or_else(|| {
                format_err!(
                    "line_id={:?}, journey_pattern_code={:?} not found",
                    pujopass.line_planning_number,
                    pujopass.journey_pattern_code
                )
            })?;

        let line = lines.get(&pujopass.line_planning_number).ok_or_else(|| {
            format_err!(
                "line with line_planning_number={:?} not found",
                pujopass.line_planning_number
            )
        })?;
        let physical_mode_id = MODES
            .get::<str>(&line.transport_type)
            .map(|m| m.to_string())
            .ok_or_else(|| {
                format_err!(
                    "transport_type={:?} of line_id={:?} not found",
                    line.transport_type,
                    pujopass.line_planning_number
                )
            })?;

        let id = format!(
            "{}:{}:{}:{}",
            pujopass.line_planning_number,
            pujopass.journey_pattern_code,
            pujopass.journey_number,
            pujopass.schedule_code
        );

        let vj = id_vj.entry(id.clone()).or_insert(VehicleJourney {
            id: id.clone(),
            codes: KeysValues::default(),
            object_properties: KeysValues::default(),
            comment_links: CommentLinksT::default(),
            route_id,
            physical_mode_id,
            dataset_id: dataset.id.clone(),
            service_id: format!(
                "{}:{}:{}",
                pujopass.organizational_unit_code,
                pujopass.schedule_code,
                pujopass.schedule_type_code
            ),
            headsign: None,
            block_id: None,
            company_id: line.data_owner_code.clone(),
            trip_property_id: None,
            geometry_id: None,
            stop_times: vec![],
        });

        map_vj_accs
            .entry(id.clone())
            .or_insert_with(HashSet::new)
            .insert(&pujopass.wheelchair_accessible);

        let stop_id = &pujopass.user_stop_code;
        let stop_point_idx = collections
            .stop_points
            .get_idx(&stop_id)
            .ok_or_else(|| format_err!("stop_id={:?} not found", stop_id))?;

        vj.stop_times.push(StopTime {
            stop_point_idx,
            sequence: pujopass.stop_order,
            arrival_time: pujopass.arrival_time,
            departure_time: pujopass.departure_time,
            boarding_duration: 0,
            alighting_duration: 0,
            pickup_type: 0,
            drop_off_type: 0,
            datetime_estimated: false,
            local_zone_id: None,
        });
    }
    collections.vehicle_journeys =
        CollectionWithId::new(id_vj.into_iter().map(|(_, vj)| vj).collect())?;

    make_trip_properties(map_vj_accs, collections)?;

    Ok(())
}

fn get_route_origin_destination<'a>(
    collections: &'a Collections,
    route_id: &str,
) -> Option<(&'a StopPoint, &'a StopPoint)> {
    let stop_times = &collections
        .vehicle_journeys
        .values()
        .filter(|vj| vj.route_id == route_id)
        .min_by_key(|vj| &vj.id)? // TODO: instead of picking the first trip, find the most frequence origin (or destination) from all the trips
        .stop_times;
    let origin = &collections.stop_points[stop_times[0].stop_point_idx];
    let destination = &collections.stop_points[stop_times[stop_times.len() - 1].stop_point_idx];
    Some((origin, destination))
}

fn make_routes(collections: &mut Collections, jopas: &Vec<Jopa>) -> Result<()> {
    let jopas_map: JopaMap = jopas
        .into_iter()
        .map(|jopa| {
            (
                (jopa.line_planning_number.clone(), jopa.direction.clone()),
                jopa,
            )
        })
        .collect();
    for ((line_id, direction), jopa) in jopas_map {
        let id = jopa.route_id();
        let (origin, destination) =
            get_route_origin_destination(collections, &id).ok_or_else(|| {
                format_err!(
                    "Failed to find an origin and a destination for route {}",
                    id
                )
            })?;
        let name = format!("{} - {}", origin.name, destination.name);
        let destination_stop_area = collections
            .stop_areas
            .get(&destination.stop_area_id)
            .ok_or_else(|| {
                format_err!(
                    "The stop point {} doesn't have a corresponding stop area.",
                    destination.id
                )
            })?;
        let destination_id = Some(destination_stop_area.id.clone());
        let direction_type = if direction == "1" || direction == "A" {
            "forward"
        } else {
            "backward"
        };
        let direction_type = Some(direction_type.to_string());
        // TODO: Remove line below once line_id have been completed (ND-215)
        let line_id = "fake_line".into();
        let route = Route {
            id,
            name,
            direction_type,
            codes: KeysValues::default(),
            object_properties: KeysValues::default(),
            comment_links: CommentLinksT::default(),
            line_id,
            geometry_id: None,
            destination_id,
        };
        collections.routes.push(route)?;
    }
    Ok(())
}

fn make_fake_collections(collections: &mut Collections) -> Result<()> {
    collections.lines = CollectionWithId::new(vec![Line {
        id: "fake_line".into(),
        code: None,
        codes: KeysValues::default(),
        object_properties: KeysValues::default(),
        comment_links: CommentLinksT::default(),
        name: "fake_line".into(),
        forward_name: None,
        forward_direction: None,
        backward_name: None,
        backward_direction: None,
        color: Some(Rgb {
            red: 120,
            green: 125,
            blue: 125,
        }),
        text_color: None,
        sort_order: None,
        network_id: "SYNTUS".into(),
        commercial_mode_id: "Bus".into(),
        geometry_id: None,
        opening_time: None,
        closing_time: None,
    }])?;

    Ok(())
}

/// Generates networks, companies, stop_times, vehicle_journeys, routes and lines
pub fn read_jopa_pujopass_line<H>(file_handler: &mut H, collections: &mut Collections) -> Result<()>
where
    for<'a> &'a mut H: FileHandler,
{
    make_fake_collections(collections)?;

    let kv1_lines = read_line(file_handler)?;
    make_networks_and_companies(collections, &kv1_lines)?;

    let list_jopas = read_jopa(file_handler)?;
    let map_pujopas = read_pujopass(file_handler)?;
    make_vjs_and_stop_times(
        file_handler,
        collections,
        &list_jopas,
        &map_pujopas,
        &kv1_lines,
    )?;

    make_routes(collections, &list_jopas)?;

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
mod tests {
    use super::Collections;
    use crate::read_utils::PathFileHandler;
    use crate::test_utils::*;

    #[test]
    #[should_panic]
    fn read_operday_with_invalid_date() {
        let operday_content =
            "[OrganizationalUnitCode]|[ScheduleCode]|[ScheduleTypeCode]|[ValidDate]\n
                2029|1|1|20190428";

        test_in_tmp_dir(|path| {
            let mut handler = PathFileHandler::new(path.to_path_buf());
            create_file_with_content(path, "OPERDAYXXX.TMI", operday_content);
            let mut collections = Collections::default();
            super::read_operday(&mut handler, &mut collections).unwrap();
        });
    }

    #[test]
    fn make_physical_and_commercial_modes() {
        let mut collections = Collections::default();
        super::make_physical_and_commercial_modes(&mut collections);

        let expected = [
            ("Bus", "Bus"),
            ("Ferry", "Ferry"),
            ("Metro", "Metro"),
            ("Train", "Train"),
            ("Tramway", "Tramway"),
        ];

        let pms: Vec<(&str, &str)> = collections
            .physical_modes
            .values()
            .map(|pm| (pm.id.as_ref(), pm.name.as_ref()))
            .collect();

        let cms: Vec<(&str, &str)> = collections
            .commercial_modes
            .values()
            .map(|cm| (cm.id.as_ref(), cm.name.as_ref()))
            .collect();

        assert_eq!(pms, expected);
        assert_eq!(cms, expected);
    }
}
