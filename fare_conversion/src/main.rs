// Copyright (C) 2017 Kisio Digital and/or its affiliates.
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

use chrono::Utc;
use transit_model::model::GetCorresponding;
use transit_model::{Model, Result};
use transit_model_collection::{CollectionWithId, Id, Idx};
use transit_model_relations::IdxSet;


fn run() -> Result<()> {
    let input_dir = std::path::Path::new(".");
    let objects = transit_model::ntfs::read(&input_dir)?;
    let output_dir = input_dir.join("output");
    std::fs::create_dir_all(&output_dir)?;
    transit_model::ntfs::write(&objects, &output_dir, Utc::now().naive_utc())?;

    Ok(())
}

fn main() {
    if let Err(err) = run() {
        for cause in err.iter_chain() {
            eprintln!("{}", cause);
        }
        std::process::exit(1);
    }
}
