// Copyright 2017 Hove and/or its affiliates.
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

use chrono::{DateTime, FixedOffset};
use clap::Parser;
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::{
    filter::{EnvFilter, LevelFilter},
    layer::SubscriberExt as _,
    util::SubscriberInitExt as _,
};
use transit_model::{configuration, objects::VehicleJourneyScheduleType, Model, Result};

lazy_static::lazy_static! {
    pub static ref GIT_VERSION: String = transit_model::binary_full_version(env!("CARGO_PKG_VERSION"));
}

fn get_version() -> &'static str {
    &GIT_VERSION
}

#[derive(Debug, Parser)]
#[command(name = "gtfs2netexfr", about = "Convert a GTFS to NeTEx France.", version = get_version())]
struct Opt {
    /// Input directory.
    #[arg(short, long, default_value = ".")]
    input: PathBuf,

    /// Output directory.
    #[arg(short, long)]
    output: PathBuf,

    /// JSON file containing additional configuration.
    ///
    /// For more information, see
    /// https://github.com/hove-io/transit_model/blob/master/documentation/common_ntfs_rules.md#configuration-of-each-converter
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Indicates if the input GTFS contains On-Demand Transport (ODT)
    /// information.
    #[arg(short = 't', long = "on-demand-transport")]
    odt: bool,

    /// On-Demand Transport GTFS comment
    #[arg(long = "odt-comment")]
    odt_comment: Option<String>,

    /// Name for the participant.
    ///
    /// For more information, see
    /// https://github.com/hove-io/transit_model/blob/master/documentation/ntfs_to_netex_france_specs.md#input-parameters
    #[arg(short, long)]
    participant: String,

    /// Code for the provider of stops.
    ///
    /// For more information, see
    /// https://github.com/hove-io/transit_model/blob/master/documentation/ntfs_to_netex_france_specs.md#input-parameters
    #[arg(short, long)]
    stop_provider: Option<String>,

    /// Current datetime.
    #[arg(
        short = 'x',
        long,
        default_value = &**transit_model::CURRENT_DATETIME
    )]
    current_datetime: DateTime<FixedOffset>,
}

fn init_logger() {
    let default_level = LevelFilter::INFO;
    let rust_log =
        std::env::var(EnvFilter::DEFAULT_ENV).unwrap_or_else(|_| default_level.to_string());
    let env_filter_subscriber = EnvFilter::try_new(rust_log).unwrap_or_else(|e| {
        eprintln!(
            "invalid {}, falling back to level '{}' - {}",
            EnvFilter::DEFAULT_ENV,
            default_level,
            e,
        );
        EnvFilter::new(default_level.to_string())
    });
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(env_filter_subscriber)
        .init();
}

fn run(opt: Opt) -> Result<()> {
    info!("Launching gtfs2netexfr...");

    let (contributor, dataset, feed_infos) = configuration::read_config(opt.config)?;
    let configuration = transit_model::gtfs::Configuration {
        contributor,
        dataset,
        feed_infos,
        on_demand_transport: opt.odt,
        on_demand_transport_comment: opt.odt_comment,
        ..Default::default()
    };

    let mut collections =
        transit_model::gtfs::Reader::new(configuration).parse_collections(opt.input)?;
    collections
        .filter_by_vj_schedule_types(vec![VehicleJourneyScheduleType::ArrivalDepartureTimesOnly])?;
    let model = Model::new(collections)?;

    let mut config = transit_model::netex_france::WriteConfiguration::new(opt.participant)
        .current_datetime(opt.current_datetime);
    if let Some(stop_provider) = opt.stop_provider {
        config = config.stop_provider(stop_provider);
    }
    match opt.output.extension() {
        Some(ext) if ext == "zip" => {
            transit_model::netex_france::write_to_zip(&model, opt.output, config)?;
        }
        _ => {
            transit_model::netex_france::write(&model, opt.output, config)?;
        }
    };

    Ok(())
}

fn main() {
    init_logger();
    if let Err(err) = run(Opt::parse()) {
        for cause in err.chain() {
            eprintln!("{}", cause);
        }
        std::process::exit(1);
    }
}
