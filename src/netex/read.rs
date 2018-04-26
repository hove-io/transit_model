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

use collection::{Collection, CollectionWithId, Id};
use model::Collections;
use objects::{self, Availability, CommentLinksT, Contributor, Coord, KeysValues, Time,
              TransportType};
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::path;
use std::result::Result as StdResult;
use utils::*;
use Result;
extern crate xmltree;
extern crate serde_json;


// TODO : a déplacer et mutualiser avec ce qui est fait dans le GTFS
#[derive(Deserialize, Debug)]
struct Dataset {
    dataset_id: String,
}

#[derive(Deserialize, Debug)]
struct Config {
    contributor: objects::Contributor,
    dataset: Dataset,
}

pub fn read_config<P: AsRef<path::Path>>(
    config_path: Option<P>,
) -> Result<(
    CollectionWithId<objects::Contributor>,
    CollectionWithId<objects::Dataset>,
)> {
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

    let contributors = CollectionWithId::new(vec![contributor])?;
    let datasets = CollectionWithId::new(vec![dataset])?;
    Ok((contributors, datasets))
}
// fin TODO : a déplacer et mutualiser avec ce qui est fait dans le GTFS

pub fn read_netex_file<P: AsRef<path::Path>>(
    collections: &mut Collections,
    path: P,
) {
    let netex_file = File::open(path).unwrap();
    let root_element = xmltree::Element::parse(netex_file).unwrap();
    let frame_element = root_element.get_child("dataObjects").unwrap().get_child("CompositeFrame").unwrap();
    read_composite_data_frame(&mut collections, &frame_element);
}

fn read_composite_data_frame(
    collections: &mut Collections,
    composite_frame: &xmltree::Element,
) {
    for frame in composite_frame.get_child("frames").unwrap().children {
        println!("{:?}", root_element.name);
    }
}