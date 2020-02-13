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

//! Exporter for Netex France profile
use crate::{
    minidom_utils::ElementWriter,
    model::Model,
    netex_france::{
        CalendarExporter, CompanyExporter, LineExporter, NetworkExporter, OfferExporter,
        StopExporter,
    },
    netex_utils::FrameType,
    objects::{Date, Line, Network},
    Result,
};
use chrono::prelude::*;
use minidom::{Element, Node};
use std::{
    convert::AsRef,
    fmt::{self, Display, Formatter},
    fs::{self, File},
    iter,
    path::Path,
};
use transit_model_collection::Idx;
use transit_model_relations::IdxSet;

const NETEX_FRANCE_CALENDARS_FILENAME: &str = "calendriers.xml";
const NETEX_FRANCE_LINES_FILENAME: &str = "lignes.xml";
const NETEX_FRANCE_STOPS_FILENAME: &str = "arrets.xml";

pub(in crate::netex_france) enum ObjectType {
    DayType,
    DayTypeAssignment,
    Line,
    Operator,
    Network,
    Quay,
    Route,
    ServiceJourneyPattern,
    StopPlace,
    UicOperatingPeriod,
}

impl Display for ObjectType {
    fn fmt(&self, f: &mut Formatter) -> std::result::Result<(), fmt::Error> {
        use ObjectType::*;
        match self {
            DayType => write!(f, "DayType"),
            DayTypeAssignment => write!(f, "DayTypeAssignment"),
            Line => write!(f, "Line"),
            Operator => write!(f, "Operator"),
            Network => write!(f, "Network"),
            Quay => write!(f, "Quay"),
            Route => write!(f, "Route"),
            ServiceJourneyPattern => write!(f, "ServiceJourneyPattern"),
            StopPlace => write!(f, "StopPlace"),
            UicOperatingPeriod => write!(f, "UicOperatingPeriod"),
        }
    }
}

enum VersionType {
    Calendars,
    Lines,
    Schedule,
    Stops,
}

impl Display for VersionType {
    fn fmt(&self, fmt: &mut Formatter) -> std::result::Result<(), fmt::Error> {
        use VersionType::*;
        match self {
            Calendars => write!(fmt, "CALENDRIER"),
            Lines => write!(fmt, "LIGNE"),
            Schedule => write!(fmt, "HORAIRE"),
            Stops => write!(fmt, "ARRET"),
        }
    }
}

fn only_alphanumeric(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).collect()
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
        self.write_lines(&path)?;
        self.write_stops(&path)?;
        self.write_calendars(&path)?;
        self.write_offers(&path)?;
        Ok(())
    }

    pub(in crate::netex_france) fn generate_id(id: &'a str, object_type: ObjectType) -> String {
        let id = id.replace(':', "_");
        format!("FR:{}:{}:", object_type, id)
    }
}

