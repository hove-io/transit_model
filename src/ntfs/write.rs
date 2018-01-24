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

use std::collections::HashMap;
use std::path;
use csv;
use collection::{CollectionWithId, Id};
use serde;

pub fn write_feed_infos(path: &path::Path, feed_infos: &HashMap<String, String>) {
    info!("Writing feed_infos.txt");
    let mut wtr = csv::Writer::from_path(&path.join("feed_infos.txt")).unwrap();
    wtr.write_record(&["feed_info_param", "feed_info_value"])
        .unwrap();
    for feed_info in feed_infos {
        wtr.serialize(feed_info).unwrap();
    }
    wtr.flush().unwrap();
}

pub fn write_collection_with_id<T>(path: &path::Path, file: &str, collection: &CollectionWithId<T>)
where
    T: Id<T>,
    T: serde::Serialize,
{
    info!("Writing {}", file);
    let mut wtr = csv::Writer::from_path(&path.join(file)).unwrap();
    for (_, obj) in collection.iter() {
        wtr.serialize(obj).unwrap();
    }
    wtr.flush().unwrap();
}
