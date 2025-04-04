// Copyright (C) 2017 Hove and/or its affiliates.
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
//! Some utilities for input dataset to the library.

use crate::{
    objects::{self, Contributor},
    Result,
};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs::File;
use std::path;
use tracing::info;

#[derive(Deserialize, Debug)]
struct ConfigDataset {
    dataset_id: String,
    description: Option<String>,
}

#[derive(Deserialize, Debug)]
struct Config {
    contributor: objects::Contributor,
    dataset: ConfigDataset,
    feed_infos: Option<BTreeMap<String, String>>,
}

/// Read a JSON configuration file to facilitate the creation of:
/// - a Contributor
/// - a Dataset
/// - a list of key/value which will be used in 'feed_infos.txt'
///
/// Below is an example of this file
/// ```text
/// {
///     "contributor": {
///         "contributor_id": "contributor_id",
///         "contributor_name": "Contributor Name",
///         "contributor_license": "AGPIT",
///         "contributor_website": "http://www.datasource-website.com"
///     },
///     "dataset": {
///         "dataset_id": "dataset-id"
///         "description": "datasource_name"
///     },
///     "feed_infos": {
///         "feed_publisher_name": "The Great Data Publisher",
///         "feed_license": "AGPIT",
///         "feed_license_url": "http://www.datasource-website.com",
///         "tartare_platform": "dev",
///         "tartare_contributor_id": "contributor_id"
///     }
/// }
/// ```
pub fn read_config<P: AsRef<path::Path>>(
    config_path: Option<P>,
) -> Result<(
    objects::Contributor,
    objects::Dataset,
    BTreeMap<String, String>,
)> {
    let contributor;
    let dataset;
    let mut feed_infos = BTreeMap::default();

    if let Some(config_path) = config_path {
        let config_path = config_path.as_ref();
        info!("Reading dataset and contributor from {:?}", config_path);
        let json_config_file = File::open(config_path)?;
        let config: Config = serde_json::from_reader(json_config_file)?;

        contributor = config.contributor;
        dataset = objects::Dataset::new(
            config.dataset.dataset_id,
            contributor.id.clone(),
            config.dataset.description.clone(),
        );
        if let Some(config_feed_infos) = config.feed_infos {
            feed_infos = config_feed_infos;
        }
    } else {
        contributor = Contributor::default();
        dataset = objects::Dataset::default();
    }

    Ok((contributor, dataset, feed_infos))
}
