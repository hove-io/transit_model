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

//! Exporter for Netex France profile
use crate::netex_france::{merge_adjacent_duplicate_segments, slug};
use crate::xml_builder::{Element, ElementWriter, Node};
use crate::{
    model::Model,
    netex_france::{
        CalendarExporter, CompanyExporter, LineExporter, NetworkExporter, StopExporter,
        TransferExporter,
    },
    objects::{Date, KeysValues, Line, Network},
    Result,
};
use anyhow::anyhow;
use chrono::prelude::*;
use rayon::prelude::*;
use std::{
    convert::AsRef,
    fmt::{self, Display, Formatter},
    fs::File,
    io::BufWriter,
    path::Path,
};
use tracing::info;
use typed_index_collection::{CollectionWithId, Idx};

const NETEX_FRANCE_NETWORK_FILENAME: &str = "network.xml";
const NETEX_FRANCE_RESOURCE_FILENAME: &str = "resource.xml";
const NETEX_FRANCE_STOPS_FILENAME: &str = "stop.xml";

/// Type of NeTEx frame.
#[derive(Debug, Eq, Hash, PartialEq, Clone, Copy)]
pub(in crate::netex_france) enum FrameType {
    /// Type of a `<CompositeFrame>`
    Composite,
    /// Type of a `<GeneralFrame>`
    General,
}

impl Display for FrameType {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        use FrameType::*;
        match self {
            Composite => write!(f, "CompositeFrame"),
            General => write!(f, "GeneralFrame"),
        }
    }
}

pub(in crate::netex_france) enum ObjectType {
    AccessibilityAssessment,
    DayType,
    DayTypeAssignment,
    FareZone,
    Line,
    Network,
    Operator,
    PassengerStopAssignment,
    PointOnRoute,
    Quay,
    Route,
    RoutePoint,
    ScheduledStopPoint,
    ServiceJourney,
    ServiceJourneyPattern,
    SiteConnection,
    StopPlace,
    StopPlaceEntrance,
    StopPointInJourneyPattern,
    TimetabledPassingTime,
    UicOperatingPeriod,
}

impl Display for ObjectType {
    fn fmt(&self, f: &mut Formatter) -> std::result::Result<(), fmt::Error> {
        use ObjectType::*;
        match self {
            AccessibilityAssessment => write!(f, "AccessibilityAssessment"),
            DayType => write!(f, "DayType"),
            DayTypeAssignment => write!(f, "DayTypeAssignment"),
            FareZone => write!(f, "FareZone"),
            Line => write!(f, "Line"),
            Network => write!(f, "Network"),
            Operator => write!(f, "Operator"),
            PassengerStopAssignment => write!(f, "PassengerStopAssignment"),
            PointOnRoute => write!(f, "PointOnRoute"),
            Quay => write!(f, "Quay"),
            Route => write!(f, "Route"),
            RoutePoint => write!(f, "RoutePoint"),
            ScheduledStopPoint => write!(f, "ScheduledStopPoint"),
            ServiceJourney => write!(f, "ServiceJourney"),
            ServiceJourneyPattern => write!(f, "ServiceJourneyPattern"),
            SiteConnection => write!(f, "SiteConnection"),
            StopPlace => write!(f, "StopPlace"),
            StopPlaceEntrance => write!(f, "StopPlaceEntrance"),
            StopPointInJourneyPattern => write!(f, "StopPointInJourneyPattern"),
            TimetabledPassingTime => write!(f, "TimetabledPassingTime"),
            UicOperatingPeriod => write!(f, "UicOperatingPeriod"),
        }
    }
}

#[derive(Clone, Copy)]
enum VersionType {
    Calendars,
    Common,
    France,
    Lines,
    LinesStructure,
    Networks,
    Schedule,
    Stops,
}

