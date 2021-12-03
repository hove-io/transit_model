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

use ntfs2gtfs::add_mode_to_line_code;
use std::path::PathBuf;
use structopt::StructOpt;
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

#[derive(Debug, StructOpt)]
#[structopt(name = "ntfs2gtfs", about = "Convert an NTFS to a GTFS.", version = get_version())]
struct Opt {
    /// Input directory.
    #[structopt(short, long, parse(from_os_str), default_value = ".")]
    input: PathBuf,

    /// Output directory.
    #[structopt(short, long, parse(from_os_str))]
    output: PathBuf,

    /// Add the commercial mode at the beginning of the route short name.
    #[structopt(short, long)]
    mode_in_route_short_name: bool,
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
    info!("Launching ntfs2gtfs...");
    let mut collections = transit_model::ntfs::read_collections(opt.input)?;
    collections.remove_stop_zones();
    collections.remove_route_points();
    let mut model = Model::new(collections)?;

    if opt.mode_in_route_short_name {
        model = add_mode_to_line_code(model)?;
    }

    match opt.output.extension() {
        Some(ext) if ext == "zip" => {
            transit_model::gtfs::write_to_zip(model, opt.output)?;
        }
        _ => {
            transit_model::gtfs::write(model, opt.output)?;
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
