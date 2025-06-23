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
use transit_model::{configuration, transfers::generates_transfers, PrefixConfiguration, Result};

lazy_static::lazy_static! {
    pub static ref GIT_VERSION: String = transit_model::binary_full_version(env!("CARGO_PKG_VERSION"));
}

fn get_version() -> &'static str {
    &GIT_VERSION
}

#[derive(Debug, Parser)]
#[command(name = "gtfs2ntfs", about = "Convert a GTFS to an NTFS.", version = get_version())]
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

    /// Prefix added to all the identifiers (`123` turned into `prefix:123`).
    #[arg(short, long)]
    prefix: Option<String>,

    /// Schedule subprefix added after the prefix on all scheduled objects (`123` turned into `prefix::schedule_subprefix::123`).
    #[arg(long)]
    schedule_subprefix: Option<String>,

    /// Indicates if the input GTFS contains On-Demand Transport (ODT)
    /// information.
    #[arg(long)]
    odt: bool,

    /// On-Demand Transport GTFS comment.
    #[arg(long = "odt-comment")]
    odt_comment: Option<String>,

    /// If true, each GTFS `Route` will generate a different `Line`.
    /// Else we group the routes by `agency_id` and `route_short_name`
    /// (or `route_long_name` if the short name is empty) and create a `Line` for each group.
    #[arg(long = "read-as-line")]
    read_as_line: bool,

    /// Current datetime.
    #[arg(
        short = 'x',
        long,
        default_value = &**transit_model::CURRENT_DATETIME
    )]
    current_datetime: DateTime<FixedOffset>,

    /// The maximum distance in meters to compute the tranfer.
    #[arg(long, short = 'd', default_value = transit_model::TRANSFER_MAX_DISTANCE)]
    max_distance: f64,

    /// The walking speed in meters per second. You may want to divide your
    /// initial speed by sqrt(2) to simulate Manhattan distances.
    #[arg(long, short = 's', default_value = transit_model::TRANSFER_WALKING_SPEED)]
    walking_speed: f64,

    /// Waiting time at stop in seconds.
    #[arg(long, short = 't', default_value = transit_model::TRANSFER_WAITING_TIME)]
    waiting_time: u32,

    /// Don't compute transfers even the transfers of the stop point to itself (max_distance = 0.0)
    #[arg(long)]
    ignore_transfers: bool,

    /// Read trip_short_name as specified in the GTFS specification.
    /// if true
    ///     NTFS trip headsign = GTFS trip headsign
    ///     NTFS trip short name = GTFS trip short name
    /// if false
    ///     NTFS trip heasign = GTFS trip short name if exists else GTFS headsign
    ///     NTFS trip short name is always None
    #[arg(long)]
    read_trip_short_name: bool,
}

fn run(opt: Opt) -> Result<()> {
    info!("Launching gtfs2ntfs...");

    let (contributor, dataset, feed_infos) = configuration::read_config(opt.config)?;
    let mut prefix_conf = PrefixConfiguration::default();
    if let Some(data_prefix) = opt.prefix {
        prefix_conf.set_data_prefix(data_prefix);
    }
    if let Some(schedule_subprefix) = opt.schedule_subprefix {
        prefix_conf.set_schedule_subprefix(schedule_subprefix);
    }
    let configuration = transit_model::gtfs::Configuration {
        contributor,
        dataset,
        feed_infos,
        prefix_conf: Some(prefix_conf),
        on_demand_transport: opt.odt,
        on_demand_transport_comment: opt.odt_comment,
        read_as_line: opt.read_as_line,
        read_trip_short_name: opt.read_trip_short_name,
    };

    let model = transit_model::gtfs::Reader::new(configuration).parse(opt.input)?;

    let model = if opt.ignore_transfers {
        model
    } else {
        generates_transfers(
            model,
            opt.max_distance,
            opt.walking_speed,
            opt.waiting_time,
            None,
        )?
    };

    match opt.output.extension() {
        Some(ext) if ext == "zip" => {
            transit_model::ntfs::write_to_zip(&model, opt.output, opt.current_datetime)?;
        }
        _ => {
            transit_model::ntfs::write(&model, opt.output, opt.current_datetime)?;
        }
    };
    Ok(())
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

fn main() {
    init_logger();
    if let Err(err) = run(Opt::parse()) {
        for cause in err.chain() {
            eprintln!("{}", cause);
        }
        std::process::exit(1);
    }
}