// Internal methods
impl Exporter<'_> {
    // Include 'stop_frame' into a complete NeTEx XML tree with
    // 'PublicationDelivery' and 'dataObjects'
    fn wrap_frame(&self, frame: Element, version_type: VersionType) -> Result<Element> {
        let publication_timestamp = Element::builder("PublicationTimestamp")
            .ns("http://www.netex.org.uk/netex/")
            .append(self.timestamp.to_rfc3339())
            .build();
        let participant_ref = Element::builder("ParticipantRef")
            .ns("http://www.netex.org.uk/netex/")
            .append(self.participant_ref.as_str())
            .build();
        let data_objects = Element::builder("dataObjects")
            .ns("http://www.netex.org.uk/netex/")
            .append(frame)
            .build();
        let root = Element::builder("PublicationDelivery")
            .attr("version", format!("1.09:FR-NETEX_{}-2.1-1.0", version_type))
            .attr("xmlns:siri", "http://www.siri.org.uk/siri")
            .attr("xmlns:core", "http://www.govtalk.gov.uk/core")
            .attr("xmlns:gml", "http://www.opengis.net/gml/3.2")
            .attr("xmlns:ifopt", "http://www.ifopt.org.uk/ifopt")
            .attr("xmlns:xlink", "http://www.w3.org/1999/xlink")
            .attr("xmlns", "http://www.netex.org.uk/netex")
            .attr("xsi:schemaLocation", "http://www.netex.org.uk/netex")
            .attr("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance")
            .append(publication_timestamp)
            .append(participant_ref)
            .append(data_objects)
            .build();
        Ok(root)
    }

    fn generate_frame_id(&self, frame_type: FrameType, id: &str) -> String {
        format!("FR:{}:{}:", frame_type, id)
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
        let mut file = File::create(filepath)?;
        let network_frames = self.create_networks_frames()?;
        let lines_frame = self.create_lines_frame()?;
        let companies_frame = self.create_companies_frame();
        let frames = network_frames
            .into_iter()
            .chain(iter::once(lines_frame))
            .chain(iter::once(companies_frame));
        let composite_frame_id = self.generate_frame_id(
            FrameType::Composite,
            &format!("NETEX_{}", VersionType::Lines),
        );
        let composite_frame = Self::create_composite_frame(composite_frame_id, frames);
        let netex = self.wrap_frame(composite_frame, VersionType::Lines)?;
        let writer = ElementWriter::new(netex, true);
        writer.write(&mut file)?;
        Ok(())
    }

    // Returns a list of 'ServiceFrame' each containing a 'Network'
    fn create_networks_frames(&self) -> Result<Vec<Element>> {
        let network_exporter = NetworkExporter::new(&self.model);
        let network_elements = network_exporter.export()?;
        let frames = network_elements
            .into_iter()
            .zip(self.model.networks.values())
            .map(|(network_element, network)| {
                let service_frame_id = self.generate_frame_id(FrameType::Service, &network.id);
                Element::builder(FrameType::Service.to_string())
                    .attr("id", service_frame_id)
                    .attr("version", "any")
                    .append(network_element)
                    .build()
            })
            .collect();
        Ok(frames)
    }

    // Returns a 'ServiceFrame' containing a list of 'Line' in 'lines'
    fn create_lines_frame(&self) -> Result<Element> {
        let line_exporter = LineExporter::new(&self.model);
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

    // Returns a 'ServiceFrame' containing a list of 'Operator' in 'organisations'
    fn create_companies_frame(&self) -> Element {
        let company_exporter = CompanyExporter::new(&self.model);
        let companies = company_exporter.export();
        let companies_list = Element::builder("organisations")
            .append_all(companies)
            .build();
        let resource_frame_id = self.generate_frame_id(FrameType::Resource, "operators");
        Element::builder(FrameType::Resource.to_string())
            .attr("id", resource_frame_id)
            .attr("version", "any")
            .append(companies_list)
            .build()
    }

    fn write_stops<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let filepath = path.as_ref().join(NETEX_FRANCE_STOPS_FILENAME);
        let mut file = File::create(filepath)?;
        let stop_frame = self.create_stops_frame()?;
        let netex = self.wrap_frame(stop_frame, VersionType::Stops)?;
        let writer = ElementWriter::new(netex, true);
        writer.write(&mut file)?;
        Ok(())
    }

    // Returns a 'GeneralFrame' containing all 'StopArea' and 'Quay'
    fn create_stops_frame(&self) -> Result<Element> {
        let stop_exporter = StopExporter::new(&self.model, &self.participant_ref)?;
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

    fn write_calendars<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let filepath = path.as_ref().join(NETEX_FRANCE_CALENDARS_FILENAME);
        let mut file = File::create(filepath)?;
        let calendars_frame = self.create_calendars_frame()?;
        let netex = self.wrap_frame(calendars_frame, VersionType::Calendars)?;
        let writer = ElementWriter::new(netex, true);
        writer.write(&mut file)?;
        Ok(())
    }

    // Returns a 'GeneralFrame' containing all 'DayType', 'DayTypeAssignment' and 'UicOperatingPeriod'
    fn create_calendars_frame(&self) -> Result<Element> {
        let calendar_exporter = CalendarExporter::new(&self.model);
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
            DateTime::<Utc>::from_utc(date.and_hms(hour, minute, second), Utc).to_rfc3339()
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
        for network in self.model.networks.values() {
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
            self.write_network_offers(&network_path, network_idx)?;
        }
        Ok(())
    }

    fn write_network_offers<P>(&self, network_path: P, network_idx: Idx<Network>) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let line_indexes: IdxSet<Line> = self.model.get_corresponding_from_idx(network_idx);
        let offer_exporter = OfferExporter::new(&self.model);
        for line_idx in line_indexes {
            let line = &self.model.lines[line_idx];
            let line_id_md5 = md5::compute(line.id.as_bytes());
            let line_code = if let Some(line_code) = line.code.as_ref() {
                format!("{}_", only_alphanumeric(line_code))
            } else {
                String::new()
            };
            let file_name = format!("offre_{}{:x}.xml", line_code, line_id_md5);
            let filepath = network_path.as_ref().join(file_name);
            let mut file = File::create(&filepath)?;
            let offer_frame = self.create_offer_frame(&offer_exporter, line_idx)?;
            let netex = self.wrap_frame(offer_frame, VersionType::Schedule)?;
            let writer = ElementWriter::new(netex, true);
            writer.write(&mut file)?;
        }
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
