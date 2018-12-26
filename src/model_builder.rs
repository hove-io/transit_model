#![allow(missing_docs)]
use crate::collection::{CollectionWithId, Id, Idx, RefMut};
use crate::model::{Collections, Model};
use crate::objects::{
    CommercialMode, Line, PhysicalMode, Route, StopPoint, StopTime, Time, VehicleJourney,
};

#[derive(Default)]
pub struct ModelBuilder {
    collections: Collections,
}

pub struct VehicleJourneyBuilder<'a> {
    model: &'a mut ModelBuilder,
    vj_idx: Idx<VehicleJourney>,
}

trait WithId {
    fn with_id(id: &str) -> Self;
}

impl WithId for Route {
    fn with_id(id: &str) -> Self {
        let mut r = Self::default();
        r.id = id.to_owned();
        r
    }
}

impl WithId for Line {
    fn with_id(id: &str) -> Self {
        let mut r = Self::default();
        r.id = id.to_owned();
        r
    }
}
impl WithId for CommercialMode {
    fn with_id(id: &str) -> Self {
        let mut r = Self::default();
        r.id = id.to_owned();
        r
    }
}
impl WithId for PhysicalMode {
    fn with_id(id: &str) -> Self {
        let mut r = Self::default();
        r.id = id.to_owned();
        r
    }
}
impl WithId for crate::objects::Dataset {
    fn with_id(id: &str) -> Self {
        let mut r = Self::default();
        r.id = id.to_owned();
        r
    }
}
impl WithId for crate::objects::Contributor {
    fn with_id(id: &str) -> Self {
        let mut r = Self::default();
        r.id = id.to_owned();
        r
    }
}
impl WithId for crate::objects::Network {
    fn with_id(id: &str) -> Self {
        let mut r = Self::default();
        r.id = id.to_owned();
        r
    }
}
impl WithId for crate::objects::StopArea {
    fn with_id(id: &str) -> Self {
        let mut r = Self::default();
        r.id = id.to_owned();
        r
    }
}
impl WithId for crate::objects::StopPoint {
    fn with_id(id: &str) -> Self {
        let mut r = Self::default();
        r.id = id.to_owned();
        r
    }
}
impl WithId for crate::objects::Company {
    fn with_id(id: &str) -> Self {
        let mut r = Self::default();
        r.id = id.to_owned();
        r
    }
}

fn get_or_create<'a, T: Id<T> + WithId>(
    col: &'a mut CollectionWithId<T>,
    id: &str,
) -> RefMut<'a, T> {
    let elt = col.get_idx(id).unwrap_or_else(|| {
        println!("creating new {}", id);
        col.push(T::with_id(id)).unwrap()
    });
    col.index_mut(elt)
}

fn get_or_create_with<'a, T: Id<T> + WithId, F>(
    col: &'a mut CollectionWithId<T>,
    id: &str,
    mut f: F,
) -> RefMut<'a, T>
where
    F: FnMut(&mut T),
{
    let elt = col.get_idx(id).unwrap_or_else(|| {
        println!("creating new {}", id);
        let mut o = T::with_id(id);
        f(&mut o);
        col.push(o).unwrap()
    });
    col.index_mut(elt)
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
        self.add_missing_parts(vj_idx);
        self
    }

    // add the missing objects to the model (routes, lines, ...)
    fn add_missing_parts(&mut self, vj_idx: Idx<VehicleJourney>) {
        let new_vj = self.collections.vehicle_journeys.index_mut(vj_idx);
        let route = get_or_create(&mut self.collections.routes, &new_vj.route_id);
        let dataset = get_or_create(&mut self.collections.datasets, &new_vj.dataset_id);
        get_or_create(&mut self.collections.contributors, &dataset.contributor_id);
        get_or_create(&mut self.collections.companies, &new_vj.company_id);
        let line = get_or_create(&mut self.collections.lines, &route.line_id);
        get_or_create(
            &mut self.collections.commercial_modes,
            &line.commercial_mode_id,
        );
        get_or_create(&mut self.collections.networks, &line.network_id);
        get_or_create(
            &mut self.collections.physical_modes,
            &new_vj.physical_mode_id,
        );
    }

    pub fn build(self) -> Model {
        Model::new(self.collections).unwrap()
    }
}

impl<'a> VehicleJourneyBuilder<'a> {
    pub fn find_or_create_sp(&mut self, sp: &str) -> Idx<StopPoint> {
        match self.model.collections.stop_points.get_idx(sp) {
            Some(e) => e,
            None => {
                let sa_id = format!("sa:{}", sp);
                let new_sp = StopPoint {
                    id: sp.to_owned(),
                    name: sp.to_owned(),
                    stop_area_id: sa_id.clone(),
                    ..Default::default()
                };

                get_or_create_with(&mut self.model.collections.stop_areas, &sa_id, |mut sa| {
                    sa.name = format!("sa {}", sp)
                });

                self.model
                    .collections
                    .stop_points
                    .push(new_sp)
                    .expect(&format!("stoppoint {} already exists", sp))
            }
        }
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
        let model = ModelBuilder::default()
            .vj("toto", |vj_builder| {
                vj_builder
                    .st("A", "10:00:00", "10:01:00")
                    .st("B", "11:00:00", "11:01:00");
            })
            .vj("tata", |vj_builder| {
                vj_builder
                    .st("C", "10:00:00", "10:01:00")
                    .st("D", "11:00:00", "11:01:00");
            })
            .build();

        assert_eq!(
            model.get_corresponding_from_idx(model.vehicle_journeys.get_idx("toto").unwrap()),
            ["A", "B"]
                .into_iter()
                .map(|s| model.stop_points.get_idx(s).unwrap())
                .collect()
        );
        assert_eq!(
            model.get_corresponding_from_idx(model.vehicle_journeys.get_idx("tata").unwrap()),
            ["C", "D"]
                .into_iter()
                .map(|s| model.stop_points.get_idx(s).unwrap())
                .collect()
        );
    }

    #[test]
    fn same_sp_model_creation() {
        let model = ModelBuilder::default()
            .vj("toto", |vj| {
                vj.st("A", "10:00:00", "10:01:00")
                    .st("B", "11:00:00", "11:01:00");
            })
            .vj("tata", |vj| {
                vj.st("A", "10:00:00", "10:01:00")
                    .st("D", "11:00:00", "11:01:00");
            })
            .build();

        assert_eq!(
            model.get_corresponding_from_idx(model.vehicle_journeys.get_idx("toto").unwrap()),
            ["A", "B"]
                .into_iter()
                .map(|s| model.stop_points.get_idx(s).unwrap())
                .collect()
        );
        assert_eq!(
            model.get_corresponding_from_idx(model.stop_points.get_idx("A").unwrap()),
            ["toto", "tata"]
                .into_iter()
                .map(|s| model.vehicle_journeys.get_idx(s).unwrap())
                .collect()
        );

        assert_eq!(model.stop_points.len(), 3);
        assert_eq!(model.stop_areas.len(), 3);
    }
}
