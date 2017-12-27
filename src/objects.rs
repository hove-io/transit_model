use collection::{Id, Idx};

// We use a Vec here for memory efficiency.  Other possible types can
// be something like BTreeSet<(String,String)> or
// BTreeMap<String,Vec<String>>.  Hash{Map,Set} are memory costy.
pub type CodesT = Vec<(String, String)>;

pub trait Codes {
    fn codes(&self) -> &CodesT;
    fn codes_mut(&mut self) -> &mut CodesT;
}
macro_rules! impl_codes {
    ($ty:ty) => {
        impl Codes for $ty {
            fn codes(&self) -> &CodesT {
                &self.codes
            }
            fn codes_mut(&mut self) -> &mut CodesT {
                &mut self.codes
            }
        }
    };
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Contributor {
    #[serde(rename = "contributor_id")] pub id: String,
    #[serde(rename = "contributor_name")] pub name: String,
    #[serde(rename = "contributor_license")] pub license: Option<String>,
    #[serde(rename = "contributor_website")] pub website: Option<String>,
}
impl Id<Contributor> for Contributor {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Dataset {
    #[serde(rename = "dataset_id")] pub id: String,
    pub contributor_id: String,
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

#[derive(Serialize, Deserialize, Debug)]
pub struct CommercialMode {
    #[serde(rename = "commercial_mode_id")] pub id: String,
    #[serde(rename = "commercial_mode_name")] pub name: String,
}
impl Id<CommercialMode> for CommercialMode {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PhysicalMode {
    #[serde(rename = "physical_mode_id")] pub id: String,
    #[serde(rename = "physical_mode_name")] pub name: String,
}
impl Id<PhysicalMode> for PhysicalMode {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Network {
    #[serde(rename = "network_id")] pub id: String,
    #[serde(rename = "network_name")] pub name: String,
    #[serde(skip)] pub codes: CodesT,
    #[serde(rename = "network_timezone")] pub timezone: String,
}
impl Id<Network> for Network {
    fn id(&self) -> &str {
        &self.id
    }
}
impl_codes!(Network);

#[derive(Clone, Debug)]
pub struct Rgb {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
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
        let color_dec = u32::from_str_radix(&color_hex, 16).map_err(Error::custom)?;

        if color_dec >= 1 << 24 {
            return Err(Error::custom(
                "color should be in hexadecimal format (ex: FF4500)",
            ));
        }

        if color_hex.chars().count() != 6 {
            return Err(Error::custom(
                "color should have 6 hexadecimal digits (ex: FF4500)",
            ));
        }

        Ok(Rgb {
            red: (color_dec >> 16) as u8,
            green: (color_dec >> 8) as u8,
            blue: color_dec as u8,
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Line {
    #[serde(rename = "line_id")] pub id: String,
    #[serde(rename = "line_code")] pub code: Option<String>,
    #[serde(skip)] pub codes: CodesT,
    #[serde(rename = "line_name")] pub name: String,
    #[serde(rename = "forward_line_name")] pub forward_name: Option<String>,
    pub forward_direction: Option<String>,
    #[serde(rename = "backward_line_name")] pub backward_name: Option<String>,
    #[serde(rename = "line_color")] pub color: Option<Rgb>,
    #[serde(rename = "line_text_color")] pub text_color: Option<Rgb>,
    #[serde(rename = "line_sort_order")] pub sort_order: Option<u32>,
    pub network_id: String,
    pub commercial_mode_id: String,
    pub geometry_id: Option<String>,
    #[serde(rename = "line_opening_time")] pub opening_time: Option<String>,
    #[serde(rename = "line_closing_time")] pub closing_time: Option<String>,
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

#[derive(Serialize, Deserialize, Debug)]
pub struct Route {
    #[serde(rename = "route_id")] pub id: String,
    #[serde(rename = "route_name")] pub name: String,
    #[serde(skip)] pub codes: CodesT,
    pub line_id: String,
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

#[derive(Serialize, Deserialize, Debug)]
pub struct VehicleJourney {
    #[serde(rename = "trip_id")] pub id: String,
    #[serde(skip)] pub codes: CodesT,
    pub route_id: String,
    pub physical_mode_id: String,
    pub dataset_id: String,
    #[serde(skip)] pub stop_times: Vec<StopTime>,
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
impl_codes!(VehicleJourney);

#[derive(Debug)]
pub struct StopTime {
    pub stop_point_idx: Idx<StopPoint>,
    pub sequence: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Coord {
    pub lon: f64,
    pub lat: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StopArea {
    pub id: String,
    pub name: String,
    #[serde(skip)] pub codes: CodesT,
    pub visible: bool,
    pub coord: Coord,
    pub timezone: Option<String>,
}
impl Id<StopArea> for StopArea {
    fn id(&self) -> &str {
        &self.id
    }
}
impl_codes!(StopArea);

#[derive(Serialize, Deserialize, Debug)]
pub struct StopPoint {
    pub id: String,
    pub name: String,
    #[serde(skip)] pub codes: CodesT,
    pub visible: bool,
    pub coord: Coord,
    pub stop_area_id: String,
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
}
