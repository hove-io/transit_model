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

use chrono::NaiveDateTime;
use log::info;
use std::path::PathBuf;
use structopt::StructOpt;
use transit_model::syntus_fares;
use transit_model::Result;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "read_syntus_fares",
    about = " Enrich the data of an NTFS with Syntus fares."
)]
struct Opt {
    /// input directory.
    #[structopt(short, long, parse(from_os_str), default_value = ".")]
    input: PathBuf,

    /// syntus fares directory.
    #[structopt(short, long, parse(from_os_str), default_value = ".")]
    fares: PathBuf,

    /// output directory
    #[structopt(short, long, parse(from_os_str))]
    output: PathBuf,

    /// current datetime
    #[structopt(
        short = "x",
        long,
        parse(try_from_str),
        raw(default_value = "&transit_model::CURRENT_DATETIME")
    )]
    current_datetime: NaiveDateTime,
}

fn run() -> Result<()> {
    info!("Launching read_syntus_fares.");
    let opt = Opt::from_args();
    let model = transit_model::ntfs::read(opt.input)?;
    let (tickets, od_rules) = syntus_fares::read(opt.fares, &model.stop_points)?;
    let mut collections = model.into_collections();
    collections.tickets = tickets;
    collections.od_rules = od_rules;
    let model = transit_model::Model::new(collections)?;
    transit_model::ntfs::write(&model, opt.output, opt.current_datetime)?;

    Ok(())
}

fn main() {
    env_logger::init();
    if let Err(err) = run() {
        for cause in err.iter_chain() {
            eprintln!("{}", cause);
        }
        std::process::exit(1);
    }
}
