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

use collection::{Id, Idx};
use utils::*;
use chrono;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ObjectType {
    StopArea,
    StopPoint,
    Network,
    Line,
    Route,
    #[serde(rename = "trip")]
    VehicleJourney,
    StopTime,
    LineGroup,
}

pub trait GetObjectType {
    fn get_object_type() -> ObjectType;
}

impl ObjectType {
    pub fn as_str(&self) -> &'static str {
        match *self {
            ObjectType::StopArea => "stop_area",
            ObjectType::StopPoint => "stop_point",
            ObjectType::Network => "network",
            ObjectType::Line => "line",
            ObjectType::Route => "route",
            ObjectType::VehicleJourney => "trip",
            ObjectType::StopTime => "stop_time",
            ObjectType::LineGroup => "line_group",
        }
    }
}

// We use a Vec here for memory efficiency.  Other possible types can
// be something like BTreeSet<(String,String)> or
// BTreeMap<String,Vec<String>>.  Hash{Map,Set} are memory costy.
pub type KeysValues = Vec<(String, String)>;

pub trait Codes {
    fn codes(&self) -> &KeysValues;
    fn codes_mut(&mut self) -> &mut KeysValues;
}
macro_rules! impl_codes {
    ($ty:ty) => {
        impl Codes for $ty {
            fn codes(&self) -> &KeysValues {
                &self.codes
            }
            fn codes_mut(&mut self) -> &mut KeysValues {
                &mut self.codes
            }
        }
    };
}

pub trait ObjectProperties {
    fn object_properties(&self) -> &KeysValues;
    fn object_properties_mut(&mut self) -> &mut KeysValues;
}
macro_rules! impl_object_properties {
    ($ty:ty) => {
        impl ObjectProperties for $ty {
            fn object_properties(&self) -> &KeysValues {
                &self.object_properties
            }
            fn object_properties_mut(&mut self) -> &mut KeysValues {
                &mut self.object_properties
            }
        }
    };
}

pub type CommentLinksT = Vec<String>;

