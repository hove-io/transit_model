// Copyright (C) 2020 Kisio Digital and/or its affiliates.
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by the
// Free Software Foundation, version 3.

// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.

// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>

use serde_json::json;
use transit_model::{gtfs, Result};

fn run() -> Result<()> {
    // read GTFS from current directory
    let objects = gtfs::read(".")?;
    // output internal model as JSON
    let json_objs = json!(objects);
    println!("{}", json_objs.to_string());
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        for cause in err.chain() {
            eprintln!("{}", cause);
        }
        std::process::exit(1);
    }
}
