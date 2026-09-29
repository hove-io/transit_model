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
        CalendarExporter, CompanyExporter, LineExporter, NetworkExporter, OfferExporter,
        StopExporter, TransferExporter,
    },
    objects::{Date, KeysValues, Line, Network},
    Result,
};
use chrono::prelude::*;
use rayon::prelude::*;
use relational_types::IdxSet;
use std::{
    convert::AsRef,
    fmt::{self, Display, Formatter},
    fs::{self, File},
    io::BufWriter,
    iter,
    path::{Path, PathBuf},
};
use tracing::info;
use typed_index_collection::Idx;

const NETEX_FRANCE_LINES_FILENAME: &str = "lignes.xml";
const NETEX_FRANCE_NETWORK_FILENAME: &str = "network.xml";
const NETEX_FRANCE_RESOURCE_FILENAME: &str = "resource.xml";
const NETEX_FRANCE_STOPS_FILENAME: &str = "stop.xml";

/// Type of NeTEx frame.
#[derive(Debug, Eq, Hash, PartialEq)]
pub(in crate::netex_france) enum FrameType {
    /// Type of a `<CompositeFrame>`
    Composite,
    /// Type of a `<GeneralFrame>`
    General,
    /// Type of a `<ServiceFrame>`
    Service,
}

impl Display for FrameType {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        use FrameType::*;
        match self {
            Composite => write!(f, "CompositeFrame"),
            General => write!(f, "GeneralFrame"),
            Service => write!(f, "ServiceFrame"),
        }
    }
}

pub(in crate::netex_france) enum ObjectType {
    AccessibilityAssessment,
    DayType,
    DayTypeAssignment,
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

enum VersionType {
    Calendars,
    Common,
    France,
    Lines,
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
            Networks => write!(fmt, "RESEAU"),
            Schedule => write!(fmt, "HORAIRE"),
            Stops => write!(fmt, "ARRET"),
        }
    }
}

fn only_alphanumeric(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).collect()
}

