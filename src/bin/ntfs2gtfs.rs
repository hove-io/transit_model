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

use env_logger;
#[macro_use]
extern crate log;
use navitia_model;
use structopt;

use std::path::PathBuf;
use structopt::StructOpt;

use navitia_model::Result;

#[derive(Debug, StructOpt)]
#[structopt(name = "ntfs2gtfs", about = "Convert an NTFS to a GTFS.")]
struct Opt {
    /// input directory.
    #[structopt(short = "i", long = "input", parse(from_os_str), default_value = ".")]
    input: PathBuf,

    /// output directory
    #[structopt(short = "o", long = "output", parse(from_os_str))]
    output: PathBuf,
}

fn run() -> Result<()> {
    info!("Launching ntfs2gtfs...");
    let opt = Opt::from_args();
    let model = navitia_model::ntfs::read(opt.input)?;
    navitia_model::gtfs::write(&model, opt.output)?;
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
