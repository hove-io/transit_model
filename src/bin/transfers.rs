// Copyright 2017-2018 Kisio Digital and/or its affiliates.
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
use navitia_model;
use navitia_model::transfers;
use navitia_model::transfers::TransfersMode;
use navitia_model::Result;
use std::path::PathBuf;
use structopt;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "transfers", about = "Generate transfers.")]
struct Opt {
    /// input directory.
    #[structopt(short = "i", long = "input", parse(from_os_str), default_value = ".")]
    input: PathBuf,

    /// modification rule files.
    #[structopt(short = "r", long = "rules-file", parse(from_os_str))]
    rule_files: Vec<PathBuf>,

    /// output report file path
    #[structopt(short = "l", long = "report", parse(from_os_str))]
    report: Option<PathBuf>,

    /// output directory
    #[structopt(short = "o", long = "output", parse(from_os_str))]
    output: PathBuf,

    #[structopt(
        long = "max-distance",
        short = "d",
        default_value = "500",
        help = "The max distance in meters to compute the tranfer"
    )]
    max_distance: f64,

    #[structopt(
        long = "walking-speed",
        short = "s",
        default_value = "0.785",
        help = "The walking speed in meters per second. \
                You may want to divide your initial speed by \
                sqrt(2) to simulate Manhattan distances"
    )]
    walking_speed: f64,

    #[structopt(
        long = "waiting-time",
        short = "t",
        default_value = "60",
        help = "Waiting time at stop in second"
    )]
    waiting_time: u32,
}

fn run() -> Result<()> {
    info!("Launching transfers...");

    let opt = Opt::from_args();

    let model = navitia_model::ntfs::read(opt.input)?;

    let model = transfers::generates_transfers(
        model,
        opt.max_distance,
        opt.walking_speed,
        opt.waiting_time,
        opt.rule_files,
        &TransfersMode::All,
        opt.report,
    )?;

    navitia_model::ntfs::write(&model, opt.output)?;
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
