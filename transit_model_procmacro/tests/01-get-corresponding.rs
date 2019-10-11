use transit_model::objects::*;
use transit_model_collection::*;
use transit_model_procmacro::*;
use transit_model_relations::*;

#[derive(GetCorresponding)]
pub struct Model {
    lines_to_routes: OneToMany<Line, Route>,
    routes_to_vehicle_journeys: OneToMany<Route, VehicleJourney>,
}

fn main() {
    let line = Line {
        id: String::from("line_id"),
        name: String::from("Line name"),
        ..Default::default()
    };
    let route1 = Route {
        id: String::from("route_id_1"),
        name: String::from("Route Name 1"),
        line_id: String::from("line_id"),
        ..Default::default()
    };
    let route2 = Route {
        id: String::from("route_id_2"),
        name: String::from("Route Name 2"),
        line_id: String::from("line_id"),
        ..Default::default()
    };
    let vehicle_journey_1 = VehicleJourney {
        id: String::from("vehicle_journey_id_1"),
        route_id: String::from("route_id_1"),
        ..Default::default()
    };
    let vehicle_journey_2 = VehicleJourney {
        id: String::from("vehicle_journey_id_2"),
        route_id: String::from("route_id_1"),
        ..Default::default()
    };
    let vehicle_journey_3 = VehicleJourney {
        id: String::from("vehicle_journey_id_3"),
        route_id: String::from("route_id_2"),
        ..Default::default()
    };
    let vehicle_journey_4 = VehicleJourney {
        id: String::from("vehicle_journey_id_4"),
        route_id: String::from("route_id_2"),
        ..Default::default()
    };
    let lines = CollectionWithId::from(line);
    let routes = CollectionWithId::new(vec![route1, route2]).unwrap();
    let vehicle_journeys = CollectionWithId::new(vec![
        vehicle_journey_1,
        vehicle_journey_2,
        vehicle_journey_3,
        vehicle_journey_4,
    ])
    .unwrap();
    let model = Model {
        lines_to_routes: OneToMany::new(&lines, &routes, "lines_to_routes").unwrap(),
        routes_to_vehicle_journeys: OneToMany::new(
            &routes,
            &vehicle_journeys,
            "routes_to_vehicle_journeys",
        )
        .unwrap(),
    };

    let line_idx = lines.get_idx("line_id").unwrap();
    let vehicle_journey_indexes = model.get_corresponding_from_idx(line_idx);
    let vehicle_journey_1_idx = vehicle_journeys.get_idx("vehicle_journey_id_1").unwrap();
    assert!(vehicle_journey_indexes.contains(&vehicle_journey_1_idx));
    let vehicle_journey_2_idx = vehicle_journeys.get_idx("vehicle_journey_id_2").unwrap();
    assert!(vehicle_journey_indexes.contains(&vehicle_journey_2_idx));
    let vehicle_journey_3_idx = vehicle_journeys.get_idx("vehicle_journey_id_3").unwrap();
    assert!(vehicle_journey_indexes.contains(&vehicle_journey_3_idx));
    let vehicle_journey_4_idx = vehicle_journeys.get_idx("vehicle_journey_id_4").unwrap();
    assert!(vehicle_journey_indexes.contains(&vehicle_journey_4_idx));

    let line_indexes = model.get_corresponding_from_idx(vehicle_journey_1_idx);
    assert!(line_indexes.contains(&line_idx));
}
