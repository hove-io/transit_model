// Copyright (C) 2017 Kisio Digital and/or its affiliates.
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

use crate::{minidom_utils::TryOnlyChild, Result};
use failure::bail;
use lazy_static::lazy_static;
use minidom::Element;
use std::{
    collections::HashMap,
    convert::TryFrom,
    fmt::{self, Display, Formatter},
};

#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IDFMMode {
    Air,
    Bus,
    CableWay,
    Coach,
    Ferry,
    Funicular,
    Lift,
    Metro,
    Other,
    RailInterregional,
    RailRegional,
    RailShuttle,
    RailSuburban,
    RailLocal,
    TrolleyBus,
    Tram,
    Water,
}

impl TryFrom<&Element> for IDFMMode {
    type Error = crate::Error;
    fn try_from(line_element: &Element) -> Result<Self> {
        let transport_mode: String = line_element
            .try_only_child("TransportMode")?
            .text()
            .parse()?;

        use IDFMMode::*;
        match transport_mode.as_str() {
            "air" => Ok(Air),
            "bus" => Ok(Bus),
            "cableway" => Ok(CableWay),
            "coach" => Ok(Coach),
            "ferry" => Ok(Ferry),
            "funicular" => Ok(Funicular),
            "lift" => Ok(Lift),
            "metro" => Ok(Metro),
            "other" => Ok(Other),
            m @ "rail" => {
                let transport_submode: Option<String> = line_element
                    .only_child("TransportSubmode")
                    .and_then(|submode| submode.only_child("RailSubmode"))
                    .map(|s| s.text())
                    .and_then(|s| s.parse().ok());
                match transport_submode.as_deref() {
                    Some("interregionalRail") => Ok(RailInterregional),
                    Some("regionalRail") => Ok(RailRegional),
                    Some("railShuttle") => Ok(RailShuttle),
                    Some("suburbanRailway") => Ok(RailSuburban),
                    Some("local") => Ok(RailLocal),
                    Some(sm) => bail!(
                        "Unknown Transport Submode '{}' for Transport Mode '{}'",
                        sm,
                        m
                    ),
                    None => bail!(
                        "Transport Submode expected but not found for Transport Mode '{}'",
                        m
                    ),
                }
            }
            "trolleyBus" => Ok(TrolleyBus),
            "tram" => Ok(Tram),
            "water" => Ok(Water),
            m => bail!("Unknown Transport Mode '{}'", m),
        }
    }
}

impl Display for IDFMMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::result::Result<(), fmt::Error> {
        use IDFMMode::*;
        match self {
            Air => write!(f, "air"),
            Bus => write!(f, "bus"),
            CableWay => write!(f, "cableway"),
            Coach => write!(f, "coach"),
            Ferry => write!(f, "ferry"),
            Funicular => write!(f, "funicular"),
            Lift => write!(f, "lift"),
            Metro => write!(f, "metro"),
            Other => write!(f, "other"),
            RailInterregional => write!(f, "rail:interregionalRail"),
            RailRegional => write!(f, "rail:regionalRail"),
            RailShuttle => write!(f, "rail:railShuttle"),
            RailSuburban => write!(f, "rail:suburbanRailway"),
            RailLocal => write!(f, "rail:local"),
            TrolleyBus => write!(f, "trolleyBus"),
            Tram => write!(f, "tram"),
            Water => write!(f, "water"),
        }
    }
}

#[derive(Debug)]
pub struct NTFSMode {
    // Tuple (mode_id, mode_name)
    pub physical_mode: (&'static str, &'static str),
    pub commercial_mode: (&'static str, &'static str),
}

impl NTFSMode {
    pub fn new(
        physical_mode: (&'static str, &'static str),
        commercial_mode: (&'static str, &'static str),
    ) -> Self {
        NTFSMode {
            physical_mode,
            commercial_mode,
        }
    }
}

