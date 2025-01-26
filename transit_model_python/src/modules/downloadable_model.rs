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
//! PyO3 bindings for transit model components to enable Python interoperability

use pyo3::{exceptions::PyValueError, prelude::*};
use std::{error::Error, future::Future, pin::Pin};
use tokio::runtime::Runtime;
use transit_model::{
    downloadable_model::{DownloadableTransitModel, Downloader, ModelConfig, NavitiaConfig},
    objects::StopTime,
};

/// Python wrapper for a downloader component implementing the Downloader trait
///
/// Acts as a bridge between Rust's Downloader trait and Python implementations,
/// allowing Python classes to provide download functionality to Rust code
#[pyclass(subclass)]
pub struct PyDownloader {
    /// The underlying Python object implementing the download logic
    inner: Py<PyAny>,
}

impl Clone for PyDownloader {
    /// Creates a new reference to the same Python downloader object
    ///
    /// Uses Python's GIL to safely clone the Python object reference
    fn clone(&self) -> Self {
        Python::with_gil(|py| PyDownloader {
            inner: self.inner.clone_ref(py),
        })
    }
}

#[pymethods]
impl PyDownloader {
    /// Creates a new PyDownloader wrapping a Python object
    ///
    /// # Arguments
    /// * `obj` - Python object implementing the `run_download` method
    #[new]
    fn new(obj: Py<PyAny>) -> Self {
        PyDownloader { inner: obj }
    }
}

impl Downloader for PyDownloader {
    /// Executes the download operation by calling into Python implementation
    ///
    /// # Arguments
    /// * `config` - Model configuration parameters
    /// * `version` - Target version identifier for download
    ///
    /// # Returns
    /// Future resolving to local path of downloaded model or error
    fn run_download(
        &self,
        config: &ModelConfig,
        version: &str,
    ) -> Pin<Box<dyn Future<Output = Result<String, Box<dyn Error + Send + Sync>>> + Send>> {
        let model = Python::with_gil(|py| self.inner.clone_ref(py));
        let version = version.to_string();
        let config = config.clone();

        Box::pin(async move {
            Python::with_gil(|py| {
                model
                    .bind(py)
                    .call_method("run_download", (config, version), None)
                    .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)
                    .and_then(|result| {
                        result
                            .extract()
                            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)
                    })
            })
        })
    }
}

/// Python-exposed interface for interacting with downloadable transit models
///
/// Provides thread-safe access to transit model data with async-aware locking
#[pyclass]
pub struct PythonDownloadableModel {
    /// The underlying Rust implementation of the downloadable transit model
    model: DownloadableTransitModel<PyDownloader>,
}

#[pymethods]
impl PythonDownloadableModel {
    /// Initializes a new downloadable transit model instance
    ///
    /// # Arguments
    /// * `navitia_config` - Configuration for Navitia integration
    /// * `model_config` - General model configuration parameters
    /// * `downloader` - Downloader component implementing the download logic
    ///
    /// # Errors
    /// Returns `PyValueError` if initialization fails at any stage
    #[new]
    pub fn new(
        navitia_config: NavitiaConfig,
        model_config: ModelConfig,
        downloader: Py<PyDownloader>,
    ) -> PyResult<Self> {
        let rt = Runtime::new()
            .map_err(|e| PyValueError::new_err(format!("Failed to create runtime: {}", e)))?;

        let model = rt
            .block_on(async {
                let rust_downloader = Python::with_gil(|py| match downloader.bind(py).extract() {
                    Ok(downloader) => Ok(downloader),
                    Err(e) => Err(PyValueError::new_err(format!(
                        "Failed to extract downloader: {}",
                        e
                    ))),
                })?;

                DownloadableTransitModel::new(navitia_config, model_config, rust_downloader).await
            })
            .map_err(|e| PyValueError::new_err(format!("Failed to create model: {}", e)))?;

        Ok(Self { model })
    }

    /// Retrieves transit lines associated with a given stop identifier
    ///
    /// # Arguments
    /// * `idx` - Unique identifier for the stop
    ///
    /// # Returns
    /// List of line names servicing the specified stop
    pub fn get_lines(&self, idx: String) -> PyResult<Vec<String>> {
        let rt = Runtime::new()
            .map_err(|e| PyValueError::new_err(format!("Failed to create runtime: {}", e)))?;

        let model = &self.model;

        let lines = rt.block_on(async {
            let guard = model.current_model.read().await;
            guard
                .lines
                .get_idx(&idx)
                .iter()
                .map(|index| guard.lines[*index].name.clone())
                .collect::<Vec<String>>()
        });

        Ok(lines)
    }