impl Display for VersionType {
    fn fmt(&self, fmt: &mut Formatter) -> std::result::Result<(), fmt::Error> {
        use VersionType::*;
        match self {
            Calendars => write!(fmt, "CALENDRIER"),
            Common => write!(fmt, "COMMUN"),
            France => write!(fmt, "FRANCE"),
            Lines => write!(fmt, "LIGNE"),
            LinesStructure => write!(fmt, "LIGNE_STRUCTURE"),
            Networks => write!(fmt, "RESEAU"),
            Schedule => write!(fmt, "HORAIRE"),
            Stops => write!(fmt, "ARRET"),
        }
    }
}

/// Build the `line_*.xml` filename for a line, per the NeTEx-fr naming rules.
fn line_filename(line: &Line, networks: &CollectionWithId<Network>) -> Result<String> {
    let network = networks.get(&line.network_id).ok_or_else(|| {
        anyhow!(
            "Line '{}' references unknown network '{}'",
            line.id,
            line.network_id
        )
    })?;
    // A non-Latin network name collapses to an empty string in slug().
    // network_id could fail the same way (rare), hence the final "x" fallback.
    let net = [&network.name, &network.id]
        .iter()
        .map(|s| slug(s, 40))
        .find(|s| !s.is_empty())
        .unwrap_or_else(|| "x".to_string());
    let code = line
        .code
        .as_deref()
        .map(|c| slug(c, 20))
        .unwrap_or_default();
    let name = slug(&line.name, 60);

    let mut segments = vec!["line".to_string(), net];
    if !code.is_empty() {
        segments.push(code.clone());
    }
    if !name.is_empty() && name != code {
        segments.push(name);
    }
    let base = merge_adjacent_duplicate_segments(&segments.join("_"));

    Ok(format!("{base}_{:x}.xml", md5::compute(line.id.as_bytes())))
}

/// Struct that can write an export of Netex France profile from a Model
pub struct Exporter<'a> {
    model: &'a Model,
    participant_ref: String,
    _stop_provider_code: String,
    timestamp: DateTime<FixedOffset>,
}

// Publicly exposed methods
impl<'a> Exporter<'a> {
    /// Build a Netex France profile exporter from the model.
    /// `path` is the expected output Path where the Netex France is going to be
    /// written. It should be a folder that already exists.
    pub fn new(
        model: &'a Model,
        participant_ref: String,
        stop_provider_code: Option<String>,
        timestamp: DateTime<FixedOffset>,
    ) -> Self {
        let _stop_provider_code = stop_provider_code.unwrap_or_else(|| String::from("LOC"));
        Exporter {
            model,
            participant_ref,
            _stop_provider_code,
            timestamp,
        }
    }

    /// Actually write `model` into `path` as a Netex France profile.
    pub fn write<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        std::fs::create_dir_all(&path)?;
        self.write_networks(&path)?;
        self.write_stops(&path)?;
        self.write_resource(&path)?;
        self.write_lines(&path)?;
        Ok(())
    }

    pub(in crate::netex_france) fn generate_id(id: &'a str, object_type: ObjectType) -> String {
        let id = id.replace(':', "_");
        format!("FR:{object_type}:{id}:")
    }

    pub(in crate::netex_france) fn generate_key_list(codes: &KeysValues) -> Option<Element> {
        let (_, source_id) = codes.iter().find(|(key, _)| key.as_str() == "source")?;
        let key = Element::builder("Key").append("source").build();
        let value = Element::builder("Value").append(source_id.as_str()).build();
        let key_value = Element::builder("KeyValue")
            .attr("typeOfKey", "ALTERNATE_IDENTIFIER")
            .append(key)
            .append(value)
            .build();
        Some(Element::builder("keyList").append(key_value).build())
    }
}

