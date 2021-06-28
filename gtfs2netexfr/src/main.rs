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

use chrono::{DateTime, FixedOffset};
use log::info;
use slog::{slog_o, Drain};
use slog_async::OverflowStrategy;
use std::path::PathBuf;
use structopt::StructOpt;
use transit_model::{read_utils, Result};

lazy_static::lazy_static! {
    pub static ref GIT_VERSION: String = transit_model::binary_full_version(env!("CARGO_PKG_VERSION"));
}

fn get_version() -> &'static str {
    &GIT_VERSION
}

#[derive(Debug, StructOpt)]
#[structopt(name = "gtfs2netexfr", about = "Convert a GTFS to NeTEx France.", version = get_version())]
struct Opt {
    /// Input directory.
    #[structopt(short, long, parse(from_os_str), default_value = ".")]
    input: PathBuf,

    /// Output directory.
    #[structopt(short, long, parse(from_os_str))]
    output: PathBuf,

    /// JSON file containing additional configuration.
    ///
    /// For more information, see
    /// https://github.com/CanalTP/transit_model/blob/master/documentation/common_ntfs_rules.md#configuration-of-each-converter
    #[structopt(short, long, parse(from_os_str))]
    config: Option<PathBuf>,

    /// Indicates if the input GTFS contains On-Demand Transport (ODT)
    /// information.
    #[structopt(short = "t", long = "on-demand-transport")]
    odt: bool,

    /// On-Demand Transport GTFS comment
    #[structopt(long = "odt-comment")]
    odt_comment: Option<String>,

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
        parse(try_from_str),
        default_value = &transit_model::CURRENT_DATETIME
    )]
    current_datetime: DateTime<FixedOffset>,
}

fn init_logger() -> slog_scope::GlobalLoggerGuard {
    let decorator = slog_term::TermDecorator::new().stdout().build();
    let drain = slog_term::CompactFormat::new(decorator).build().fuse();
    let mut builder = slog_envlogger::LogBuilder::new(drain).filter(None, slog::FilterLevel::Info);
    if let Ok(s) = std::env::var("RUST_LOG") {
        builder = builder.parse(&s);
    }
    let drain = slog_async::Async::new(builder.build())
        .chan_size(256) // Double the default size
        .overflow_strategy(OverflowStrategy::Block)
        .build()
        .fuse();
    let logger = slog::Logger::root(drain, slog_o!());

    let scope_guard = slog_scope::set_global_logger(logger);
    slog_stdlog::init().unwrap();
    scope_guard
}

fn run(opt: Opt) -> Result<()> {
    info!("Launching gtfs2netexfr...");

    let (contributor, dataset, feed_infos) = read_utils::read_config(opt.config)?;
    let configuration = transit_model::gtfs::Configuration {
        contributor,
        dataset,
        feed_infos,
        on_demand_transport: opt.odt,
        on_demand_transport_comment: opt.odt_comment,
        ..Default::default()
    };

    let model = transit_model::gtfs::Reader::new(configuration).parse(opt.input)?;

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
    let _log_guard = init_logger();
    if let Err(err) = run(Opt::from_args()) {
        for cause in err.iter_chain() {
            eprintln!("{}", cause);
        }
        std::process::exit(1);
    }
}
