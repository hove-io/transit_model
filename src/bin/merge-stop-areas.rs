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

extern crate env_logger;
#[macro_use]
extern crate log;
extern crate navitia_model;
#[macro_use]
extern crate structopt;

use std::path::PathBuf;
use structopt::StructOpt;

use navitia_model::{Model, Result};

#[derive(Debug, StructOpt)]
#[structopt(
    name = "merge-stop-areas",
    about = "Automatic and/or manual merge of ntfs stop areas."
)]
struct Opt {
    /// input directory.
    #[structopt(short = "i", long = "input", parse(from_os_str))]
    input: PathBuf,

    /// configuration csv rules path.
    #[structopt(short = "c", long = "config", parse(from_os_str))]
    rules: Vec<PathBuf>,

    /// maximum distance in meters used to merge stop areas
    #[structopt(short = "d", long = "distance")]
    automatic_max_distance: u16,

    /// output report file path
    #[structopt(short = "r", long = "report", parse(from_os_str))]
    report: PathBuf,

    /// output directory
    #[structopt(short = "o", long = "output", parse(from_os_str))]
    output: PathBuf,
}

fn run() -> Result<()> {
    info!("Launching merge-stop-areas...");

    let opt = Opt::from_args();

    let objects = navitia_model::ntfs::read(opt.input)?;
    let mut collections = objects.into_collections();
    collections = navitia_model::merge_stop_areas::merge_stop_areas(
        collections,
        opt.rules,
        opt.automatic_max_distance,
        opt.report,
    );
    let new_model = Model::new(collections)?;

    navitia_model::ntfs::write(&new_model, opt.output)?;
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
