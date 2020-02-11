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

use crate::{objects::Line, Model, Result};
use minidom::Element;
use transit_model_collection::Idx;

pub struct OfferExporter<'a> {
    _model: &'a Model,
}

// Publicly exposed methods
impl<'a> OfferExporter<'a> {
    pub fn new(_model: &'a Model) -> Self {
        OfferExporter { _model }
    }
    pub fn export(&self, _line_idx: Idx<Line>) -> Result<Vec<Element>> {
        Ok(Vec::new())
    }
}

// Internal methods
impl<'a> OfferExporter<'a> {}
