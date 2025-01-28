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

use modules::downloadable_model::{NTFSDownloader, PythonDownloadableModel};
use modules::python_transit_model::PythonTransitModel;
use pyo3::prelude::*;
use transit_model::downloadable_model::{ModelConfig, NavitiaConfig};
use transit_model::objects::StopTime;

#[pyclass]
pub struct PStopTime(StopTime);

#[pymethods]
impl PStopTime {
    #[getter]
    fn get_stop_time(&self) -> PyResult<PyObject> {
        Python::with_gil(|py| {
            let stop_time = &self.0;
            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("stop_point_idx", stop_time.stop_point_idx.get().to_string())?;
            dict.set_item("sequence", stop_time.sequence)?;
            dict.set_item(
                "arrival_time",
                stop_time
                    .arrival_time
                    .as_ref()
                    .map_or("None".to_string(), |t| t.total_seconds().to_string()),
            )?;
            dict.set_item(
                "departure_time",
                stop_time
                    .departure_time
                    .as_ref()
                    .map_or("None".to_string(), |t| t.total_seconds().to_string()),
            )?;
            dict.set_item(
                "start_pickup_drop_off_window",
                stop_time
                    .start_pickup_drop_off_window
                    .as_ref()
                    .map_or("None".to_string(), |t| t.to_string()),
            )?;
            dict.set_item(
                "end_pickup_drop_off_window",
                stop_time
                    .end_pickup_drop_off_window
                    .as_ref()
                    .map_or("None".to_string(), |t| t.to_string()),
            )?;
            dict.set_item("boarding_duration", stop_time.boarding_duration.to_string())?;
            dict.set_item(
                "alighting_duration",
                stop_time.alighting_duration.to_string(),
            )?;
            dict.set_item("pickup_type", stop_time.pickup_type.to_string())?;
            dict.set_item("drop_off_type", stop_time.drop_off_type.to_string())?;
            dict.set_item(
                "local_zone_id",
                stop_time
                    .local_zone_id
                    .as_ref()
                    .map_or("None".to_string(), |t| t.to_string()),
            )?;
            dict.set_item(
                "precision",
                stop_time.precision.as_ref().map_or(
                    "None".to_string(),
                    |t: &transit_model::objects::StopTimePrecision| t.clone().to_string(),
                ),
            )?;
            Ok(dict.into()) // Return the Python object
        })
    }
}

#[pyclass]
#[derive(Clone)]
pub struct PythonModelConfig(ModelConfig);

#[pymethods]
impl PythonModelConfig {
    #[new]
    fn new(check_interval_secs: u64, path: String) -> Self {
        let model = ModelConfig {
            check_interval_secs,
            path,
        };
        PythonModelConfig(model)
    }
}

impl From<PythonModelConfig> for ModelConfig {
    fn from(config: PythonModelConfig) -> Self {
        config.0
    }
}

impl From<ModelConfig> for PythonModelConfig {
    fn from(config: ModelConfig) -> Self {
        PythonModelConfig(config)
    }
}

#[pyclass]
#[derive(Clone)]
pub struct PythonNavitiaConfig(NavitiaConfig);

impl From<PythonNavitiaConfig> for NavitiaConfig {
    fn from(config: PythonNavitiaConfig) -> Self {
        config.0
    }
}

#[pymethods]
impl PythonNavitiaConfig {
    #[new]
    fn new(navitia_url: String, coverage: String, navitia_token: String) -> Self {
        let navitia_config = NavitiaConfig {
            navitia_url,
            coverage,
            navitia_token,
        };
        PythonNavitiaConfig(navitia_config)
    }
}

#[pymodule]
fn transit_model_python(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PythonTransitModel>()?;
    m.add_class::<NTFSDownloader>()?;
    m.add_class::<PythonDownloadableModel>()?;
    m.add_class::<PythonNavitiaConfig>()?;
    m.add_class::<PythonModelConfig>()?;
    Ok(())
}
