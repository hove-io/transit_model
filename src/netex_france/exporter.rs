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
use crate::{minidom_utils::ElementWriter, model::Model, Result};
use chrono::prelude::*;
use minidom::Element;
use std::{
    convert::AsRef,
    fmt::{self, Display, Formatter},
    fs::File,
    path::Path,
};

const NETEX_FRANCE_STOPS_FILENAME: &str = "arrets.xml";

enum VersionType {
    Stops,
}

impl Display for VersionType {
    fn fmt(&self, fmt: &mut Formatter) -> std::result::Result<(), fmt::Error> {
        use VersionType::*;
        match self {
            Stops => write!(fmt, "ARRET"),
        }
    }
}

/// Struct that can write an export of Netex France profile from a Model
pub struct Exporter {
    _model: Model,
    participant_ref: String,
    timestamp: NaiveDateTime,
}

// Publicly exposed methods
impl Exporter {
    /// Build a Netex France profile exporter from the model.
    /// `path` is the expected output Path where the Netex France is going to be
    /// written. It should be a folder that already exists.
    pub fn new(_model: Model, participant_ref: String, timestamp: NaiveDateTime) -> Self {
        Exporter {
            _model,
            participant_ref,
            timestamp,
        }
    }

    /// Actually write `model` into `path` as a Netex France profile.
    pub fn write<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        self.write_stops(path)?;
        Ok(())
    }
}

// Internal methods
impl Exporter {
    // Include 'stop_frame' into a complete NeTEx XML tree with
    // 'PublicationDelivery' and 'dataObjects'
    fn wrap_frame(&self, frame: Element, version_type: VersionType) -> Result<Element> {
        let publication_timestamp = Element::builder("PublicationTimestamp")
            .ns("http://www.netex.org.uk/netex/")
            // FIXME: This is not compliant with ISO8601 (the final offset is missing)
            .append(self.timestamp.format("%FT%T").to_string())
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
            .ns("http://www.netex.org.uk/netex/")
            .attr("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance")
            .attr("version", format!("1.09:FR-NETEX_{}-2.1-1.0", version_type))
            .append(publication_timestamp)
            .append(participant_ref)
            .append(data_objects)
            .build();
        Ok(root)
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
        let frame = Element::builder("GeneralFrame").build();
        Ok(frame)
    }
}
