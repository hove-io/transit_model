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
use std::path::PathBuf;
use structopt::StructOpt;
use tracing::info;
use tracing_subscriber::{
    filter::{EnvFilter, LevelFilter},
    layer::SubscriberExt as _,
    util::SubscriberInitExt as _,
};
use transit_model::{transfers::generates_transfers, Result};

lazy_static::lazy_static! {
    pub static ref GIT_VERSION: String = transit_model::binary_full_version(env!("CARGO_PKG_VERSION"));
}

fn get_version() -> &'static str {
    &GIT_VERSION
}

#[derive(Debug, StructOpt)]
#[structopt(name = "ntfs2ntfs", about = "Convert an NTFS to an NTFS.", version = get_version())]
struct Opt {
    /// Input directory.
    #[structopt(short = "i", long = "input", parse(from_os_str), default_value = ".")]
    input: PathBuf,

    /// Output directory.
    #[structopt(short = "o", long = "output", parse(from_os_str))]
    output: Option<PathBuf>,

    /// Current datetime.
    #[structopt(
        short = "x",
        long,
        parse(try_from_str),
        default_value = &transit_model::CURRENT_DATETIME
    )]
    current_datetime: DateTime<FixedOffset>,

    /// The maximum distance in meters to compute the tranfer.
    #[structopt(long, short = "d", default_value = transit_model::TRANSFER_MAX_DISTANCE)]
    max_distance: f64,

    /// The walking speed in meters per second. You may want to divide your
    /// initial speed by sqrt(2) to simulate Manhattan distances.
    #[structopt(long, short = "s", default_value = transit_model::TRANSFER_WALKING_SPEED)]
    walking_speed: f64,

    /// Waiting time at stop in seconds.
    #[structopt(long, short = "t", default_value = transit_model::TRANSFER_WAITING_TIME)]
    waiting_time: u32,
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
    info!("Launching ntfs2ntfs...");

    let model = transit_model::ntfs::read(opt.input)?;
    let model = generates_transfers(
        model,
        opt.max_distance,
        opt.walking_speed,
        opt.waiting_time,
        None,
    )?;

    if let Some(output) = opt.output {
        match output.extension() {
            Some(ext) if ext == "zip" => {
                transit_model::ntfs::write_to_zip(&model, output, opt.current_datetime)?;
            }
            _ => {
                transit_model::ntfs::write(&model, output, opt.current_datetime)?;
            }
        };
    }
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
