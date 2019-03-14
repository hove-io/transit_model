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

//! See function apply_rules

use crate::collection::{CollectionWithId, Id};
use crate::model::Collections;
use crate::objects::{Codes, Geometry};
use crate::utils::{Report, ReportType};
use crate::Result;
use csv;
use failure::ResultExt;
use geo_types::Geometry as GeoGeometry;
use lazy_static::lazy_static;
use log::{info, warn};
use serde_derive::Deserialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use wkt::{self, conversion::try_into_geometry};

#[derive(Deserialize, Debug, Ord, PartialOrd, Eq, PartialEq, Clone, Copy, Hash)]
#[serde(rename_all = "snake_case")]
enum ObjectType {
    Line,
    Route,
    StopPoint,
    StopArea,
}
impl ObjectType {
    pub fn as_str(&self) -> &'static str {
        match *self {
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

#[derive(Deserialize, Debug, Ord, Eq, PartialOrd, PartialEq, Clone)]
struct PropertyRule {
    object_type: ObjectType,
    object_id: String,
    property_name: String,
    property_old_value: Option<String>,
    property_value: String,
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

fn read_property_rules_files<P: AsRef<Path>>(
    rule_files: Vec<P>,
    report: &mut Report,
) -> Result<Vec<PropertyRule>> {
    info!("Reading property rules.");
    let mut properties: BTreeMap<(ObjectType, String, String), BTreeSet<PropertyRule>> =
        BTreeMap::default();
    for rule_path in rule_files {
        let path = rule_path.as_ref();
        let mut rdr = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_path(&path)
            .with_context(ctx_from_path!(path))?;
        for p in rdr.deserialize() {
            let p: PropertyRule = match p {
                Ok(val) => val,
                Err(e) => {
                    report.add_warning(
                        format!("Error reading {:?}: {}", path.file_name().unwrap(), e),
                        ReportType::InvalidFile,
                    );
                    continue;
                }
            };

            properties
                .entry((p.object_type, p.object_id.clone(), p.property_name.clone()))
                .or_insert_with(BTreeSet::new)
                .insert(p);
        }
    }

    let properties = properties
        .into_iter()
        .filter(|((object_type, object_id, property_name), property)| {
            if !PROPERTY_UPDATER.contains_key(&(*object_type, property_name)) {
                report.add_warning(
                    format!(
                        "object_type={}, object_id={}: unknown property_name {} defined",
                        object_type.as_str(), object_id, property_name,
                    ),
                    ReportType::UnknownPropertyName,
                );
                return false;
            }

            if property.len() > 1 {
                report.add_warning(
                    format!(
                        "object_type={}, object_id={}: multiple values specified for the property {}",
                        object_type.as_str(), object_id, property_name
                    ),
                    ReportType::MultipleValue,
                );
                return false;
            }
            true
        })
        .flat_map(|(_, p)| p)
        .collect();

    Ok(properties)
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

fn update_prop<T: Clone + From<String> + Into<Option<String>>>(
    p: &PropertyRule,
    field: &mut T,
    report: &mut Report,
) {
    let any_prop = Some("*".to_string());
    if p.property_old_value == any_prop || p.property_old_value == field.clone().into() {
        *field = T::from(p.property_value.clone());
    } else {
        report.add_warning(
            format!(
                "object_type={}, object_id={}, property_name={}: property_old_value does not match the value found in the data",
                p.object_type.as_str(),
                p.object_id,
                p.property_name
            ),
            ReportType::OldPropertyValueDoesNotMatch,
        );
    }
}

fn wkt_to_geo(wkt: &str, report: &mut Report, p: &PropertyRule) -> Option<GeoGeometry<f64>> {
    if let Ok(wkt) = wkt::Wkt::from_str(wkt) {
        if let Ok(geo) = try_into_geometry(&wkt.items[0]) {
            Some(geo)
        } else {
            warn!("impossible to convert empty point");
            None
        }
    } else {
        report.add_warning(
            format!(
                "object_type={}, object_id={}: invalid geometry",
                p.object_type.as_str(),
                p.object_id,
            ),
            ReportType::GeometryNotValid,
        );
        None
    }
}

fn get_geometry_id(
    wkt: &str,
    collection: &mut CollectionWithId<Geometry>,
    p: &PropertyRule,
    report: &mut Report,
) -> Option<String> {
    if let Some(geo) = wkt_to_geo(wkt, report, p) {
        let id = p.object_type.as_str().to_owned() + ":" + &p.object_id;
        let mut obj = collection.get_or_create_with(&id, || Geometry {
            id: id.to_string(),
            geometry: geo.clone(),
        });
        if obj.geometry != geo {
            obj.geometry = geo.clone();
        }
        return Some(id);
    }

    None
}

fn update_geometry(
    p: &mut PropertyRule,
    field: &mut Option<String>,
    geometries: &mut CollectionWithId<Geometry>,
    report: &mut Report,
) {
    match (p.property_old_value.as_ref(), field.as_ref()) {
        (Some(pov), Some(geo_id)) if *pov != "*" => {
            let pov_geo = match wkt_to_geo(&pov, report, &p) {
                Some(pov_geo) => pov_geo,
                None => return,
            };
            let route_geo = match geometries.get(geo_id) {
                Some(geo) => &geo.geometry,
                None => {
                    // this should not happen
                    report.add_warning(
                        format!(
                            "object_type={}, object_id={}: geometry {} not found",
                            p.object_type.as_str(),
                            p.object_id,
                            geo_id
                        ),
                        ReportType::ObjectNotFound,
                    );
                    return;
                }
            };

            if &pov_geo != route_geo {
                update_prop(&p, field, report);
                return;
            }
            p.property_old_value = Some(geo_id.to_string())
        }
        (Some(pov), None) if *pov != "*" => {
            update_prop(&p, field, report);
            return;
        }
        (None, Some(_)) => {
            update_prop(&p, field, report);
            return;
        }
        (_, _) => {}
    }

    if let Some(id) = get_geometry_id(&p.property_value, geometries, &p, report) {
        p.property_value = id;
        update_prop(&p, field, report);
    }
}

type FnUpdater = Box<Fn(&mut Collections, &mut PropertyRule, &mut Report) -> bool + Send + Sync>;

lazy_static! {
    static ref PROPERTY_UPDATER: HashMap<(ObjectType, &'static str), FnUpdater> = {
        let mut m: HashMap<(ObjectType, &'static str), FnUpdater> = HashMap::new();
        m.insert(
            (ObjectType::Route, "route_name"),
            Box::new(|c, p, r| {
                c.routes.get_mut(&p.object_id).map_or(false, |mut route| {
                    update_prop(p, &mut route.name, r);
                    true
                })
            }),
        );
        m.insert(
            (ObjectType::Route, "direction_type"),
            Box::new(|c, p, r| {
                c.routes.get_mut(&p.object_id).map_or(false, |mut route| {
                    update_prop(p, &mut route.direction_type, r);
                    true
                })
            }),
        );
        m.insert(
            (ObjectType::Route, "destination_id"),
            Box::new(|c, p, r| {
                c.routes.get_mut(&p.object_id).map_or(false, |mut route| {
                    update_prop(p, &mut route.destination_id, r);
                    true
                })
            }),
        );
        m.insert(
            (ObjectType::Route, "route_geometry"),
            Box::new(|c, p, r| {
                let geometries = &mut c.geometries;
                c.routes.get_mut(&p.object_id).map_or(false, |mut route| {
                    update_geometry(p, &mut route.geometry_id, geometries, r);
                    true
                })
            }),
        );
        m
    };
}

/// Applying rules
///
/// `complementary_code_rules_files` Csv files containing codes to add for certain objects
pub fn apply_rules(
    collections: &mut Collections,
    complementary_code_rules_files: Vec<PathBuf>,
    property_rules_files: Vec<PathBuf>,
    report_path: PathBuf,
) -> Result<()> {
    info!("Applying rules...");
    let mut report = Report::default();
    let codes = read_complementary_code_rules_files(complementary_code_rules_files, &mut report)?;
    for code in codes {
        match code.object_type {
            ObjectType::Line => insert_code(&mut collections.lines, code, &mut report),
            ObjectType::Route => insert_code(&mut collections.routes, code, &mut report),
            ObjectType::StopPoint => insert_code(&mut collections.stop_points, code, &mut report),
            ObjectType::StopArea => insert_code(&mut collections.stop_areas, code, &mut report),
        }
    }

    let properties = read_property_rules_files(property_rules_files, &mut report)?;
    for mut p in properties {
        if let Some(func) = PROPERTY_UPDATER.get(&(p.object_type, &p.property_name.clone())) {
            if !func(collections, &mut p, &mut report) {
                report.add_warning(
                    format!(
                        "{} {} not found in the data",
                        p.object_type.as_str(),
                        p.object_id
                    ),
                    ReportType::ObjectNotFound,
                );
            }
        }
    }

    let serialized_report = serde_json::to_string_pretty(&report)?;
    fs::write(report_path, serialized_report)?;
    Ok(())
}