    /// Retrieves contributors associated with a given stop identifier
    ///
    /// # Arguments
    /// * `idx` - Unique identifier for the stop
    ///
    /// # Returns
    /// List of contributor names providing data for the specified stop
    pub fn get_contributors(&self, idx: String) -> PyResult<Vec<String>> {
        let rt = Runtime::new()
            .map_err(|e| PyValueError::new_err(format!("Failed to create runtime: {}", e)))?;

        let model = &self.model;

        let contributors = rt.block_on(async {
            let guard = model.current_model.read().await;
            guard
                .contributors
                .get_idx(&idx)
                .iter()
                .map(|index| guard.contributors[*index].name.clone())
                .collect::<Vec<String>>()
        });

        Ok(contributors)
    }

    /// Retrieves networks associated with a given stop identifier
    ///
    /// # Arguments
    /// * `idx` - Unique identifier for the stop
    ///
    /// # Returns
    /// List of network names the specified stop belongs to
    pub fn get_networks(&self, idx: String) -> PyResult<Vec<String>> {
        let rt = Runtime::new()
            .map_err(|e| PyValueError::new_err(format!("Failed to create runtime: {}", e)))?;

        let model = &self.model;

        let networks = rt.block_on(async {
            let guard = model.current_model.read().await;
            guard
                .networks
                .get_idx(&idx)
                .iter()
                .map(|index| guard.networks[*index].name.clone())
                .collect::<Vec<String>>()
        });

        Ok(networks)
    }

    /// Retrieves the name of a stop area by its identifier
    ///
    /// # Arguments
    /// * `idx` - Unique identifier for the stop area
    ///
    /// # Returns
    /// Name of the specified stop area
    pub fn get_stop_area_by_id(&self, idx: String) -> PyResult<String> {
        let rt = Runtime::new()
            .map_err(|e| PyValueError::new_err(format!("Failed to create runtime: {}", e)))?;

        let model = &self.model;

        let stop_area = rt.block_on(async {
            let guard = model.current_model.read().await;
            guard
                .stop_areas
                .get_idx(&idx)
                .iter()
                .map(|index| guard.stop_areas[*index].name.clone())
                .collect::<String>()
        });

        Ok(stop_area)
    }

    /// Retrieves a vehicle journey by its identifier
    ///
    /// # Arguments
    /// * `idx` - Unique identifier for the vehicle journey
    ///
    /// # Returns
    /// ID of the specified vehicle journey
    pub fn get_vehicle_journey_by_id(&self, idx: String) -> PyResult<String> {
        let rt = Runtime::new()
            .map_err(|e| PyValueError::new_err(format!("Failed to create runtime: {}", e)))?;

        let model = &self.model;

        let vehicle_journey = rt.block_on(async {
            let guard = model.current_model.read().await;
            guard
                .vehicle_journeys
                .get_idx(&idx)
                .iter()
                .map(|index| guard.vehicle_journeys[*index].id.clone())
                .collect::<String>()
        });

        Ok(vehicle_journey)
    }

    /// Retrieves stop times for a specific vehicle journey
    ///
    /// # Arguments
    /// * `idx` - Unique identifier for the vehicle journey
    ///
    /// # Returns
    /// List of stop times associated with the specified vehicle journey
    pub fn get_vehicle_journey_stop_times(&self, idx: String) -> PyResult<Vec<StopTime>> {
        let rt = Runtime::new()
            .map_err(|e| PyValueError::new_err(format!("Failed to create runtime: {}", e)))?;

        let model = &self.model;

        let vehicle_journey_stop_times = rt.block_on(async {
            let guard = model.current_model.read().await;
            guard
                .vehicle_journeys
                .get_idx(&idx)
                .iter()
                .flat_map(|index| guard.vehicle_journeys[*index].stop_times.iter().cloned())
                .collect::<Vec<StopTime>>()
        });

        Ok(vehicle_journey_stop_times)
    }
}
