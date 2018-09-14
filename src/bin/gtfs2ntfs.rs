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

use navitia_model::Result;

#[derive(Debug, StructOpt)]
#[structopt(name = "gtfs2ntfs", about = "Convert a GTFS to an NTFS.")]
struct Opt {
    /// input directory.
    #[structopt(
        short = "i",
        long = "input",
        parse(from_os_str),
        default_value = "."
    )]
    input: PathBuf,

    /// output directory
    #[structopt(short = "o", long = "output", parse(from_os_str))]
    output: PathBuf,

    /// config file
    #[structopt(short = "c", long = "config", parse(from_os_str))]
    config_path: Option<PathBuf>,

    /// prefix
    #[structopt(short = "p", long = "prefix")]
    prefix: Option<String>,
}

fn run() -> Result<()> {
    info!("Launching gtfs2ntfs...");

    let opt = Opt::from_args();

    let objects = navitia_model::gtfs::read(opt.input, opt.config_path, opt.prefix)?;

    navitia_model::ntfs::write(&objects, opt.output)?;
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
