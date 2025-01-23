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

use pyo3::{exceptions::PyValueError, prelude::*};
use std::sync::Arc;
use transit_model::{model::Model, objects::StopTime};
use typed_index_collection::Id;
use transit_model as intern_transit_model;

#[pyclass]
pub struct PythonTransitModel {
    model: Arc<Model>,
}

#[pymethods]
impl PythonTransitModel {
    /// Create a new PythonTransitModel
    ///
    /// # Arguments
    /// * `path` - The path to the NTFS file
    ///
    /// # Returns
    /// * A new PythonTransitModel
    /// 
    #[new]
    pub fn new(path: &str) -> Self {
        let transit_objects =
            intern_transit_model::ntfs::read(path).expect("Failed to read transit objects");
        Self {
            model: Arc::new(transit_objects),
        }
    }

    /// Get the name of the stop
    /// 
    /// # Arguments
    /// 
    /// * `idx` - The index of the stop
    /// 
    /// # Returns
    /// 
    /// * The name of the line
    pub fn get_lines(&self, idx: String) -> PyResult<Vec<String>> {
        Ok(self
            .model
            .lines
            .get_idx(&idx)
            .iter()
            .map(|idx| self.model.lines[*idx].name.to_string())
            .collect())
    }

    /// Get the contributors providing the data for the stop
    /// 
    /// # Arguments
    /// 
    /// * `idx` - The index of the stop
    /// 
    /// # Returns
    /// 
    /// * The list of contributors providing the data for the stop
    pub fn get_contributors(&self, idx: String) -> PyResult<Vec<String>> {
        Ok(self
            .model
            .contributors
            .get_idx(&idx)
            .iter()
            .map(|idx| self.model.contributors[*idx].name.to_string())
            .collect())
    }

    /// Get the networks the stop belongs to
    /// 
    /// # Arguments
    /// 
    /// * `idx` - The index of the stop
    /// 
    /// # Returns
    /// 
    /// * The list of networks the stop belongs to
    pub fn get_networks(&self, idx: String) -> PyResult<Vec<String>> {
        Ok(self
            .model
            .networks
            .get_idx(&idx)
            .iter()
            .map(|idx| self.model.networks[*idx].id().to_string())
            .collect())
    }

    /// Get the name of the stop
    /// 
    /// # Arguments
    /// 
    /// * `idx` - The index of the stop
    /// 
    /// # Returns
    /// 
    /// * The name of the stop
    pub fn get_stop_area_by_id(&self, idx: String) -> PyResult<String> {
        Ok(self
            .model
            .stop_areas
            .get_idx(&idx)
            .iter()
            .map(|idx| self.model.stop_areas[*idx].name.clone())
            .collect())
    }

    /// Get the vehicule journey by id
    /// 
    /// # Arguments
    /// 
    /// * `idx` - The index of the vehicule journey
    /// 
    /// # Returns
    /// 
    /// * The vehicule journey id
    pub fn get_vehicule_journey_by_id(&self, idx: String) -> PyResult<String> {
        Ok(self
            .model
            .vehicle_journeys
            .get_idx(&idx)
            .iter()
            .map(|idx| self.model.vehicle_journeys[*idx].id.as_str())
            .collect())
    }


    /// Get the vehicule journey stop times
    /// 
    /// # Arguments
    /// 
    /// * `idx` - The index of the vehicule journey
    /// 
    /// # Returns
    /// 
    /// * The vehicule journey stop times
    pub fn get_vehicule_journey_stop_times(&self, idx: String) -> PyResult<Vec<StopTime>> {
        Ok(self
            .model
            .vehicle_journeys
            .get_idx(&idx)
            .iter()
            .flat_map(|idx| {
                self.model
                    .vehicle_journeys[*idx]
                    .stop_times
                    .iter()
                    .cloned()
            })
            .collect())
    }

    /// Get the vehicule journey stop times by vehicule journey id and stop id
    /// 
    /// # Arguments
    /// 
    /// * `vehicule_id` - The index of the vehicule journey
    /// * `stop_id` - The index of the stop
    /// 
    /// # Returns
    /// 
    /// * The vehicule journey stop times
    pub fn get_vehicule_journey_stop_times_by_stop_id(&self, vehicule_id: String, stop_id: String) -> PyResult<Vec<StopTime>> {
        let stop_point = match self.model.stop_points.get_idx(&stop_id) {
            Some(idx) => Some(idx),
            None => None,
        };
        if stop_point.is_none() {
            return Err(PyValueError::new_err("StopPoint not found"));
        }
        let stop_times: Vec<StopTime> = self
            .model
            .vehicle_journeys
            .get_idx(&vehicule_id)
            .iter()
            .flat_map(|idx| {
                self.model
                    .vehicle_journeys[*idx]
                    .stop_times
                    .iter()
                    .filter(|st| st.stop_point_idx == stop_point.unwrap())
                    .cloned()
            })
            .collect();
    
        if stop_times.is_empty() {
            Err(PyValueError::new_err("StopTime not found"))
        } else {
            Ok(stop_times)
        }
    }
}

#[pymodule]
fn transit_model_python(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PythonTransitModel>()?;
    Ok(())
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
