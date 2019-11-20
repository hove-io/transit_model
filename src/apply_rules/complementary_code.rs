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

use crate::{
    model::Collections,
    objects::Codes,
    utils::{Report, ReportType},
    Result,
};
use failure::ResultExt;
use log::info;
use serde::Deserialize;
use std::{collections::BTreeSet, path::Path};
use transit_model_collection::{CollectionWithId, Id};

#[derive(Deserialize, Debug, Ord, PartialOrd, Eq, PartialEq, Clone, Copy, Hash)]
#[serde(rename_all = "snake_case")]
enum ObjectType {
    Line,
    Route,
    StopPoint,
    StopArea,
}

impl ObjectType {
    fn as_str(self) -> &'static str {
        match self {
            ObjectType::Line => "line",
            ObjectType::Route => "route",
            ObjectType::StopPoint => "stop_point",
            ObjectType::StopArea => "stop_area",
        }
    }
}

#[derive(Deserialize, Debug, Ord, Eq, PartialOrd, PartialEq, Clone)]
struct ComplementaryCode {
    object_type: ObjectType,
    object_id: String,
    object_system: String,
    object_code: String,
}

fn read_complementary_code_rules_files<P: AsRef<Path>>(
    rule_files: Vec<P>,
    report: &mut Report,
) -> Result<Vec<ComplementaryCode>> {
    info!("Reading complementary code rules.");
    let mut codes = BTreeSet::new();
    for rule_path in rule_files {
        let path = rule_path.as_ref();
        let mut rdr = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_path(&path)
            .with_context(ctx_from_path!(path))?;
        for c in rdr.deserialize() {
            let c: ComplementaryCode = match c {
                Ok(val) => val,
                Err(e) => {
                    report.add_warning(
                        format!("Error reading {:?}: {}", path.file_name().unwrap(), e),
                        ReportType::InvalidFile,
                    );
                    continue;
                }
            };
            codes.insert(c);
        }
    }
    Ok(codes.into_iter().collect())
}

fn insert_code<T>(
    collection: &mut CollectionWithId<T>,
    code: ComplementaryCode,
    report: &mut Report,
) where
    T: Codes + Id<T>,
{
    let idx = match collection.get_idx(&code.object_id) {
        Some(idx) => idx,
        None => {
            report.add_warning(
                format!(
                    "Error inserting code: object_codes.txt: object={},  object_id={} not found",
                    code.object_type.as_str(),
                    code.object_id
                ),
                ReportType::ObjectNotFound,
            );
            return;
        }
    };

    collection
        .index_mut(idx)
        .codes_mut()
        .insert((code.object_system, code.object_code));
}

pub fn apply_rules<P: AsRef<Path>>(
    rule_files: Vec<P>,
    collections: &mut Collections,
    mut report: &mut Report,
) -> Result<()> {
    let codes = read_complementary_code_rules_files(rule_files, &mut report)?;
    for code in codes {
        match code.object_type {
            ObjectType::Line => insert_code(&mut collections.lines, code, &mut report),
            ObjectType::Route => insert_code(&mut collections.routes, code, &mut report),
            ObjectType::StopPoint => insert_code(&mut collections.stop_points, code, &mut report),
            ObjectType::StopArea => insert_code(&mut collections.stop_areas, code, &mut report),
        }
    }

    Ok(())
}
