// Copyright (C) 2024 Hove and/or its affiliates.
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

//! PyO3 bindings for transit model data structures and operations
//!
//! Provides Python interfaces to access and manipulate transit model data
//! from NTFS (NeTEx France Standard) datasets.

pub mod modules;

use modules::downloadable_model::{PyDownloader, PythonDownloadableModel};
use modules::python_transit_model::PythonTransitModel;
use pyo3::prelude::*;
use transit_model::downloadable_model::{ModelConfig, NavitiaConfig};

#[pymodule]
fn transit_model_python(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PythonTransitModel>()?;
    m.add_class::<PyDownloader>()?;
    m.add_class::<PythonDownloadableModel>()?;
    m.add_class::<NavitiaConfig>()?;
    m.add_class::<ModelConfig>()?;
    Ok(())
}