/// Build the `line_*.xml` filename for a line, per the NeTEx-fr naming rules.
fn line_filename(network: &Network, line: &Line) -> String {
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

    format!("{base}_{:x}.xml", md5::compute(line.id.as_bytes()))
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
        self.write_lines(&path)?;
        self.write_networks(&path)?;
        self.write_stops(&path)?;
        self.write_resource(&path)?;
        self.write_offers(&path)?;
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
    // Include 'stop_frame' into a complete NeTEx XML tree with
    // 'PublicationDelivery' and 'dataObjects'
    // `_version_type` is currently unused: `PublicationDelivery/@version` is
    // now fixed to `FRANCE` (NeTEx-fr 2.4). It will be needed again to set
    // the `version` attribute on each individual frame (NeTEx-fr 2.4 §1,
    // not yet implemented).
    fn wrap_frame(&self, frame: Element, _version_type: VersionType) -> Element {
        let publication_timestamp = Element::builder("PublicationTimestamp")
            .append(self.timestamp.to_rfc3339())
            .build();
        let participant_ref = Element::builder("ParticipantRef")
            .append(self.participant_ref.as_str())
            .build();
        let data_objects = Element::builder("dataObjects").append(frame).build();
        Element::builder("PublicationDelivery")
            .attr(
                "version",
                format!("1.3:FR-NETEX_{}-2.4", VersionType::France),
            )
            .attr("xmlns", "http://www.netex.org.uk/netex")
            .append(publication_timestamp)
            .append(participant_ref)
            .append(data_objects)
            .build()
    }

    fn generate_frame_id(&self, frame_type: FrameType, id: &str) -> String {
        format!("FR:{frame_type}:{id}:")
    }

    fn create_composite_frame<I, T>(id: String, frames: I) -> Element
    where
        I: IntoIterator<Item = T>,
        T: Into<Node>,
    {
        let frame_list = Element::builder("frames").append_all(frames).build();
        Element::builder(FrameType::Composite.to_string())
            .attr("id", id)
            .attr("version", "any")
            .append(frame_list)
            .build()
    }

    pub(crate) fn create_members<I, T>(members: I) -> Element
    where
        I: IntoIterator<Item = T>,
        T: Into<Node>,
    {
        Element::builder("members").append_all(members).build()
    }

    fn write_lines<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let filepath = path.as_ref().join(NETEX_FRANCE_LINES_FILENAME);
        let file = BufWriter::new(File::create(&filepath)?);
        let lines_frame = self.create_lines_frame()?;
        let composite_frame_id = self.generate_frame_id(
            FrameType::Composite,
            &format!("NETEX_{}", VersionType::Lines),
        );
        let composite_frame =
            Self::create_composite_frame(composite_frame_id, iter::once(lines_frame));
        let netex = self.wrap_frame(composite_frame, VersionType::Lines);
        let mut writer = ElementWriter::pretty(file);
        info!("Writing {:?}", &filepath);
        writer.write(&netex)?;
        Ok(())
    }

    // Returns a 'ServiceFrame' containing a list of 'Line' in 'lines'
    fn create_lines_frame(&self) -> Result<Element> {
        let line_exporter = LineExporter::new(self.model);
        let lines = line_exporter.export()?;
        let line_list = Element::builder("lines").append_all(lines).build();
        let service_frame_id = self.generate_frame_id(FrameType::Service, "lines");
        let frame = Element::builder(FrameType::Service.to_string())
            .attr("id", service_frame_id)
            .attr("version", "any")
            .append(line_list)
            .build();
        Ok(frame)
    }

    fn write_networks<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let filepath = path.as_ref().join(NETEX_FRANCE_NETWORK_FILENAME);
        let file = BufWriter::new(File::create(&filepath)?);
        let networks_frame = self.create_networks_frame();
        let netex = self.wrap_frame(networks_frame, VersionType::Networks);
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
        let general_frame_id = self.generate_frame_id(
            FrameType::General,
            &format!("NETEX_{}", VersionType::Networks),
        );
        Element::builder(FrameType::General.to_string())
            .attr("id", general_frame_id)
            .attr("version", "any")
            .append(members)
            .build()
    }

    // Returns a 'GeneralFrame' containing all 'Operator' and 'SiteConnection'
    fn create_common_frame(&self) -> Result<Element> {
        let company_exporter = CompanyExporter::new(self.model);
        let companies = company_exporter.export();
        let transfer_exporter = TransferExporter::new(self.model);
        let transfers = transfer_exporter.export()?;
        let members = Self::create_members(companies.into_iter().chain(transfers));
        let general_frame_id = self.generate_frame_id(
            FrameType::General,
            &format!("NETEX_{}", VersionType::Common),
        );
        Ok(Element::builder(FrameType::General.to_string())
            .attr("id", general_frame_id)
            .attr("version", "any")
            .append(members)
            .build())
    }

    fn write_stops<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let filepath = path.as_ref().join(NETEX_FRANCE_STOPS_FILENAME);
        let file = BufWriter::new(File::create(&filepath)?);
        let stop_frame = self.create_stops_frame()?;
        let netex = self.wrap_frame(stop_frame, VersionType::Stops);
        let mut writer = ElementWriter::pretty(file);
        info!("Writing {:?}", &filepath);
        writer.write(&netex)?;
        Ok(())
    }

    // Returns a 'GeneralFrame' containing all 'StopArea' and 'Quay'
    fn create_stops_frame(&self) -> Result<Element> {
        let stop_exporter = StopExporter::new(self.model, &self.participant_ref)?;
        let stops = stop_exporter.export()?;
        let members = Self::create_members(stops);
        let general_frame_id =
            self.generate_frame_id(FrameType::General, &format!("NETEX_{}", VersionType::Stops));
        let frame = Element::builder(FrameType::General.to_string())
            .attr("id", general_frame_id)
            .attr("version", "any")
            .append(members)
            .build();
        Ok(frame)
    }

    fn write_resource<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let filepath = path.as_ref().join(NETEX_FRANCE_RESOURCE_FILENAME);
        let file = BufWriter::new(File::create(&filepath)?);
        let common_frame = self.create_common_frame()?;
        let calendars_frame = self.create_calendars_frame()?;
        let composite_frame_id = self.generate_frame_id(
            FrameType::Composite,
            &format!("NETEX_{}", VersionType::France),
        );
        let composite_frame =
            Self::create_composite_frame(composite_frame_id, [common_frame, calendars_frame]);
        let netex = self.wrap_frame(composite_frame, VersionType::France);
        let mut writer = ElementWriter::pretty(file);
        info!("Writing {:?}", &filepath);
        writer.write(&netex)?;
        Ok(())
    }

    // Returns a 'GeneralFrame' containing all 'DayType', 'DayTypeAssignment' and 'UicOperatingPeriod'
    fn create_calendars_frame(&self) -> Result<Element> {
        let calendar_exporter = CalendarExporter::new(self.model);
        let calendars = calendar_exporter.export()?;
        let valid_between = self.create_valid_between()?;
        let members = Self::create_members(calendars);
        let general_frame_id = self.generate_frame_id(
            FrameType::General,
            &format!("NETEX_{}", VersionType::Calendars),
        );
        let frame = Element::builder(FrameType::General.to_string())
            .attr("id", general_frame_id)
            .attr("version", "any")
            .append(valid_between)
            .append(members)
            .build();
        Ok(frame)
    }

    fn create_valid_between(&self) -> Result<Element> {
        let format_date = |date: Date, hour, minute, second| -> String {
            DateTime::<Utc>::from_naive_utc_and_offset(
                date.and_hms_opt(hour, minute, second).unwrap(),
                Utc,
            )
            .to_rfc3339()
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

    fn write_offers<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let offer_exporter = OfferExporter::new(self.model)?;

        // Phase 1 (sequential): create network directories and collect all work items.
        let work_items: Vec<(PathBuf, Idx<Line>)> = self.model.networks.values().try_fold(
            Vec::new(),
            |mut acc, network| -> Result<Vec<(PathBuf, Idx<Line>)>> {
                let network_id_md5 = md5::compute(network.id.as_bytes());
                let folder_name = format!(
                    "reseau_{}_{:x}",
                    only_alphanumeric(&network.name),
                    network_id_md5
                );
                let network_path = path.as_ref().join(folder_name);
                fs::create_dir(&network_path)?;
                // Unwrap is safe because we're iterating over existing networks
                let network_idx = self.model.networks.get_idx(&network.id).unwrap();
                let line_indexes: IdxSet<Line> = self.model.get_corresponding_from_idx(network_idx);
                acc.extend(
                    line_indexes
                        .into_iter()
                        .map(|line_idx| (network_path.clone(), line_idx)),
                );
                Ok(acc)
            },
        )?;

        // Phase 2 (parallel): generate and write each offer file independently.
        work_items
            .par_iter()
            .try_for_each(|(network_path, line_idx)| -> Result<()> {
                let line = &self.model.lines[*line_idx];
                let line_id_md5 = md5::compute(line.id.as_bytes());
                let line_code = if let Some(code) = line.code.as_ref() {
                    format!("{}_", only_alphanumeric(code))
                } else {
                    String::new()
                };
                let file_name = format!("offre_{line_code}{line_id_md5:x}.xml");
                let filepath = network_path.join(&file_name);
                let file = BufWriter::new(File::create(&filepath)?);
                let offer_frame = self.create_offer_frame(&offer_exporter, *line_idx)?;
                let netex = self.wrap_frame(offer_frame, VersionType::Schedule);
                let mut writer = ElementWriter::pretty(file);
                info!("Writing {:?}", &filepath);
                writer.write(&netex)?;
                Ok(())
            })?;

        Ok(())
    }

    // Returns a 'GeneralFrame' containing all the schedules for a line
    fn create_offer_frame(
        &self,
        offer_exporter: &OfferExporter,
        line_idx: Idx<Line>,
    ) -> Result<Element> {
        let offer = offer_exporter.export(line_idx)?;
        let members = Self::create_members(offer);
        let general_frame_id = self.generate_frame_id(
            FrameType::General,
            &format!("NETEX_{}", VersionType::Schedule),
        );
        let frame = Element::builder(FrameType::General.to_string())
            .attr("id", general_frame_id)
            .attr("version", "any")
            .append(members)
            .build();
        Ok(frame)
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
            let network = Network {
                id: "TCL".to_string(),
                name: "TCL".to_string(),
                ..Default::default()
            };
            let line = Line {
                id: "SYTNEX:102".to_string(),
                code: Some("102".to_string()),
                name: "Croix-Rousse - Plateau de Saint Rambert".to_string(),
                ..Default::default()
            };
            assert_eq!(
                line_filename(&network, &line),
                "line_tcl_102_croix_rousse_plateau_de_saint_rambert_f2a4eb5f0123026eb1e7ccde5ef6eba3.xml"
            );
        }

        #[test]
        fn name_identical_to_code_is_dropped() {
            // IDFM, line IDFM:C01624: line_code and line_name are both "4244".
            let network = Network {
                id: "IDFM:1080".to_string(),
                name: "Evry Centre Essonne".to_string(),
                ..Default::default()
            };
            let line = Line {
                id: "IDFM:C01624".to_string(),
                code: Some("4244".to_string()),
                name: "4244".to_string(),
                ..Default::default()
            };
            assert_eq!(
                line_filename(&network, &line),
                "line_evry_centre_essonne_4244_e0cb7609cdbd9bf17f4d989dd34bbb94.xml"
            );
        }

        #[test]
        fn accented_network_and_line_names() {
            // CH, network "Sihltal-Zürich-Uetliberg-Bahn", line OCH:91-10-A-j26-1.
            let network = Network {
                id: "OCH:78".to_string(),
                name: "Sihltal-Zürich-Uetliberg-Bahn".to_string(),
                ..Default::default()
            };
            let line = Line {
                id: "OCH:91-10-A-j26-1".to_string(),
                code: Some("S10".to_string()),
                name: "Uetliberg - Zürich HB".to_string(),
                ..Default::default()
            };
            assert_eq!(
                line_filename(&network, &line),
                "line_sihltal_zurich_uetliberg_bahn_s10_uetliberg_zurich_hb_ce505b256619e5d786e4ce00b75dd2e2.xml"
            );
        }

        #[test]
        fn non_latin_network_name_falls_back_to_network_id() {
            // Israel dataset, network ISR:135 (Hebrew name), line ISR:5878.
            // This is the real-world case cited in the working doc's own example.
            let network = Network {
                id: "ISR:135".to_string(),
                name: "דרך אגד עוטף ירושלים".to_string(),
                ..Default::default()
            };
            let line = Line {
                id: "ISR:5878".to_string(),
                code: Some("104".to_string()),
                name: "ממילא/קריב-ירושלים<->שכונה י''א א-מבשרת ציון-1#".to_string(),
                ..Default::default()
            };
            assert_eq!(
                line_filename(&network, &line),
                "line_isr_135_104_1_6e4e7238e072c1ad1280fda127906606.xml"
            );
        }

        #[test]
        fn no_code_is_omitted() {
            let network = Network {
                id: "RER".to_string(),
                name: "RER".to_string(),
                ..Default::default()
            };
            let line = Line {
                id: "RATP:Line:A".to_string(),
                code: None,
                name: "RER A".to_string(),
                ..Default::default()
            };
            assert_eq!(
                line_filename(&network, &line),
                "line_rer_a_cb68c30f80b859d8d82ff5cd44e8305e.xml"
            );
        }

        #[test]
        fn name_absent_with_code_present() {
            let network = Network {
                id: "RATP".to_string(),
                name: "RATP".to_string(),
                ..Default::default()
            };
            let line = Line {
                id: "RATP:Line:14".to_string(),
                code: Some("14".to_string()),
                name: "".to_string(),
                ..Default::default()
            };
            assert_eq!(
                line_filename(&network, &line),
                "line_ratp_14_9ad325ee67c40600689d1badac32e1d0.xml"
            );
        }

        #[test]
        fn neither_code_nor_name_exploitable() {
            let network = Network {
                id: "NET".to_string(),
                name: "NET".to_string(),
                ..Default::default()
            };
            let line = Line {
                id: "X:4".to_string(),
                code: None,
                name: "אגד".to_string(),
                ..Default::default()
            };
            assert_eq!(
                line_filename(&network, &line),
                "line_net_f4031a78bc723373381168309ce57a54.xml"
            );
        }

        #[test]
        fn network_name_and_id_both_empty_falls_back_to_x() {
            let network = Network {
                id: "".to_string(),
                name: "".to_string(),
                ..Default::default()
            };
            let line = Line {
                id: "X:1".to_string(),
                code: Some("14".to_string()),
                name: "Ligne 14".to_string(),
                ..Default::default()
            };
            assert_eq!(
                line_filename(&network, &line),
                "line_x_14_ligne_14_eb1a6accc61ea1d3c179abd0a367ed49.xml"
            );
        }

        #[test]
        fn network_and_code_segments_are_merged_when_identical() {
            // network slug and line code slug are both "bus" —
            // merge_adjacent_duplicate_segments collapses them into one.
            let network = Network {
                id: "Bus".to_string(),
                name: "Bus".to_string(),
                ..Default::default()
            };
            let line = Line {
                id: "X:3".to_string(),
                code: Some("Bus".to_string()),
                name: "".to_string(),
                ..Default::default()
            };
            assert_eq!(
                line_filename(&network, &line),
                "line_bus_33409777a8bb7c6347b2371c9bfdf918.xml"
            );
        }

        #[test]
        fn filename_never_exceeds_the_netex_fr_250_char_limit() {
            // 250 characters is the NeTEx-fr profile's own limit on filenames,
            // not an arbitrary choice here.
            let very_long_text = "a".repeat(300);
            let network = Network {
                id: very_long_text.clone(),
                name: very_long_text.clone(),
                ..Default::default()
            };
            let line = Line {
                id: "X".to_string(),
                code: Some(very_long_text.clone()),
                name: very_long_text,
                ..Default::default()
            };
            assert!(
                line_filename(&network, &line).len() <= 250,
                "line_*.xml filenames must stay under the NeTEx-fr 250-character limit"
            );
        }
    }
}