pub trait CommentLinks {
    fn comment_links(&self) -> &CommentLinksT;
    fn comment_links_mut(&mut self) -> &mut CommentLinksT;
}
macro_rules! impl_comment_links {
    ($ty:ty) => {
        impl CommentLinks for $ty {
            fn comment_links(&self) -> &CommentLinksT {
                &self.comment_links
            }
            fn comment_links_mut(&mut self) -> &mut CommentLinksT {
                &mut self.comment_links
            }
        }
    };
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Contributor {
    #[serde(rename = "contributor_id")]
    pub id: String,
    #[serde(rename = "contributor_name")]
    pub name: String,
    #[serde(rename = "contributor_license")]
    pub license: Option<String>,
    #[serde(rename = "contributor_website")]
    pub website: Option<String>,
}
impl Id<Contributor> for Contributor {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum DatasetType {
    #[serde(rename = "0")]
    Theorical,
    #[serde(rename = "1")]
    Revised,
    #[serde(rename = "2")]
    Production,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Dataset {
    #[serde(rename = "dataset_id")]
    pub id: String,
    pub contributor_id: String,
    #[serde(rename = "dataset_start_date", deserialize_with = "de_from_date_string",
            serialize_with = "ser_from_naive_date")]
    pub start_date: Date,
    #[serde(rename = "dataset_end_date", deserialize_with = "de_from_date_string",
            serialize_with = "ser_from_naive_date")]
    pub end_date: Date,
    pub dataset_type: Option<DatasetType>,
    #[serde(rename = "dataset_extrapolation", default, deserialize_with = "de_from_u8",
            serialize_with = "ser_from_bool")]
    pub extrapolation: bool,
    #[serde(rename = "dataset_desc")]
    pub desc: Option<String>,
    #[serde(rename = "dataset_system")]
    pub system: Option<String>,
}
impl Id<Dataset> for Dataset {
    fn id(&self) -> &str {
        &self.id
    }
}
impl Id<Contributor> for Dataset {
    fn id(&self) -> &str {
        &self.contributor_id
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct CommercialMode {
    #[serde(rename = "commercial_mode_id")]
    pub id: String,
    #[serde(rename = "commercial_mode_name")]
    pub name: String,
}
impl Id<CommercialMode> for CommercialMode {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct PhysicalMode {
    #[serde(rename = "physical_mode_id")]
    pub id: String,
    #[serde(rename = "physical_mode_name")]
    pub name: String,
    pub co2_emission: Option<f32>,
}
impl Id<PhysicalMode> for PhysicalMode {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Network {
    #[serde(rename = "network_id")]
    pub id: String,
    #[serde(rename = "network_name")]
    pub name: String,
    #[serde(rename = "network_url")]
    pub url: Option<String>,
    #[serde(skip)]
    pub codes: KeysValues,
    #[serde(rename = "network_timezone")]
    pub timezone: Option<String>,
    #[serde(rename = "network_lang")]
    pub lang: Option<String>,
    #[serde(rename = "network_phone")]
    pub phone: Option<String>,
    #[serde(rename = "network_address")]
    pub address: Option<String>,
    #[serde(rename = "network_sort_order")]
    pub sort_order: Option<u32>,
}
impl Id<Network> for Network {
    fn id(&self) -> &str {
        &self.id
    }
}
impl_codes!(Network);

impl GetObjectType for Network {
    fn get_object_type() -> ObjectType {
        ObjectType::Network
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rgb {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl std::fmt::Display for Rgb {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let color = format!("{:02X}{:02X}{:02X}", self.red, self.green, self.blue);
        f.write_str(color.as_ref())
    }
}

#[derive(Debug)]
pub enum RgbError {
    NotHexa,
    TooLongHexa,
    TooShortHexa,
}

use std::error::Error;
use std;

impl std::fmt::Display for RgbError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            RgbError::NotHexa => f.write_str("RgbError_NotHexa"),
            RgbError::TooLongHexa => f.write_str("RgbError_TooLongHexa"),
            RgbError::TooShortHexa => f.write_str("RgbError_NumberOfChar"),
        }
    }
}

impl Error for RgbError {
    fn description(&self) -> &str {
        match *self {
            RgbError::NotHexa => "String is not a valid Hexadecimal value",
            RgbError::TooLongHexa => "String is too long (6 characters expected)",
            RgbError::TooShortHexa => "String is too short (6 characters expected)",
        }
    }
}

impl FromStr for Rgb {
    type Err = RgbError;

    fn from_str(color_hex: &str) -> Result<Self, Self::Err> {
        let color_dec = u32::from_str_radix(color_hex, 16).map_err(|_err| RgbError::NotHexa)?;

        if color_dec >= 1 << 24 {
            return Err(RgbError::TooLongHexa);
        }

        if color_hex.chars().count() != 6 {
            return Err(RgbError::TooShortHexa);
        }

        Ok(Rgb {
            red: (color_dec >> 16) as u8,
            green: (color_dec >> 8) as u8,
            blue: color_dec as u8,
        })
    }
}

impl ::serde::Serialize for Rgb {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        let color = format!("{:02X}{:02X}{:02X}", self.red, self.green, self.blue);
        serializer.serialize_str(&color)
    }
}

impl<'de> ::serde::Deserialize<'de> for Rgb {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let color_hex = String::deserialize(deserializer)?;
        Rgb::from_str(&color_hex).map_err(Error::custom)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Line {
    #[serde(rename = "line_id")]
    pub id: String,
    #[serde(rename = "line_code")]
    pub code: Option<String>,
    #[serde(skip)]
    pub codes: KeysValues,
    #[serde(skip)]
    pub object_properties: KeysValues,
    #[serde(skip)]
    pub comment_links: CommentLinksT,
    #[serde(rename = "line_name")]
    pub name: String,
    #[serde(rename = "forward_line_name")]
    pub forward_name: Option<String>,
    pub forward_direction: Option<String>,
    #[serde(rename = "backward_line_name")]
    pub backward_name: Option<String>,
    pub backward_direction: Option<String>,
    #[serde(rename = "line_color")]
    pub color: Option<Rgb>,
    #[serde(rename = "line_text_color")]
    pub text_color: Option<Rgb>,
    #[serde(rename = "line_sort_order")]
    pub sort_order: Option<u32>,
    pub network_id: String,
    pub commercial_mode_id: String,
    pub geometry_id: Option<String>,
    #[serde(rename = "line_opening_time")]
    pub opening_time: Option<Time>,
    #[serde(rename = "line_closing_time")]
    pub closing_time: Option<Time>,
}

impl Id<Line> for Line {
    fn id(&self) -> &str {
        &self.id
    }
}
impl Id<Network> for Line {
    fn id(&self) -> &str {
        &self.network_id
    }
}
impl Id<CommercialMode> for Line {
    fn id(&self) -> &str {
        &self.commercial_mode_id
    }
}
impl_codes!(Line);
impl_object_properties!(Line);
impl_comment_links!(Line);

impl GetObjectType for Line {
    fn get_object_type() -> ObjectType {
        ObjectType::Line
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Route {
    #[serde(rename = "route_id")]
    pub id: String,
    #[serde(rename = "route_name")]
    pub name: String,
    pub direction_type: Option<String>,
    #[serde(skip)]
    pub codes: KeysValues,
    #[serde(skip)]
    pub object_properties: KeysValues,
    #[serde(skip)]
    pub comment_links: CommentLinksT,
    pub line_id: String,
    pub geometry_id: Option<String>,
    pub destination_id: Option<String>,
}
impl Id<Route> for Route {
    fn id(&self) -> &str {
        &self.id
    }
}
impl Id<Line> for Route {
    fn id(&self) -> &str {
        &self.line_id
    }
}
impl_codes!(Route);
impl_object_properties!(Route);
impl_comment_links!(Route);

impl GetObjectType for Route {
    fn get_object_type() -> ObjectType {
        ObjectType::Route
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct VehicleJourney {
    #[serde(rename = "trip_id")]
    pub id: String,
    #[serde(skip)]
    pub codes: KeysValues,
    #[serde(skip)]
    pub object_properties: KeysValues,
    #[serde(skip)]
    pub comment_links: CommentLinksT,
    pub route_id: String,
    pub physical_mode_id: String,
    pub dataset_id: String,
    pub service_id: String,
    #[serde(rename = "trip_headsign")]
    pub headsign: Option<String>,
    pub block_id: Option<String>,
    pub company_id: String,
    pub trip_property_id: Option<String>,
    pub geometry_id: Option<String>,
    #[serde(skip)]
    pub stop_times: Vec<StopTime>,
}
impl Id<VehicleJourney> for VehicleJourney {
    fn id(&self) -> &str {
        &self.id
    }
}
impl Id<Route> for VehicleJourney {
    fn id(&self) -> &str {
        &self.route_id
    }
}
impl Id<PhysicalMode> for VehicleJourney {
    fn id(&self) -> &str {
        &self.physical_mode_id
    }
}
impl Id<Dataset> for VehicleJourney {
    fn id(&self) -> &str {
        &self.dataset_id
    }
}
impl Id<Company> for VehicleJourney {
    fn id(&self) -> &str {
        &self.company_id
    }
}
impl_codes!(VehicleJourney);
impl_object_properties!(VehicleJourney);
impl_comment_links!(VehicleJourney);

impl GetObjectType for VehicleJourney {
    fn get_object_type() -> ObjectType {
        ObjectType::VehicleJourney
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct Time(u32);
impl Time {
    pub fn new(h: u32, m: u32, s: u32) -> Time {
        Time(h * 60 * 60 + m * 60 + s)
    }
    pub fn hours(&self) -> u32 {
        self.0 / 60 / 60
    }
    pub fn minutes(&self) -> u32 {
        self.0 / 60 % 60
    }
    pub fn seconds(&self) -> u32 {
        self.0 % 60
    }
}
impl ::serde::Serialize for Time {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        let time = format!(
            "{:02}:{:02}:{:02}",
            self.hours(),
            self.minutes(),
            self.seconds()
        );
        serializer.serialize_str(&time)
    }
}
impl<'de> ::serde::Deserialize<'de> for Time {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        use serde::de::{self, Error, Visitor};
        use std::fmt;

        // using the visitor pattern to avoid a string allocation
        struct TimeVisitor;
        impl<'de> Visitor<'de> for TimeVisitor {
            type Value = Time;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a time in the format HH:MM:SS")
            }
            fn visit_str<E: de::Error>(self, time: &str) -> Result<Time, E> {
                let mut t = time.split(':');
                let (hours, minutes, seconds) = match (t.next(), t.next(), t.next(), t.next()) {
                    (Some(h), Some(m), Some(s), None) => (h, m, s),
                    _ => return Err(Error::custom("format should be HH:MM:SS")),
                };
                let hours: u32 = hours.parse().map_err(Error::custom)?;
                let minutes: u32 = minutes.parse().map_err(Error::custom)?;
                let seconds: u32 = seconds.parse().map_err(Error::custom)?;
                // TODO: check 0 <= minutes, seconds < 60?
                Ok(Time::new(hours, minutes, seconds))
            }
        }

        deserializer.deserialize_str(TimeVisitor)
    }
}

#[derive(Debug, PartialEq)]
pub struct StopTime {
    pub stop_point_idx: Idx<StopPoint>,
    pub sequence: u32,
    pub arrival_time: Time,
    pub departure_time: Time,
    pub boarding_duration: u16,
    pub alighting_duration: u16,
    pub pickup_type: u8,
    pub dropoff_type: u8,
    pub datetime_estimated: bool,
    pub local_zone_id: Option<u16>,
}

impl GetObjectType for StopTime {
    fn get_object_type() -> ObjectType {
        ObjectType::StopTime
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Coord {
    pub lon: f64,
    pub lat: f64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct StopArea {
    pub id: String,
    pub name: String,
    #[serde(skip)]
    pub codes: KeysValues,
    #[serde(skip)]
    pub object_properties: KeysValues,
    #[serde(skip)]
    pub comment_links: CommentLinksT,
    pub visible: bool,
    pub coord: Coord,
    pub timezone: Option<String>,
    pub geometry_id: Option<String>,
    pub equipment_id: Option<String>,
}
impl Id<StopArea> for StopArea {
    fn id(&self) -> &str {
        &self.id
    }
}
impl_codes!(StopArea);
impl_object_properties!(StopArea);
impl_comment_links!(StopArea);

impl GetObjectType for StopArea {
    fn get_object_type() -> ObjectType {
        ObjectType::StopArea
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct StopPoint {
    pub id: String,
    pub name: String,
    #[serde(skip)]
    pub codes: KeysValues,
    #[serde(skip)]
    pub object_properties: KeysValues,
    #[serde(skip)]
    pub comment_links: CommentLinksT,
    pub visible: bool,
    pub coord: Coord,
    pub stop_area_id: String,
    pub timezone: Option<String>,
    pub geometry_id: Option<String>,
    pub equipment_id: Option<String>,
    pub fare_zone_id: Option<String>,
}
impl Id<StopPoint> for StopPoint {
    fn id(&self) -> &str {
        &self.id
    }
}
impl Id<StopArea> for StopPoint {
    fn id(&self) -> &str {
        &self.stop_area_id
    }
}
impl_codes!(StopPoint);
impl_object_properties!(StopPoint);
impl_comment_links!(StopPoint);

impl GetObjectType for StopPoint {
    fn get_object_type() -> ObjectType {
        ObjectType::StopPoint
    }
}

pub type Date = chrono::NaiveDate;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ExceptionType {
    #[serde(rename = "1")]
    Add,
    #[serde(rename = "2")]
    Remove,
}

pub type CalendarDates = Vec<(Date, ExceptionType)>;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Calendar {
    #[serde(rename = "service_id")]
    pub id: String,
    #[serde(deserialize_with = "de_from_u8", serialize_with = "ser_from_bool")]
    pub monday: bool,
    #[serde(deserialize_with = "de_from_u8", serialize_with = "ser_from_bool")]
    pub tuesday: bool,
    #[serde(deserialize_with = "de_from_u8", serialize_with = "ser_from_bool")]
    pub wednesday: bool,
    #[serde(deserialize_with = "de_from_u8", serialize_with = "ser_from_bool")]
    pub thursday: bool,
    #[serde(deserialize_with = "de_from_u8", serialize_with = "ser_from_bool")]
    pub friday: bool,
    #[serde(deserialize_with = "de_from_u8", serialize_with = "ser_from_bool")]
    pub saturday: bool,
    #[serde(deserialize_with = "de_from_u8", serialize_with = "ser_from_bool")]
    pub sunday: bool,
    #[serde(deserialize_with = "de_from_date_string", serialize_with = "ser_from_naive_date")]
    pub start_date: Date,
    #[serde(deserialize_with = "de_from_date_string", serialize_with = "ser_from_naive_date")]
    pub end_date: Date,
    #[serde(skip)]
    pub calendar_dates: CalendarDates,
}

impl Id<Calendar> for Calendar {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Company {
    #[serde(rename = "company_id")]
    pub id: String,
    #[serde(rename = "company_name")]
    pub name: String,
    #[serde(rename = "company_address")]
    pub address: Option<String>,
    #[serde(rename = "company_url")]
    pub url: Option<String>,
    #[serde(rename = "company_mail")]
    pub mail: Option<String>,
    #[serde(rename = "company_phone")]
    pub phone: Option<String>,
}

impl Id<Company> for Company {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Derivative)]
#[derivative(Default(bound = ""))]
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum CommentType {
    #[derivative(Default)]
    Information,
    OnDemandTransport,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Comment {
    #[serde(rename = "comment_id")]
    pub id: String,
    #[serde(deserialize_with = "de_with_empty_default")]
    pub comment_type: CommentType,
    #[serde(rename = "comment_label")]
    pub label: Option<String>,
    #[serde(rename = "comment_value")]
    pub value: String,
    #[serde(rename = "comment_url")]
    pub url: Option<String>,
}

impl Id<Comment> for Comment {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Serialize, Deserialize, Debug, Derivative, PartialEq)]
#[derivative(Default)]
pub enum Availability {
    #[derivative(Default)]
    #[serde(rename = "0")]
    InformationNotAvailable,
    #[serde(rename = "1")]
    Available,
    #[serde(rename = "2")]
    NotAvailable,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Equipment {
    #[serde(rename = "equipment_id")]
    pub id: String,
    #[serde(deserialize_with = "de_with_empty_default")]
    pub wheelchair_boarding: Availability,
    #[serde(deserialize_with = "de_with_empty_default")]
    pub sheltered: Availability,
    #[serde(deserialize_with = "de_with_empty_default")]
    pub elevator: Availability,
    #[serde(deserialize_with = "de_with_empty_default")]
    pub escalator: Availability,
    #[serde(deserialize_with = "de_with_empty_default")]
    pub bike_accepted: Availability,
    #[serde(deserialize_with = "de_with_empty_default")]
    pub bike_depot: Availability,
    #[serde(deserialize_with = "de_with_empty_default")]
    pub visual_announcement: Availability,
    #[serde(deserialize_with = "de_with_empty_default")]
    pub audible_announcement: Availability,
    #[serde(deserialize_with = "de_with_empty_default")]
    pub appropriate_escort: Availability,
    #[serde(deserialize_with = "de_with_empty_default")]
    pub appropriate_signage: Availability,
}

impl Id<Equipment> for Equipment {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Transfer {
    pub from_stop_id: String,
    pub to_stop_id: String,
    pub min_transfer_time: Option<u32>,
    pub real_min_transfer_time: Option<u32>,
    pub equipment_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Derivative, PartialEq)]
#[derivative(Default)]
pub enum TransportType {
    #[derivative(Default)]
    #[serde(rename = "0")]
    Regular,
    #[serde(rename = "1")]
    ExclusiveSchool,
    #[serde(rename = "2")]
    RegularAndSchool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct TripProperty {
    #[serde(rename = "trip_property_id")]
    pub id: String,
    #[serde(deserialize_with = "de_with_empty_default")]
    pub wheelchair_accessible: Availability,
    #[serde(deserialize_with = "de_with_empty_default")]
    pub bike_accepted: Availability,
    #[serde(deserialize_with = "de_with_empty_default")]
    pub air_conditioned: Availability,
    #[serde(deserialize_with = "de_with_empty_default")]
    pub visual_announcement: Availability,
    #[serde(deserialize_with = "de_with_empty_default")]
    pub audible_announcement: Availability,
    #[serde(deserialize_with = "de_with_empty_default")]
    pub appropriate_escort: Availability,
    #[serde(deserialize_with = "de_with_empty_default")]
    pub appropriate_signage: Availability,
    #[serde(deserialize_with = "de_with_empty_default")]
    pub school_vehicle_type: TransportType,
}

impl Id<TripProperty> for TripProperty {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Geometry {
    #[serde(rename = "geometry_id")]
    pub id: String,
    #[serde(rename = "geometry_wkt")]
    pub wkt: String,
}

impl Id<Geometry> for Geometry {
    fn id(&self) -> &str {
        &self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate serde_json;

    #[test]
    fn rgb_serialization() {
        let white = Rgb {
            red: 255,
            green: 255,
            blue: 255,
        };
        assert_eq!("FFFFFF", serde_json::to_value(&white).unwrap());

        let black = Rgb {
            red: 0,
            green: 0,
            blue: 0,
        };
        assert_eq!("000000", serde_json::to_value(&black).unwrap());

        let blue = Rgb {
            red: 0,
            green: 125,
            blue: 255,
        };
        assert_eq!("007DFF", serde_json::to_value(&blue).unwrap());
    }

    #[test]
    fn rgb_deserialization_with_too_big_color_hex() {
        let json_value = serde_json::Value::String("1FFFFFF".to_string());
        let rgb: Result<Rgb, _> = serde_json::from_value(json_value);

        assert!(rgb.is_err());
    }

    #[test]
    fn rgb_deserialization_with_bad_number_of_digits() {
        for color in ["F", "FF", "FFF", "FFFF", "FFFFF"].iter() {
            let json_value = serde_json::Value::String(color.to_string());
            let rgb: Result<Rgb, _> = serde_json::from_value(json_value);

            assert!(rgb.is_err());
        }
    }

    #[test]
    fn rgb_good_deserialization() {
        let json_value = serde_json::Value::String("FFFFFF".to_string());
        let rgb: Rgb = serde_json::from_value(json_value).unwrap();

        assert_eq!(255, rgb.red);
        assert_eq!(255, rgb.green);
        assert_eq!(255, rgb.blue);

        let json_value = serde_json::Value::String("000000".to_string());
        let rgb: Rgb = serde_json::from_value(json_value).unwrap();

        assert_eq!(0, rgb.red);
        assert_eq!(0, rgb.green);
        assert_eq!(0, rgb.blue);

        let json_value = serde_json::Value::String("007DFF".to_string());
        let rgb: Rgb = serde_json::from_value(json_value).unwrap();

        assert_eq!(0, rgb.red);
        assert_eq!(125, rgb.green);
        assert_eq!(255, rgb.blue);
    }

    #[test]
    fn time_serialization() {
        let ser = |h, m, s| serde_json::to_value(&Time::new(h, m, s)).unwrap();

        assert_eq!("13:37:00", ser(13, 37, 0));
        assert_eq!("00:00:00", ser(0, 0, 0));
        assert_eq!("25:42:42", ser(25, 42, 42));
    }

    #[test]
    fn time_deserialization() {
        let de = |s: &str| serde_json::from_value(serde_json::Value::String(s.to_string()));

        assert_eq!(Time::new(13, 37, 0), de("13:37:00").unwrap());
        assert_eq!(Time::new(0, 0, 0), de("0:0:0").unwrap());
        assert_eq!(Time::new(25, 42, 42), de("25:42:42").unwrap());

        assert!(de("").is_err());
        assert!(de("13:37").is_err());
        assert!(de("AA:00:00").is_err());
        assert!(de("00:AA:00").is_err());
        assert!(de("00:00:AA").is_err());
    }
}
