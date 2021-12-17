// Copyright 2017 Kisio Digital and/or its affiliates.
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

use std::path::PathBuf;
use structopt::StructOpt;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tracing::info;
use tracing_subscriber::{
    filter::{EnvFilter, LevelFilter},
    layer::SubscriberExt as _,
    util::SubscriberInitExt as _,
};
use transit_model::{Model, Result};

lazy_static::lazy_static! {
    pub static ref GIT_VERSION: String = transit_model::binary_full_version(env!("CARGO_PKG_VERSION"));
}

fn get_version() -> &'static str {
    &GIT_VERSION
}

fn parse_offset_datetime(offset_date_time: &str) -> Result<OffsetDateTime, time::error::Parse> {
    OffsetDateTime::parse(offset_date_time, &Rfc3339)
}

#[derive(Debug, StructOpt)]
#[structopt(name = "ntfs2netexfr", about = "Convert a NTFS to NeTEx France.", version = get_version())]
struct Opt {
    /// Input directory.
    #[structopt(short, long, parse(from_os_str), default_value = ".")]
    input: PathBuf,

    /// Output directory
    #[structopt(short, long, parse(from_os_str))]
    output: PathBuf,

    /// Name for the participant.
    ///
    /// For more information, see
    /// https://github.com/CanalTP/transit_model/blob/master/documentation/ntfs_to_netex_france_specs.md#input-parameters
    #[structopt(short, long)]
    participant: String,

    /// Code for the provider of stops.
    ///
    /// For more information, see
    /// https://github.com/CanalTP/transit_model/blob/master/documentation/ntfs_to_netex_france_specs.md#input-parameters
    #[structopt(short, long)]
    stop_provider: Option<String>,

    /// Current datetime.
    #[structopt(
        short = "x",
        long,
        parse(try_from_str = parse_offset_datetime),
        default_value = &transit_model::CURRENT_DATETIME
    )]
    current_datetime: time::OffsetDateTime,
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
    info!("Launching ntfs2netexfr...");

    let mut collections = transit_model::ntfs::read_collections(opt.input)?;
    collections.remove_route_points();
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
    if let Err(err) = run(Opt::from_args()) {
        for cause in err.chain() {
            eprintln!("{}", cause);
        }
        std::process::exit(1);
    }
}
