use crate::{
    model::Collections,
    objects::{Line, Route},
};
use relational_types::Relation;
use std::collections::BTreeSet;
use typed_index_collection::Idx;

/// If a line name is empty, it's set with the name of its first "forward" route (in alphabetical order)
/// Note: possible improvement of this functionality; factoring, pooling and using
/// the same algorithm as for route names enhancing (with traffic analysis)
pub fn adjust_lines_names(
    collections: &mut Collections,
    lines_to_routes: &impl Relation<From = Line, To = Route>,
) {
    let mut line_names: Vec<(Idx<Line>, String)> = Vec::new();
    for (line_idx, _) in collections
        .lines
        .iter()
        .filter(|(_, l)| l.name.trim().is_empty())
    {
        let routes_idx =
            lines_to_routes.get_corresponding_forward(&std::iter::once(line_idx).collect());
        let route_name = routes_idx
            .iter()
            .map(|route_idx| &collections.routes[*route_idx])
            .filter(|route| !route.name.trim().is_empty())
            .filter(|route| route.direction_type == Some(String::from("forward")))
            .map(|route| &route.name)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .next();
        if let Some(route_name) = route_name {
            line_names.push((line_idx, route_name.clone()));
        }
    }
    for (line_idx, line_name) in line_names {
        // note: choice is made to keep 'forward_line_name' and 'backward_line_name' as its are
        collections.lines.index_mut(line_idx).name = line_name;
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;
    use relational_types::OneToMany;

    #[test]
    fn empty_line_with_non_empty_route() {
        let mut collections = Collections::default();
        collections
            .lines
            .push(Line {
                id: String::from("line_id1"),
                name: String::new(),
                ..Default::default()
            })
            .unwrap();
        collections
            .routes
            .push(Route {
                id: String::from("route_id1"),
                line_id: String::from("line_id1"),
                direction_type: Some(String::from("forward")),
                name: String::from("my route id1"),
                ..Default::default()
            })
            .unwrap();

        let lines_to_routes =
            OneToMany::new(&collections.lines, &collections.routes, "lines_to_routes").unwrap();

        adjust_lines_names(&mut collections, &lines_to_routes);
        let line1 = collections.lines.get("line_id1").unwrap();
        assert_eq!("my route id1", line1.name);
    }

    #[test]
    fn non_empty_line_with_non_empty_route() {
        let mut collections = Collections::default();
        collections
            .lines
            .push(Line {
                id: String::from("line_id1"),
                name: String::from("my line id1"),
                ..Default::default()
            })
            .unwrap();
        collections
            .routes
            .push(Route {
                id: String::from("route_id1"),
                line_id: String::from("line_id1"),
                direction_type: Some(String::from("forward")),
                name: String::from("my route id1"),
                ..Default::default()
            })
            .unwrap();

        let lines_to_routes =
            OneToMany::new(&collections.lines, &collections.routes, "lines_to_routes").unwrap();

        adjust_lines_names(&mut collections, &lines_to_routes);
        let line1 = collections.lines.get("line_id1").unwrap();
        assert_eq!("my line id1", line1.name);
    }

    #[test]
    fn empty_line_with_backward_route() {
        let mut collections = Collections::default();
        collections
            .lines
            .push(Line {
                id: String::from("line_id1"),
                name: String::new(),
                ..Default::default()
            })
            .unwrap();
        collections
            .routes
            .push(Route {
                id: String::from("route_id1"),
                line_id: String::from("line_id1"),
                direction_type: Some(String::from("backward")),
                name: String::from("my route id1"),
                ..Default::default()
            })
            .unwrap();

        let lines_to_routes =
            OneToMany::new(&collections.lines, &collections.routes, "lines_to_routes").unwrap();

        adjust_lines_names(&mut collections, &lines_to_routes);
        let line1 = collections.lines.get("line_id1").unwrap();
        assert_eq!("", line1.name);
    }

    #[test]
    fn empty_line_with_non_empty_route_but_direction_opts() {
        let mut collections = Collections::default();
        collections
            .lines
            .push(Line {
                id: String::from("line_id1"),
                name: String::new(),
                forward_name: Some("A".to_string()),
                forward_direction: Some("B".to_string()),
                backward_name: Some("C".to_string()),
                backward_direction: Some("D".to_string()),
                ..Default::default()
            })
            .unwrap();
        collections
            .routes
            .push(Route {
                id: String::from("route_id1"),
                line_id: String::from("line_id1"),
                direction_type: Some(String::from("forward")),
                name: String::from("my route id1"),
                ..Default::default()
            })
            .unwrap();

        let lines_to_routes =
            OneToMany::new(&collections.lines, &collections.routes, "lines_to_routes").unwrap();

        adjust_lines_names(&mut collections, &lines_to_routes);
        let line1 = collections.lines.get("line_id1").unwrap();
        assert_eq!("my route id1", line1.name);
        assert_eq!("A", line1.forward_name.as_ref().unwrap());
        assert_eq!("B", line1.forward_direction.as_ref().unwrap());
        assert_eq!("C", line1.backward_name.as_ref().unwrap());
        assert_eq!("D", line1.backward_direction.as_ref().unwrap());
    }

    #[test]
    fn empty_line_with_2_non_empty_routes() {
        let mut collections = Collections::default();
        collections
            .lines
            .push(Line {
                id: String::from("line_id1"),
                name: String::new(),
                ..Default::default()
            })
            .unwrap();
        collections
            .routes
            .push(Route {
                id: String::from("route_id1"),
                line_id: String::from("line_id1"),
                direction_type: Some(String::from("forward")),
                name: String::from("my route id B"),
                ..Default::default()
            })
            .unwrap();
        collections
            .routes
            .push(Route {
                id: String::from("route_id2"),
                line_id: String::from("line_id1"),
                direction_type: Some(String::from("forward")),
                name: String::from("my route id A"),
                ..Default::default()
            })
            .unwrap();

        let lines_to_routes =
            OneToMany::new(&collections.lines, &collections.routes, "lines_to_routes").unwrap();

        adjust_lines_names(&mut collections, &lines_to_routes);
        let line1 = collections.lines.get("line_id1").unwrap();
        assert_eq!("my route id A", line1.name);
    }
}
