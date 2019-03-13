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
use navitia_model;
use navitia_model::Result;
use std::path::PathBuf;
use structopt;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "gtfs2ntfs", about = "Convert a GTFS to an NTFS.")]
struct Opt {
    /// input directory.
    #[structopt(short = "i", long = "input", parse(from_os_str), default_value = ".")]
    input: PathBuf,

    /// input url.
    /// If both input and url are set, the url is used.
    #[structopt(short = "u", long = "url")]
    url: Option<String>,

    /// output directory
    #[structopt(short = "o", long = "output", parse(from_os_str))]
    output: PathBuf,

    /// config file
    #[structopt(short = "c", long = "config", parse(from_os_str))]
    config_path: Option<PathBuf>,

    /// prefix
    #[structopt(short = "p", long = "prefix")]
    prefix: Option<String>,

    /// current datetime
    #[structopt(
        short = "x",
        long,
        parse(try_from_str),
        raw(default_value = "&navitia_model::CURRENT_DATETIME")
    )]
    current_datetime: NaiveDateTime,
}

fn run() -> Result<()> {
    info!("Launching gtfs2ntfs...");

    let opt = Opt::from_args();

    let objects = if let Some(url) = opt.url {
        navitia_model::gtfs::read_from_url(&url, opt.config_path, opt.prefix)?
    } else if opt.input.is_file() {
        navitia_model::gtfs::read_from_zip(opt.input, opt.config_path, opt.prefix)?
    } else {
        navitia_model::gtfs::read_from_path(opt.input, opt.config_path, opt.prefix)?
    };

    navitia_model::ntfs::write(&objects, opt.output, opt.current_datetime)?;
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
