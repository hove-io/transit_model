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

//! Module to handle Netex France profile

mod calendars;
use calendars::CalendarExporter;
mod companies;
use companies::CompanyExporter;
mod exporter;
use exporter::Exporter;
mod lines;
use lines::LineExporter;
use lines::LineModes;
mod modes;
use modes::NetexMode;
mod networks;
use networks::NetworkExporter;
mod offer;
use offer::OfferExporter;
mod route_points;
use route_points::build_route_points;
mod stops;
use stops::StopExporter;
mod transfers;
use transfers::TransferExporter;

use crate::{model::Model, Result};
use chrono::{DateTime, FixedOffset};

/// Configuration options for exporting a NeTEx France.
/// 3 options can be configured:
/// - participant (required): see [specifications](https://github.com/hove-io/ntfs-specification/blob/master/ntfs_to_netex_france_specs.md) for more details
/// - stop_provider (optional): see [specifications](https://github.com/hove-io/ntfs-specification/blob/master/ntfs_to_netex_france_specs.md) for more details. Default to no stop provider.
/// - current_datetime (optional): date of the export. Default to the current date of execution in UTC.
pub struct WriteConfiguration {
    participant: String,
    stop_provider: Option<String>,
    current_datetime: DateTime<FixedOffset>,
}

impl WriteConfiguration {
    /// Create a new `WriteConfiguration`.
    pub fn new<S: Into<String>>(participant: S) -> Self {
        WriteConfiguration {
            participant: participant.into(),
            stop_provider: None,
            current_datetime: chrono::Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap()),
        }
    }
    /// Setup the Stop Provider (see [specifications](https://github.com/hove-io/ntfs-specification/blob/master/ntfs_to_netex_france_specs.md) for more details)
    pub fn stop_provider<S: Into<String>>(self, stop_provider: S) -> Self {
        WriteConfiguration {
            stop_provider: Some(stop_provider.into()),
            ..self
        }
    }
    /// Setup the date and time of the export.
    pub fn current_datetime(self, current_datetime: DateTime<FixedOffset>) -> Self {
        WriteConfiguration {
            current_datetime,
            ..self
        }
    }
}

/// Exports a `Model` to the
/// [NeTEx France](https://github.com/hove-io/ntfs-specification/blob/master/ntfs_to_netex_france_specs.md)
/// files in the given directory.
pub fn write<P: AsRef<std::path::Path>>(
    model: &Model,
    path: P,
    config: WriteConfiguration,
) -> Result<()> {
    let exporter = Exporter::new(
        model,
        config.participant,
        config.stop_provider,
        config.current_datetime,
    );
    exporter.write(path)?;
    Ok(())
}

/// Exports a `Model` to a
/// [NeTEx France](https://github.com/hove-io/ntfs-specification/blob/master/ntfs_to_netex_france_specs.md)
/// ZIP archive at the given full path.
pub fn write_to_zip<P: AsRef<std::path::Path>>(
    model: &Model,
    path: P,
    config: WriteConfiguration,
) -> Result<()> {
    let output_dir = tempfile::tempdir()?;
    write(model, output_dir.path(), config)?;
    crate::utils::zip_to(output_dir.path(), path)?;
    output_dir.close()?;
    Ok(())
}