// Internal methods
impl Exporter<'_> {
    // Include 'frame' into a complete NeTEx XML tree with 'PublicationDelivery'
    // and 'dataObjects'. PublicationDelivery/@version is always FRANCE.
    fn wrap_frame(&self, frame: Element) -> Element {
        let publication_timestamp = Element::builder("PublicationTimestamp")
            .append(self.timestamp.to_rfc3339())
            .build();
        let participant_ref = Element::builder("ParticipantRef")
            .append(self.participant_ref.as_str())
            .build();
        let data_objects = Element::builder("dataObjects").append(frame).build();
        Element::builder("PublicationDelivery")
            .attr("version", self.generate_frame_version(VersionType::France))
            .attr("xmlns", "http://www.netex.org.uk/netex")
            .append(publication_timestamp)
            .append(participant_ref)
            .append(data_objects)
            .build()
    }

    fn generate_frame_id(
        &self,
        frame_type: FrameType,
        part: VersionType,
        instance_suffix: Option<&str>,
    ) -> String {
        match instance_suffix {
            Some(suffix) => format!("FR:{frame_type}:NETEX_{part}-{suffix}:"),
            None => format!("FR:{frame_type}:NETEX_{part}:"),
        }
    }

    fn generate_frame_version(&self, part: VersionType) -> String {
        format!("1.3:FR-NETEX_{part}-2.4")
    }

    fn generate_type_of_frame_ref(&self, part: VersionType) -> Element {
        Element::builder("TypeOfFrameRef")
            .attr("ref", format!("FR:TypeOfFrame:NETEX_{part}:"))
            .build()
    }

    fn create_frame<I, T>(
        &self,
        frame_type: FrameType,
        part: VersionType,
        instance_suffix: Option<&str>,
        valid_between: Option<Element>,
        children: I,
    ) -> Element
    where
        I: IntoIterator<Item = T>,
        T: Into<Node>,
    {
        let id = self.generate_frame_id(frame_type, part, instance_suffix);
        let type_of_frame_ref = self.generate_type_of_frame_ref(part);
        let mut builder = Element::builder(frame_type.to_string())
            .attr("id", id)
            .attr("version", self.generate_frame_version(part));
        if let Some(valid_between) = valid_between {
            builder = builder.append(valid_between);
        }
        builder = builder.append(type_of_frame_ref);
        match frame_type {
            FrameType::Composite => {
                let frame_list = Element::builder("frames").append_all(children).build();
                builder.append(frame_list).build()
            }
            FrameType::General => builder.append_all(children).build(),
        }
    }

    pub(in crate::netex_france) fn create_members<I, T>(members: I) -> Element
    where
        I: IntoIterator<Item = T>,
        T: Into<Node>,
    {
        Element::builder("members").append_all(members).build()
    }

    fn write_networks<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let filepath = path.as_ref().join(NETEX_FRANCE_NETWORK_FILENAME);
        let file = BufWriter::new(File::create(&filepath)?);
        let networks_frame = self.create_networks_frame();
        let netex = self.wrap_frame(networks_frame);
        let mut writer = ElementWriter::pretty(file);
        info!("Writing {:?}", &filepath);
        writer.write(&netex)?;
        Ok(())
    }

    // Returns a 'GeneralFrame' containing all 'Network'
    fn create_networks_frame(&self) -> Element {
        let network_exporter = NetworkExporter::new(self.model);
        let network_elements = network_exporter.export();
        let members = Self::create_members(network_elements);
        self.create_frame(
            FrameType::General,
            VersionType::Networks,
            None,
            None,
            [members],
        )
    }

    fn write_resource<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let filepath = path.as_ref().join(NETEX_FRANCE_RESOURCE_FILENAME);
        let file = BufWriter::new(File::create(&filepath)?);
        let common_frame = self.create_common_frame()?;
        let calendars_frame = self.create_calendars_frame()?;
        let composite_frame = self.create_frame(
            FrameType::Composite,
            VersionType::France,
            None,
            None,
            [common_frame, calendars_frame],
        );
        let netex = self.wrap_frame(composite_frame);
        let mut writer = ElementWriter::pretty(file);
        info!("Writing {:?}", &filepath);
        writer.write(&netex)?;
        Ok(())
    }

    // Returns a 'GeneralFrame' containing all 'Operator' and 'SiteConnection'
    fn create_common_frame(&self) -> Result<Element> {
        let company_exporter = CompanyExporter::new(self.model);
        let companies = company_exporter.export();
        let transfer_exporter = TransferExporter::new(self.model);
        let transfers = transfer_exporter.export()?;
        let members = Self::create_members(companies.into_iter().chain(transfers));
        Ok(self.create_frame(
            FrameType::General,
            VersionType::Common,
            None,
            None,
            [members],
        ))
    }

    // Returns a 'GeneralFrame' containing all 'DayType', 'DayTypeAssignment' and 'UicOperatingPeriod'
    fn create_calendars_frame(&self) -> Result<Element> {
        let calendar_exporter = CalendarExporter::new(self.model);
        let calendars = calendar_exporter.export()?;
        let valid_between = self.create_valid_between()?;
        let members = Self::create_members(calendars);
        Ok(self.create_frame(
            FrameType::General,
            VersionType::Calendars,
            None,
            Some(valid_between),
            [members],
        ))
    }

    fn create_valid_between(&self) -> Result<Element> {
        let format_date = |date: Date, hour, minute, second| -> String {
            date.and_hms_opt(hour, minute, second)
                .unwrap()
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string()
        };
        let (start_date, end_date) = self.model.calculate_validity_period()?;
        let from_date = Element::builder("FromDate")
            .append(Node::Text(format_date(start_date, 0, 0, 0)))
            .build();
        let to_date = Element::builder("ToDate")
            .append(Node::Text(format_date(end_date, 23, 59, 59)))
            .build();
        let valid_between = Element::builder("ValidBetween")
            .append(from_date)
            .append(to_date)
            .build();
        Ok(valid_between)
    }

    fn write_stops<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let filepath = path.as_ref().join(NETEX_FRANCE_STOPS_FILENAME);
        let file = BufWriter::new(File::create(&filepath)?);
        let stop_frame = self.create_stops_frame()?;
        let netex = self.wrap_frame(stop_frame);
        let mut writer = ElementWriter::pretty(file);
        info!("Writing {:?}", &filepath);
        writer.write(&netex)?;
        Ok(())
    }

    // Returns a 'GeneralFrame' containing all 'StopArea' and 'Quay'
    fn create_stops_frame(&self) -> Result<Element> {
        let stop_exporter = StopExporter::new(self.model)?;
        let stops = stop_exporter.export()?;
        let members = Self::create_members(stops);
        Ok(self.create_frame(
            FrameType::General,
            VersionType::Stops,
            None,
            None,
            [members],
        ))
    }

    fn write_lines<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let line_exporter = LineExporter::new(self.model)?;
        let line_indexes: Vec<Idx<Line>> = self.model.lines.indexes().collect();
        line_indexes
            .par_iter()
            .try_for_each(|line_idx| -> Result<()> {
                let line = &self.model.lines[*line_idx];
                let filepath = path.join(line_filename(line, &self.model.networks)?);
                let composite_frame =
                    self.create_line_composite_frame(&line_exporter, *line_idx)?;
                let netex = self.wrap_frame(composite_frame);
                let file = BufWriter::new(File::create(&filepath)?);
                let mut writer = ElementWriter::pretty(file);
                info!("Writing {:?}", &filepath);
                writer.write(&netex)?;
                Ok(())
            })?;
        Ok(())
    }

    // Returns a 'CompositeFrame' NETEX_LIGNE for a single line, wrapping its
    // NETEX_LIGNE_STRUCTURE and NETEX_HORAIRE frames.
    fn create_line_composite_frame(
        &self,
        line_exporter: &LineExporter,
        line_idx: Idx<Line>,
    ) -> Result<Element> {
        let line = &self.model.lines[line_idx];
        let instance_suffix = format!("{:x}", md5::compute(line.id.as_bytes()));
        let offer = line_exporter.export(line_idx)?;
        let structure_frame = self.create_frame(
            FrameType::General,
            VersionType::LinesStructure,
            Some(&instance_suffix),
            None,
            [Self::create_members(offer.structure)],
        );
        let schedule_frame = self.create_frame(
            FrameType::General,
            VersionType::Schedule,
            Some(&instance_suffix),
            None,
            [Self::create_members(offer.schedule)],
        );
        Ok(self.create_frame(
            FrameType::Composite,
            VersionType::Lines,
            Some(&instance_suffix),
            None,
            [structure_frame, schedule_frame],
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod line_filename {
        use super::*;

        #[test]
        fn code_and_name_present_and_different() {
            // SYTRAL (TCL network), line SYTNEX:102.
            let networks = CollectionWithId::from(Network {
                id: "TCL".to_string(),
                name: "TCL".to_string(),
                ..Default::default()
            });
            let line = Line {
                id: "SYTNEX:102".to_string(),
                network_id: "TCL".to_string(),
                code: Some("102".to_string()),
                name: "Croix-Rousse - Plateau de Saint Rambert".to_string(),
                ..Default::default()
            };
            assert_eq!(
                line_filename(&line, &networks).unwrap(),
                "line_tcl_102_croix_rousse_plateau_de_saint_rambert_f2a4eb5f0123026eb1e7ccde5ef6eba3.xml"
            );
        }

        #[test]
        fn name_identical_to_code_is_dropped() {
            // IDFM, line IDFM:C01624: line_code and line_name are both "4244".
            let networks = CollectionWithId::from(Network {
                id: "IDFM:1080".to_string(),
                name: "Evry Centre Essonne".to_string(),
                ..Default::default()
            });
            let line = Line {
                id: "IDFM:C01624".to_string(),
                network_id: "IDFM:1080".to_string(),
                code: Some("4244".to_string()),
                name: "4244".to_string(),
                ..Default::default()
            };
            assert_eq!(
                line_filename(&line, &networks).unwrap(),
                "line_evry_centre_essonne_4244_e0cb7609cdbd9bf17f4d989dd34bbb94.xml"
            );
        }

        #[test]
        fn accented_network_and_line_names() {
            // CH, network "Sihltal-Zürich-Uetliberg-Bahn", line OCH:91-10-A-j26-1.
            let networks = CollectionWithId::from(Network {
                id: "OCH:78".to_string(),
                name: "Sihltal-Zürich-Uetliberg-Bahn".to_string(),
                ..Default::default()
            });
            let line = Line {
                id: "OCH:91-10-A-j26-1".to_string(),
                network_id: "OCH:78".to_string(),
                code: Some("S10".to_string()),
                name: "Uetliberg - Zürich HB".to_string(),
                ..Default::default()
            };
            assert_eq!(
                line_filename(&line, &networks).unwrap(),
                "line_sihltal_zurich_uetliberg_bahn_s10_uetliberg_zurich_hb_ce505b256619e5d786e4ce00b75dd2e2.xml"
            );
        }

        #[test]
        fn non_latin_network_name_falls_back_to_network_id() {
            // Israel dataset, network ISR:135 (Hebrew name), line ISR:5878.
            // This is the real-world case cited in the working doc's own example.
            let networks = CollectionWithId::from(Network {
                id: "ISR:135".to_string(),
                name: "דרך אגד עוטף ירושלים".to_string(),
                ..Default::default()
            });
            let line = Line {
                id: "ISR:5878".to_string(),
                network_id: "ISR:135".to_string(),
                code: Some("104".to_string()),
                name: "ממילא/קריב-ירושלים<->שכונה י''א א-מבשרת ציון-1#".to_string(),
                ..Default::default()
            };
            assert_eq!(
                line_filename(&line, &networks).unwrap(),
                "line_isr_135_104_1_6e4e7238e072c1ad1280fda127906606.xml"
            );
        }

        #[test]
        fn no_code_is_omitted() {
            let networks = CollectionWithId::from(Network {
                id: "RER".to_string(),
                name: "RER".to_string(),
                ..Default::default()
            });
            let line = Line {
                id: "RATP:Line:A".to_string(),
                network_id: "RER".to_string(),
                code: None,
                name: "RER A".to_string(),
                ..Default::default()
            };
            assert_eq!(
                line_filename(&line, &networks).unwrap(),
                "line_rer_a_cb68c30f80b859d8d82ff5cd44e8305e.xml"
            );
        }

        #[test]
        fn name_absent_with_code_present() {
            let networks = CollectionWithId::from(Network {
                id: "RATP".to_string(),
                name: "RATP".to_string(),
                ..Default::default()
            });
            let line = Line {
                id: "RATP:Line:14".to_string(),
                network_id: "RATP".to_string(),
                code: Some("14".to_string()),
                name: "".to_string(),
                ..Default::default()
            };
            assert_eq!(
                line_filename(&line, &networks).unwrap(),
                "line_ratp_14_9ad325ee67c40600689d1badac32e1d0.xml"
            );
        }

        #[test]
        fn neither_code_nor_name_exploitable() {
            let networks = CollectionWithId::from(Network {
                id: "NET".to_string(),
                name: "NET".to_string(),
                ..Default::default()
            });
            let line = Line {
                id: "X:4".to_string(),
                network_id: "NET".to_string(),
                code: None,
                name: "אגד".to_string(),
                ..Default::default()
            };
            assert_eq!(
                line_filename(&line, &networks).unwrap(),
                "line_net_f4031a78bc723373381168309ce57a54.xml"
            );
        }

        #[test]
        fn network_name_and_id_both_empty_falls_back_to_x() {
            let networks = CollectionWithId::from(Network {
                id: "".to_string(),
                name: "".to_string(),
                ..Default::default()
            });
            let line = Line {
                id: "X:1".to_string(),
                network_id: "".to_string(),
                code: Some("14".to_string()),
                name: "Ligne 14".to_string(),
                ..Default::default()
            };
            assert_eq!(
                line_filename(&line, &networks).unwrap(),
                "line_x_14_ligne_14_eb1a6accc61ea1d3c179abd0a367ed49.xml"
            );
        }

        #[test]
        fn network_and_code_segments_are_merged_when_identical() {
            // network slug and line code slug are both "bus" —
            // merge_adjacent_duplicate_segments collapses them into one.
            let networks = CollectionWithId::from(Network {
                id: "Bus".to_string(),
                name: "Bus".to_string(),
                ..Default::default()
            });
            let line = Line {
                id: "X:3".to_string(),
                network_id: "Bus".to_string(),
                code: Some("Bus".to_string()),
                name: "".to_string(),
                ..Default::default()
            };
            assert_eq!(
                line_filename(&line, &networks).unwrap(),
                "line_bus_33409777a8bb7c6347b2371c9bfdf918.xml"
            );
        }

        #[test]
        fn filename_never_exceeds_the_netex_fr_250_char_limit() {
            // 250 characters is the NeTEx-fr profile's own limit on filenames,
            // not an arbitrary choice here.
            let very_long_text = "a".repeat(300);
            let networks = CollectionWithId::from(Network {
                id: very_long_text.clone(),
                name: very_long_text.clone(),
                ..Default::default()
            });
            let line = Line {
                id: "X".to_string(),
                network_id: very_long_text.clone(),
                code: Some(very_long_text.clone()),
                name: very_long_text,
                ..Default::default()
            };
            assert!(
                line_filename(&line, &networks).unwrap().len() <= 250,
                "line_*.xml filenames must stay under the NeTEx-fr 250-character limit"
            );
        }

        #[test]
        fn unknown_network_is_an_error() {
            let networks = CollectionWithId::from(Network {
                id: "RATP".to_string(),
                name: "RATP".to_string(),
                ..Default::default()
            });
            let line = Line {
                id: "X:5".to_string(),
                network_id: "does_not_exist".to_string(),
                ..Default::default()
            };
            assert!(line_filename(&line, &networks).is_err());
        }
    }
}