lazy_static! {
    pub static ref MODES: HashMap<IDFMMode, NTFSMode> = {
        let mut m = HashMap::new();
        m.insert(
            IDFMMode::Air,
            NTFSMode::new(("Air", "Avion"), ("Air", "Avion")),
        );
        m.insert(IDFMMode::Bus, NTFSMode::new(("Bus", "Bus"), ("Bus", "Bus")));
        m.insert(
            IDFMMode::Coach,
            NTFSMode::new(("Coach", "Autocar"), ("Coach", "Autocar")),
        );
        m.insert(
            IDFMMode::Ferry,
            NTFSMode::new(("Ferry", "Ferry"), ("Ferry", "Ferry")),
        );
        m.insert(
            IDFMMode::Metro,
            NTFSMode::new(("Metro", "Métro"), ("Metro", "Métro")),
        );
        m.insert(
            IDFMMode::RailShuttle,
            NTFSMode::new(
                ("RailShuttle", "Orlyval, CDG VAL"),
                ("RailShuttle", "Orlyval, CDG VAL"),
            ),
        );
        m.insert(
            IDFMMode::RailSuburban,
            NTFSMode::new(
                ("LocalTrain", "Train Transilien"),
                ("LocalTrain", "Train Transilien"),
            ),
        );
        m.insert(
            IDFMMode::RailRegional,
            NTFSMode::new(("Train", "TER / Intercités"), ("regionalRail", "TER")),
        );
        m.insert(
            IDFMMode::RailInterregional,
            NTFSMode::new(
                ("Train", "TER / Intercités"),
                ("interregionalRail", "Intercités"),
            ),
        );
        m.insert(
            IDFMMode::RailLocal,
            NTFSMode::new(("RapidTransit", "RER"), ("RapidTransit", "RER")),
        );
        m.insert(
            IDFMMode::TrolleyBus,
            NTFSMode::new(("Tramway", "Tramway"), ("TrolleyBus", "TrolleyBus")),
        );
        m.insert(
            IDFMMode::Tram,
            NTFSMode::new(("Tramway", "Tramway"), ("Tramway", "Tramway")),
        );
        m.insert(
            IDFMMode::Water,
            NTFSMode::new(
                ("Boat", "Navette maritime / fluviale"),
                ("Boat", "Navette maritime / fluviale"),
            ),
        );
        m.insert(
            IDFMMode::CableWay,
            NTFSMode::new(("Tramway", "Tramway"), ("CableWay", "CableWay")),
        );
        m.insert(
            IDFMMode::Funicular,
            NTFSMode::new(("Funicular", "Funiculaire"), ("Funicular", "Funiculaire")),
        );
        m.insert(
            IDFMMode::Lift,
            NTFSMode::new(("Bus", "Bus"), ("Bus", "Bus")),
        );
        m.insert(
            IDFMMode::Other,
            NTFSMode::new(("Bus", "Bus"), ("Bus", "Bus")),
        );
        m
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn known_transport_mode() {
        let xml = r#"
            <Line>
                <TransportMode>bus</TransportMode>
            </Line>"#;
        let line_element: Element = xml.parse().unwrap();
        let mode = IDFMMode::try_from(&line_element).unwrap();
        assert_eq!(IDFMMode::Bus, mode);
    }

    #[test]
    fn known_transport_submode() {
        let xml = r#"
            <Line>
                <TransportMode>rail</TransportMode>
                <TransportSubmode>
                    <RailSubmode>local</RailSubmode>
                </TransportSubmode>
            </Line>"#;
        let line_element: Element = xml.parse().unwrap();
        let mode = IDFMMode::try_from(&line_element).unwrap();
        assert_eq!(IDFMMode::RailLocal, mode);
    }

    #[test]
    #[should_panic(expected = "Unknown Transport Mode \\'UNKNOWN\\'")]
    fn unknown_transport_mode() {
        let xml = r#"
            <Line>
                <TransportMode>UNKNOWN</TransportMode>
            </Line>"#;
        let line_element: Element = xml.parse().unwrap();
        IDFMMode::try_from(&line_element).unwrap();
    }

    #[test]
    #[should_panic(
        expected = "Unknown Transport Submode \\'UNKNOWN\\' for Transport Mode \\'rail\\'"
    )]
    fn unknown_transport_submode() {
        let xml = r#"
            <Line>
                <TransportMode>rail</TransportMode>
                <TransportSubmode>
                    <RailSubmode>UNKNOWN</RailSubmode>
                </TransportSubmode>
            </Line>"#;
        let line_element: Element = xml.parse().unwrap();
        IDFMMode::try_from(&line_element).unwrap();
    }

    #[test]
    #[should_panic(
        expected = "Transport Submode expected but not found for Transport Mode \\'rail\\'"
    )]
    fn rail_transport_without_submode() {
        let xml = r#"
            <Line>
                <TransportMode>rail</TransportMode>
            </Line>"#;
        let line_element: Element = xml.parse().unwrap();
        IDFMMode::try_from(&line_element).unwrap();
    }
}
