use crate::{
    model::{self, Collections},
    objects::PhysicalMode,
};
use typed_index_collection::CollectionWithId;

lazy_static::lazy_static! {
    static ref CO2_EMISSIONS: std::collections::HashMap<&'static str, f32> = {
        let mut modes_map = std::collections::HashMap::new();
        modes_map.insert(model::AIR_PHYSICAL_MODE, 144.6f32);
        modes_map.insert(model::BIKE_PHYSICAL_MODE, 0f32);
        modes_map.insert(model::BIKE_SHARING_SERVICE_PHYSICAL_MODE, 0f32);
        // Unknown value
        // modes_map.insert(model::"Boat", 0.0f32);
        modes_map.insert(model::BUS_PHYSICAL_MODE, 132f32);
        modes_map.insert(model::BUS_RAPID_TRANSIT_PHYSICAL_MODE, 84f32);
        modes_map.insert(model::CAR_PHYSICAL_MODE, 184f32);
        modes_map.insert(model::COACH_PHYSICAL_MODE, 171f32);
        modes_map.insert(model::FERRY_PHYSICAL_MODE, 279f32);
        modes_map.insert(model::FUNICULAR_PHYSICAL_MODE, 3f32);
        modes_map.insert(model::LOCAL_TRAIN_PHYSICAL_MODE, 30.7f32);
        modes_map.insert(model::LONG_DISTANCE_TRAIN_PHYSICAL_MODE, 3.4f32);
        modes_map.insert(model::METRO_PHYSICAL_MODE, 3f32);
        modes_map.insert(model::RAPID_TRANSIT_PHYSICAL_MODE, 6.2f32);
        // Unknown value
        // modes_map.insert(model::RailShuttle, 0.0f32);
        // Unknown value
        // modes_map.insert(model::Shuttle, 0.0f32);
        // Unknown value
        // modes_map.insert(model::SuspendedCableCar, 0.0f32);
        modes_map.insert(model::TAXI_PHYSICAL_MODE, 184f32);
        modes_map.insert(model::TRAIN_PHYSICAL_MODE, 11.9f32);
        modes_map.insert(model::TRAMWAY_PHYSICAL_MODE, 4f32);
        modes_map
    };
}

/// Physical mode should contains CO2 emissions. If the values are not present
/// in the NTFS, some default values will be used.
pub fn fill_co2(collections: &mut Collections) {
    let mut physical_modes = collections.physical_modes.take();
    for physical_mode in &mut physical_modes {
        if physical_mode.co2_emission.is_none() {
            physical_mode.co2_emission = CO2_EMISSIONS.get(physical_mode.id.as_str()).copied();
        }
    }
    collections.physical_modes = CollectionWithId::new(physical_modes).unwrap();
    // Add fallback modes
    for &fallback_mode in &[
        model::BIKE_PHYSICAL_MODE,
        model::BIKE_SHARING_SERVICE_PHYSICAL_MODE,
        model::CAR_PHYSICAL_MODE,
    ] {
        if !collections.physical_modes.contains_id(fallback_mode) {
            // Can unwrap because we first check that the ID doesn't exist
            collections
                .physical_modes
                .push(PhysicalMode {
                    id: fallback_mode.to_string(),
                    name: fallback_mode.to_string(),
                    co2_emission: CO2_EMISSIONS.get(fallback_mode).copied(),
                })
                .unwrap();
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use approx::assert_relative_eq;
    use pretty_assertions::assert_eq;

    #[test]
    fn enhance_with_default() {
        let mut collections = Collections::default();
        collections
            .physical_modes
            .push(PhysicalMode {
                id: String::from(model::BUS_PHYSICAL_MODE),
                name: String::from("Bus"),
                ..Default::default()
            })
            .unwrap();
        fill_co2(&mut collections);

        let bus_mode = collections
            .physical_modes
            .get(model::BUS_PHYSICAL_MODE)
            .unwrap();
        assert_relative_eq!(bus_mode.co2_emission.unwrap(), 132f32);
    }

    #[test]
    fn preserve_existing() {
        let mut collections = Collections::default();
        collections
            .physical_modes
            .push(PhysicalMode {
                id: String::from(model::BUS_PHYSICAL_MODE),
                name: String::from("Bus"),
                co2_emission: Some(42.0f32),
            })
            .unwrap();
        fill_co2(&mut collections);

        let bus_mode = collections
            .physical_modes
            .get(model::BUS_PHYSICAL_MODE)
            .unwrap();
        assert_relative_eq!(bus_mode.co2_emission.unwrap(), 42.0f32);
    }

    #[test]
    fn add_fallback_modes() {
        let mut collections = Collections::default();
        fill_co2(&mut collections);

        assert_eq!(3, collections.physical_modes.len());
        let bike_mode = collections
            .physical_modes
            .get(model::BIKE_PHYSICAL_MODE)
            .unwrap();
        assert_relative_eq!(bike_mode.co2_emission.unwrap(), 0.0f32);
        let walk_mode = collections
            .physical_modes
            .get(model::BIKE_SHARING_SERVICE_PHYSICAL_MODE)
            .unwrap();
        assert_relative_eq!(walk_mode.co2_emission.unwrap(), 0.0f32);
        let car_mode = collections
            .physical_modes
            .get(model::CAR_PHYSICAL_MODE)
            .unwrap();
        assert_relative_eq!(car_mode.co2_emission.unwrap(), 184.0f32);
    }
}
