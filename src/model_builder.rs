#![allow(missing_docs)]
use crate::collection::Idx;
use crate::model::{Collections, Model};
use crate::objects::{StopArea, StopPoint, StopTime, Time, VehicleJourney};

#[derive(Default)]
pub struct ModelBuilder {
    collections: Collections,
}

pub struct VehicleJourneyBuilder<'a> {
    model: &'a mut ModelBuilder,
    vj_idx: Idx<VehicleJourney>,
}

impl<'a> ModelBuilder {
    pub fn vj<F>(mut self, name: &str, mut vj_initer: F) -> Self
    where
        F: FnMut(VehicleJourneyBuilder),
    {
        let mut new_vj = VehicleJourney::default();
        new_vj.id = name.into();
        let vj_idx = self
            .collections
            .vehicle_journeys
            .push(new_vj)
            .expect(&format!("vj {} already exists", name));
        let vj_builder = VehicleJourneyBuilder {
            model: &mut self,
            vj_idx,
        };

        vj_initer(vj_builder);
        self
    }

    pub fn build(self) -> Model {
        Model::new(self.collections).unwrap()
    }
}

impl<'a> VehicleJourneyBuilder<'a> {
    pub fn find_or_create_sp(&mut self, sp_name: &str) -> Idx<StopPoint> {
        //TODO the find part
        let sa_id = format!("sa:{}", sp_name);
        let sa = StopArea {
            id: sa_id.clone(),
            name: sp_name.to_owned(),
            ..Default::default()
        };
        self.model
            .collections
            .stop_areas
            .push(sa)
            .expect(&format!("stoparea {} already exists", sp_name));

        let mut new_sp = StopPoint {
            id: sp_name.to_owned(),
            name: sp_name.to_owned(),
            stop_area_id: sa_id,
            ..Default::default()
        };

        new_sp.id = sp_name.into();
        self.model
            .collections
            .stop_points
            .push(new_sp)
            .expect(&format!("stoppoint {} already exists", sp_name))
    }

    pub fn st(mut self, name: &str, arrival: impl Into<Time>, departure: impl Into<Time>) -> Self {
        let stop_point_idx = self.find_or_create_sp(name);
        {
            let vj = &mut self
                .model
                .collections
                .vehicle_journeys
                .index_mut(self.vj_idx);
            let sequence = vj.stop_times.len() as u32;
            let stop_time = StopTime {
                stop_point_idx,
                sequence,
                arrival_time: arrival.into(),
                departure_time: departure.into(),
                boarding_duration: 0u16,
                alighting_duration: 0u16,
                pickup_type: 0u8,
                drop_off_type: 0u8,
                datetime_estimated: false,
                local_zone_id: None,
            };

            vj.stop_times.push(stop_time);
        }

        self
    }
}

#[cfg(test)]
mod test {
    use crate::model_builder::ModelBuilder;

    #[test]
    fn simple_model_creation() {
        let _: crate::Model = ModelBuilder::default()
            .vj("toto", |vj_builder| {
                vj_builder
                    .st("A", "10:00:00", "10:01:00")
                    .st("B", "11:00:00", "11:01:00");
            })
            // .vj(...)
            .build();

        // let vj = model.collection.stop_areas.get()
    }
}
