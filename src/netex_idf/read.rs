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

use super::{
    lines::{self, LineNetexIDF},
    offers, stops,
};
use crate::{
    model::{Collections, Model},
    objects::Dataset,
    AddPrefix, Result,
};
use chrono::naive::{MAX_DATE, MIN_DATE};
use log::{info, warn, Level as LogLevel};
use skip_error::skip_error_and_log;
use std::path::Path;
use transit_model_collection::CollectionWithId;
use walkdir::WalkDir;

const STOPS_FILENAME: &str = "arrets.xml";
const LINES_FILENAME: &str = "lignes.xml";
/// Read Netex IDF format into a Navitia Transit Model
pub fn read<P>(netex_idf_path: P, config_path: Option<P>, prefix: Option<String>) -> Result<Model>
where
    P: AsRef<Path>,
{
    fn init_dataset_validity_period(dataset: &mut Dataset) {
        dataset.start_date = MAX_DATE;
        dataset.end_date = MIN_DATE;
    }

    let mut collections = Collections::default();
    let (contributor, mut dataset, feed_infos) = crate::read_utils::read_config(config_path)?;
    collections.contributors = CollectionWithId::from(contributor);
    init_dataset_validity_period(&mut dataset);
    collections.datasets = CollectionWithId::from(dataset);
    collections.feed_infos = feed_infos;

    let path = netex_idf_path.as_ref();
    stops::from_path(&path.join(STOPS_FILENAME), &mut collections)?;
    let lines_netex_idf = lines::from_path(&path.join(LINES_FILENAME), &mut collections)?;
    for offer_folder in WalkDir::new(path)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|dir_entry| dir_entry.file_type().is_dir())
    {
        info!("Reading offer in folder {:?}", offer_folder.path());
        skip_error_and_log!(
            offers::read_offer_folder(offer_folder.path(), &mut collections, &lines_netex_idf),
            LogLevel::Warn
        );
    }
    enhance_with_line_comments(&mut collections, &lines_netex_idf);

    if let Some(prefix) = prefix {
        collections.add_prefix_with_sep(prefix.as_str(), ":");
    }

    collections.calendar_deduplication();
    Model::new(collections)
}

fn enhance_with_line_comments(
    collections: &mut Collections,
    lines_netex_idf: &CollectionWithId<LineNetexIDF>,
) {
    let mut lines = collections.lines.take();
    for line in &mut lines {
        if let Some(line_netex_idf) = lines_netex_idf.get(&line.id) {
            line.comment_links = line_netex_idf
                .comment_ids
                .iter()
                .filter_map(
                    |comment_id| match collections.comments.get_idx(comment_id) {
                        Some(comment_idx) => Some(comment_idx),
                        None => {
                            warn!("The comment with ID {} doesn't exist", comment_id);
                            None
                        }
                    },
                )
                .collect();
        }
    }
    collections.lines = CollectionWithId::new(lines).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::objects::{Comment, CommentType, Line};

    #[test]
    fn enhance_line_comments() {
        let mut collections = Collections::default();
        collections
            .lines
            .push(Line {
                id: String::from("line_id"),
                ..Default::default()
            })
            .unwrap();
        collections
            .comments
            .push(Comment {
                id: String::from("comment_id"),
                comment_type: CommentType::Information,
                label: None,
                name: String::from("Comment"),
                url: None,
            })
            .unwrap();
        let lines_netex_idf = CollectionWithId::from(LineNetexIDF {
            id: String::from("line_id"),
            name: String::from("Line Name"),
            code: None,
            private_code: None,
            network_id: String::from("network_id"),
            company_id: String::from("company_id"),
            mode: String::from("physical_mode_id"),
            color: None,
            text_color: None,
            comment_ids: vec![String::from("comment_id")].into_iter().collect(),
            trip_property_id: None,
        });
        enhance_with_line_comments(&mut collections, &lines_netex_idf);
        let line = collections.lines.get("line_id").unwrap();
        assert!(line
            .comment_links
            .contains(&collections.comments.get_idx("comment_id").unwrap()));
    }
}
