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

use collection::CollectionWithId;
use model::Collections;
use objects::{self, Contributor};
use std::collections::BTreeSet;
use std::fs::File;
use std::path;
use utils::{add_prefix_to_collection, add_prefix_to_collection_with_id};
use Result;
extern crate serde_json;

#[derive(Deserialize, Debug)]
struct ConfigDataset {
    dataset_id: String,
}

#[derive(Deserialize, Debug)]
struct Config {
    contributor: objects::Contributor,
    dataset: ConfigDataset,
}

pub fn read_config<P: AsRef<path::Path>>(
    config_path: Option<P>,
) -> Result<(objects::Contributor, objects::Dataset)> {
    let contributor;
    let dataset;
    if let Some(config_path) = config_path {
        let json_config_file = File::open(config_path)?;
        let config: Config = serde_json::from_reader(json_config_file)?;
        info!("Reading dataset and contributor from config: {:?}", config);

        contributor = config.contributor;

        use chrono::{Duration, Utc};
        let duration = Duration::days(15);
        let today = Utc::today();
        let start_date = today - duration;
        let end_date = today + duration;
        dataset = objects::Dataset {
            id: config.dataset.dataset_id,
            contributor_id: contributor.id.clone(),
            start_date: start_date.naive_utc(),
            end_date: end_date.naive_utc(),
            dataset_type: None,
            extrapolation: false,
            desc: None,
            system: None,
        };
    } else {
        contributor = Contributor::default();
        dataset = objects::Dataset::default();
    }
    Ok((contributor, dataset))
}

pub fn add_prefix(prefix: String, collections: &mut Collections) -> Result<()> {
    let prefix = prefix + ":";
    info!("Adding prefix: \"{}\"", &prefix);
    add_prefix_to_collection_with_id(&mut collections.commercial_modes, &prefix)?;
    add_prefix_to_collection_with_id(&mut collections.networks, &prefix)?;
    add_prefix_to_collection_with_id(&mut collections.companies, &prefix)?;
    add_prefix_to_collection_with_id(&mut collections.stop_points, &prefix)?;
    add_prefix_to_collection_with_id(&mut collections.stop_areas, &prefix)?;
    add_prefix_to_collection(&mut collections.transfers, &prefix);
    add_prefix_to_collection_with_id(&mut collections.routes, &prefix)?;
    add_prefix_to_collection_with_id(&mut collections.lines, &prefix)?;
    add_prefix_to_collection_with_id(&mut collections.contributors, &prefix)?;
    add_prefix_to_collection_with_id(&mut collections.datasets, &prefix)?;
    add_prefix_to_collection_with_id(&mut collections.vehicle_journeys, &prefix)?;
    add_prefix_to_collection_with_id(&mut collections.trip_properties, &prefix)?;
    add_prefix_to_collection_with_id(&mut collections.equipments, &prefix)?;
    add_prefix_to_collection_with_id(&mut collections.comments, &prefix)?;

    Ok(())
}

pub fn get_validity_period(
    calendars: &CollectionWithId<objects::Calendar>,
) -> Option<objects::ValidityPeriod> {
    let dates = calendars.values().fold(BTreeSet::new(), |acc, c| {
        acc.union(&c.dates).cloned().collect()
    });

    if dates.is_empty() {
        return None;
    }

    Some(objects::ValidityPeriod {
        start_date: *dates.iter().next().unwrap(),
        end_date: *dates.iter().next_back().unwrap(),
    })
}
