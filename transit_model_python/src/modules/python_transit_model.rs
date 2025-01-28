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
//!

use pyo3::{exceptions::PyValueError, prelude::*};
use std::sync::Arc;
use transit_model as intern_transit_model;
use transit_model::model::Model;
use typed_index_collection::Id;

use crate::PStopTime;

/// Thread-safe wrapper for transit model data with Python bindings
///
/// Provides read-only access to transit model components through
/// atomic reference counting for safe concurrent access.
#[pyclass]
pub struct PythonTransitModel {
    /// Shared reference to the underlying transit model data
    model: Arc<Model>,
}

#[pymethods]
impl PythonTransitModel {
    /// Creates a new transit model instance from NTFS data
    ///
    /// # Arguments
    /// * `path` - Path to the NTFS dataset directory
    ///
    /// # Panics
    /// Will panic if the NTFS data cannot be read from the specified path
    ///
    /// # Example
    /// ```python
    /// model = PythonTransitModel("/path/to/ntfs/data")
    /// ```
    #[new]
    pub fn new(path: &str) -> Self {
        let transit_objects =
            intern_transit_model::ntfs::read(path).expect("Failed to read transit objects");
        Self {
            model: Arc::new(transit_objects),
        }
    }

    /// Retrieves line names by line identifier
    ///
    /// # Arguments
    /// * `idx` - Unique identifier for the transit line
    ///
    /// # Returns
    /// Vector of line names matching the identifier (empty if not found)
    ///
    /// # Example
    /// ```python
    /// line_names = model.get_lines("line:123")
    /// ```
    pub fn get_lines(&self, idx: String) -> PyResult<Vec<String>> {
        Ok(self
            .model
            .lines
            .get_idx(&idx)
            .iter()
            .map(|idx| self.model.lines[*idx].name.to_string())
            .collect())
    }

    /// Retrieves contributor names by contributor identifier
    ///
    /// # Arguments
    /// * `idx` - Unique identifier for the contributor
    ///
    /// # Returns
    /// Vector of contributor names matching the identifier (empty if not found)
    pub fn get_contributors(&self, idx: String) -> PyResult<Vec<String>> {
        Ok(self
            .model
            .contributors
            .get_idx(&idx)
            .iter()
            .map(|idx| self.model.contributors[*idx].name.to_string())
            .collect())
    }

    /// Retrieves network identifiers by network identifier
    ///
    /// # Arguments
    /// * `idx` - Unique identifier for the network
    ///
    /// # Returns
    /// Vector containing the network ID if found (empty if not found)
    ///
    /// # Note
    /// Returns the same identifier provided as input when exists in the model
    pub fn get_networks(&self, idx: String) -> PyResult<Vec<String>> {
        Ok(self
            .model
            .networks
            .get_idx(&idx)
            .iter()
            .map(|idx| self.model.networks[*idx].id().to_string())
            .collect())
    }

    /// Retrieves stop area name by stop area identifier
    ///
    /// # Arguments
    /// * `idx` - Unique identifier for the stop area
    ///
    /// # Returns
    /// Concatenated string of stop area names (empty if not found)
    pub fn get_stop_area_by_id(&self, idx: String) -> PyResult<String> {
        Ok(self
            .model
            .stop_areas
            .get_idx(&idx)
            .iter()
            .map(|idx| self.model.stop_areas[*idx].name.clone())
            .collect())
    }

    /// Retrieves vehicle journey identifier by journey ID
    ///
    /// # Arguments
    /// * `idx` - Unique identifier for the vehicle journey
    ///
    /// # Returns
    /// Concatenated string of journey IDs (empty if not found)
    pub fn get_vehicule_journey_by_id(&self, idx: String) -> PyResult<String> {
        Ok(self
            .model
            .vehicle_journeys
            .get_idx(&idx)
            .iter()
            .map(|idx| self.model.vehicle_journeys[*idx].id.as_str())
            .collect())
    }

    /// Retrieves all stop times for a specific vehicle journey
    ///
    /// # Arguments
    /// * `idx` - Unique identifier for the vehicle journey
    ///
    /// # Returns
    /// Vector of StopTime objects for the specified journey (empty if not found)
    pub fn get_vehicule_journey_stop_times(&self, idx: String) -> PyResult<Vec<PStopTime>> {
        Ok(self
            .model
            .vehicle_journeys
            .get_idx(&idx)
            .iter()
            .flat_map(|idx| self.model.vehicle_journeys[*idx].stop_times.iter().cloned())
            .map(PStopTime)
            .collect())
    }

    /// Filters stop times for a specific vehicle journey and stop point
    ///
    /// # Arguments
    /// * `vehicule_id` - Vehicle journey identifier
    /// * `stop_id` - Stop point identifier
    ///
    /// # Returns
    /// Vector of matching StopTime objects
    ///
    /// # Errors
    /// Returns PyValueError if either the stop point or vehicle journey is not found
    pub fn get_vehicule_journey_stop_times_by_stop_id(
        &self,
        vehicule_id: String,
        stop_id: String,
    ) -> PyResult<Vec<PStopTime>> {
        let stop_point_idx = self
            .model
            .stop_points
            .get_idx(&stop_id)
            .ok_or_else(|| PyValueError::new_err("StopPoint not found"))?;

        let stop_times: Vec<PStopTime> = self
            .model
            .vehicle_journeys
            .get_idx(&vehicule_id)
            .into_iter()
            .flat_map(|idx| &self.model.vehicle_journeys[idx].stop_times)
            .filter(|st| st.stop_point_idx == stop_point_idx)
            .cloned()
            .map(PStopTime)
            .collect::<Vec<PStopTime>>();

        if stop_times.is_empty() {
            Err(PyValueError::new_err("StopTime not found"))
        } else {
            Ok(stop_times)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_lines() {
        let transit_objects = intern_transit_model::ntfs::read("../tests/fixtures/minimal_ntfs/")
            .expect("Failed to read transit objects");
        let python_transit_model = PythonTransitModel {
            model: Arc::new(transit_objects),
        };
        let lines = python_transit_model.get_lines("M1".to_string()).unwrap();
        assert_eq!(lines, vec!["Metro 1".to_string(),]);
    }

    #[test]
    fn test_get_contributors() {
        let transit_objects = intern_transit_model::ntfs::read("../tests/fixtures/minimal_ntfs/")
            .expect("Failed to read transit objects");
        let python_transit_model = PythonTransitModel {
            model: Arc::new(transit_objects),
        };
        let contributors = python_transit_model
            .get_contributors("TGC".to_string())
            .unwrap();
        assert_eq!(contributors, vec!["The Great Contributor".to_string(),]);
    }

    #[test]
    fn test_get_networks() {
        let transit_objects = intern_transit_model::ntfs::read("../tests/fixtures/minimal_ntfs/")
            .expect("Failed to read transit objects");
        let python_transit_model = PythonTransitModel {
            model: Arc::new(transit_objects),
        };
        let networks: Vec<String> = python_transit_model
            .get_networks("TGN".to_string())
            .unwrap();
        assert_eq!(networks, vec!["TGN".to_string(),]);
    }

    #[test]
    fn test_get_stop_area_by_id() {
        let transit_objects = intern_transit_model::ntfs::read("../tests/fixtures/minimal_ntfs/")
            .expect("Failed to read transit objects");
        let python_transit_model = PythonTransitModel {
            model: Arc::new(transit_objects),
        };
        let stop_area = python_transit_model
            .get_stop_area_by_id("GDL".to_string())
            .unwrap();
        assert_eq!(stop_area, "Gare de Lyon".to_string());
    }
}
