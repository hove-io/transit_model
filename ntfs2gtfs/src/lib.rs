// Copyright 2017 Hove and/or its affiliates.
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

use transit_model::{Model, Result};

pub fn add_mode_to_line_code(model: Model) -> Result<Model> {
    let mut collections = model.into_collections();
    for idx in collections.lines.indexes() {
        let mut line = collections.lines.index_mut(idx);
        let mode = collections.commercial_modes.get(&line.commercial_mode_id);
        let code = match (line.code.clone(), mode) {
            (Some(c), Some(m)) => Some(m.name.clone() + " " + &c),
            (Some(c), None) => Some(c),
            (None, Some(m)) => Some(m.name.clone()),
            _ => None,
        };
        line.code = code;
    }

    Model::new(collections)
}
