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

use log::info;
use ntfs2gtfs::add_mode_to_line_code;
use slog::{slog_o, Drain};
use slog_async::OverflowStrategy;
use std::path::PathBuf;
use structopt::StructOpt;
use transit_model::Result;

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
    info!("Launching ntfs2gtfs...");
    let mut model;
    model = transit_model::ntfs::read(opt.input)?;

    if opt.mode_in_route_short_name {
        model = add_mode_to_line_code(model)?;
    }

    transit_model::gtfs::write(model, opt.output)?;
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
