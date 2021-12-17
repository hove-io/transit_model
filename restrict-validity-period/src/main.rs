// Copyright 2020 Kisio Digital and/or its affiliates.
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
use time::{
    format_description::well_known::Rfc3339, macros::format_description, Date, OffsetDateTime,
};
use tracing::info;
use tracing_subscriber::{
    filter::{EnvFilter, LevelFilter},
    layer::SubscriberExt as _,
    util::SubscriberInitExt as _,
};
use transit_model::{Model, Result};

fn parse_date(date: &str) -> Result<Date, time::error::Parse> {
    Date::parse(date, format_description!("[year]-[month]-[day]"))
}

fn parse_offset_datetime(offset_date_time: &str) -> Result<OffsetDateTime, time::error::Parse> {
    OffsetDateTime::parse(offset_date_time, &Rfc3339)
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "restrict-validity-period",
    about = "Restrict the validity period of a NTFS and purge out-of-date data.",
    rename_all = "kebab-case"
)]
struct Opt {
    /// input directory.
    #[structopt(short, long, parse(from_os_str), default_value = ".")]
    input: PathBuf,

    /// start of the desired validity period [included], e.g. 2019-01-01
    #[structopt(short, long, parse(try_from_str = parse_date))]
    start_validity_date: Date,

    /// end of the desired validity period [included], e.g. 2019-01-01
    #[structopt(short, long, parse(try_from_str = parse_date))]
    end_validity_date: Date,

    /// output directory
    #[structopt(short, long, parse(from_os_str))]
    output: PathBuf,

    /// current datetime
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
    info!("Launching restrict-validity-period...");

    let model = transit_model::ntfs::read(opt.input)?;
    let mut collections = model.into_collections();
    collections.restrict_period(opt.start_validity_date, opt.end_validity_date)?;
    let model = Model::new(collections)?;
    transit_model::ntfs::write(&model, opt.output, opt.current_datetime)?;
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
