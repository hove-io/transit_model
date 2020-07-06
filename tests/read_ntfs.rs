// Copyright (C) 2017 Kisio Digital and/or its affiliates.
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

use pretty_assertions::assert_eq;
use relational_types::IdxSet;
use std::collections::HashMap;
use transit_model::model::{Collections, GetCorresponding, Model};
use transit_model::objects::*;
use transit_model::test_utils::*;
use typed_index_collection::{CollectionWithId, Id, Idx};

fn get<T, U>(idx: Idx<T>, collection: &CollectionWithId<U>, objects: &Model) -> Vec<String>
where
    U: Id<U>,
    IdxSet<T>: GetCorresponding<U>,
{
    let mut ids: Vec<_> = objects
        .get_corresponding_from_idx(idx)
        .iter()
        .map(|idx| collection[*idx].id().to_string())
        .collect();
    ids.sort();
    ids
}

#[test]
#[allow(clippy::cognitive_complexity)]
fn minimal() {
    let ntm = transit_model::ntfs::read("tests/fixtures/minimal_ntfs/").unwrap();

    assert_eq!(8, ntm.stop_areas.len());
    assert_eq!(12, ntm.stop_points.len());
    assert_eq!(3, ntm.commercial_modes.len());
    assert_eq!(3, ntm.lines.len());
    assert_eq!(6, ntm.routes.len());
    // 3 + 3 automatically inserted modes 'Bike', 'BikeSharingService', and 'Car'
    assert_eq!(6, ntm.physical_modes.len());
    assert_eq!(6, ntm.vehicle_journeys.len());
    assert_eq!(1, ntm.networks.len());
    assert_eq!(1, ntm.companies.len());
    assert_eq!(1, ntm.contributors.len());
    assert_eq!(1, ntm.datasets.len());
    assert_eq!(0, ntm.geometries.len());

    let gdl = ntm.stop_areas.get_idx("GDL").unwrap();
    assert_eq!(3, ntm.get_corresponding_from_idx::<_, StopPoint>(gdl).len());
    assert_eq!(
        vec!["Bus", "Metro", "RapidTransit"],
        get(gdl, &ntm.physical_modes, &ntm)
    );
    assert_eq!(
        vec!["Bus", "Metro", "RER"],
        get(gdl, &ntm.commercial_modes, &ntm)
    );
    assert_eq!(vec!["TGN"], get(gdl, &ntm.networks, &ntm));
    assert_eq!(vec!["TGC"], get(gdl, &ntm.contributors, &ntm));

    let rera = ntm.lines.get_idx("RERA").unwrap();
    assert_eq!(
        vec!["Bus", "RapidTransit"],
        get(rera, &ntm.physical_modes, &ntm)
    );
    assert_eq!(vec!["RER"], get(rera, &ntm.commercial_modes, &ntm));
    assert_eq!(vec!["TGN"], get(rera, &ntm.networks, &ntm));
    assert_eq!(vec!["TGC"], get(rera, &ntm.contributors, &ntm));
    assert_eq!(vec!["RERAB", "RERAF"], get(rera, &ntm.routes, &ntm));
    assert_eq!(
        vec!["RERAB1", "RERAF1"],
        get(rera, &ntm.vehicle_journeys, &ntm)
    );
    assert_eq!(
        vec!["CDGR", "CDGZ", "DEFR", "GDLR", "MTPZ", "NATR"],
        get(rera, &ntm.stop_points, &ntm)
    );
    assert_eq!(
        vec!["CDG", "DEF", "GDL", "NAT", "Navitia:CDGZ", "Navitia:MTPZ"],
        get(rera, &ntm.stop_areas, &ntm)
    );
}

#[test]
fn ntfs_stop_zones() {
    let ntm = transit_model::ntfs::read("tests/fixtures/minimal_ntfs/").unwrap();
    let stop_zone_1 = ntm.stop_points.get("MTPZ").unwrap();
    assert_eq!(StopType::Zone, stop_zone_1.stop_type);
    let stop_zone_2 = ntm.stop_points.get("CDGZ").unwrap();
    assert_eq!(StopType::Zone, stop_zone_2.stop_type);
}

#[test]
fn ntfs_stops_output() {
    let ntm = transit_model::ntfs::read("tests/fixtures/minimal_ntfs/").unwrap();
    test_in_tmp_dir(|output_dir| {
        transit_model::ntfs::write(&ntm, output_dir, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &output_dir,
            Some(vec!["stops.txt", "stop_times.txt"]),
            "tests/fixtures/ntfs2ntfs/stops",
        );
    });
}

#[test]
fn test_minimal_fares_stay_same() {
    let ntm = transit_model::ntfs::read("tests/fixtures/ntfs2ntfs/fares").unwrap();
    test_in_tmp_dir(|output_dir| {
        transit_model::ntfs::write(&ntm, output_dir, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &output_dir,
            Some(vec!["stops.txt", "fares.csv", "od_fares.csv", "prices.csv"]),
            "tests/fixtures/ntfs2ntfs/fares",
        );
    });
}

#[test]
fn test_minimal_platforms_stay_same() {
    let ntm = transit_model::ntfs::read("tests/fixtures/ntfs2ntfs/platforms").unwrap();
    test_in_tmp_dir(|output_dir| {
        transit_model::ntfs::write(&ntm, output_dir, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &output_dir,
            Some(vec!["stops.txt"]),
            "tests/fixtures/ntfs2ntfs/platforms",
        );
    });
}

#[test]
fn test_minimal_fares_stay_same_with_empty_of_fares() {
    let ntm = transit_model::ntfs::read("tests/fixtures/ntfs2ntfs/empty_od_fares").unwrap();
    test_in_tmp_dir(|output_dir| {
        transit_model::ntfs::write(&ntm, output_dir, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &output_dir,
            Some(vec!["fares.csv", "od_fares.csv", "prices.csv"]),
            "tests/fixtures/ntfs2ntfs/empty_od_fares",
        );
    });
}

#[test]
fn ntfs() {
    let pt_objects = transit_model::ntfs::read("tests/fixtures/ntfs/").unwrap();

    // comments
    use crate::CommentType::*;
    fn assert_eq_comment(comment: &Comment, id: &str, name: &str, comment_type: CommentType) {
        let expect = Comment {
            id: id.to_string(),
            name: name.to_string(),
            comment_type,
            label: None,
            url: None,
        };
        assert_eq!(&expect, comment);
    }
    assert_eq!(4, pt_objects.comments.len());
    let rera_comment_indexes = &pt_objects.lines.get("RERA").unwrap().comment_links;
    let mut iter = rera_comment_indexes
        .iter()
        .filter_map(|comment_id| pt_objects.comments.get(comment_id));
    assert_eq_comment(
        iter.next().unwrap(),
        "RERACOM1",
        "some information",
        Information,
    );
    assert_eq_comment(
        iter.next().unwrap(),
        "RERACOM2",
        "strange comment type",
        Information,
    );
    assert_eq_comment(
        iter.next().unwrap(),
        "RERACOM3",
        "no comment type",
        Information,
    );
    assert_eq_comment(
        iter.next().unwrap(),
        "RERACOM4",
        "on demand transport comment",
        OnDemandTransport,
    );
    assert_eq!(None, iter.next());

    let mut stop_time_comments = HashMap::<(String, u32), String>::new();
    stop_time_comments.insert(("RERAB1".to_string(), 5), "RERACOM1".to_string());

    assert_eq!(stop_time_comments, pt_objects.stop_time_comments);
}

#[test]
fn optional_empty_collections_not_created() {
    let ntm = transit_model::ntfs::read("tests/fixtures/minimal_ntfs/").unwrap();
    test_in_tmp_dir(|path| {
        transit_model::ntfs::write(&ntm, path, get_test_datetime()).unwrap();

        use std::collections::HashSet;
        let entries: HashSet<String> = ::std::fs::read_dir(path)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert!(!entries.contains("comments.txt"));
        assert!(!entries.contains("comment_links.txt"));
        assert!(!entries.contains("equipments.txt"));
        assert!(!entries.contains("transfers.txt"));
        assert!(!entries.contains("trip_properties.txt"));
        assert!(!entries.contains("geometries.txt"));
        assert!(!entries.contains("object_properties.txt"));
        assert!(!entries.contains("object_codes.txt"));
        assert!(!entries.contains("admin_stations.txt"));
    });
}

#[test]
fn preserve_frequencies() {
    let ntm = transit_model::ntfs::read("tests/fixtures/ntfs/").unwrap();
    test_in_tmp_dir(|output_dir| {
        transit_model::ntfs::write(&ntm, output_dir, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &output_dir,
            Some(vec!["frequencies.txt", "stop_times.txt", "trips.txt"]),
            "tests/fixtures/ntfs2ntfs/frequencies",
        );
    });
}

#[test]
fn preserve_grid() {
    let ntm = transit_model::ntfs::read("tests/fixtures/ntfs/").unwrap();
    test_in_tmp_dir(|output_dir| {
        transit_model::ntfs::write(&ntm, output_dir, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &output_dir,
            Some(vec![
                "grid_calendars.txt",
                "grid_exception_dates.txt",
                "grid_periods.txt",
                "grid_rel_calendar_line.txt",
            ]),
            "tests/fixtures/ntfs2ntfs/grid",
        );
    });
}

#[test]
fn enhance_lines_opening_time() {
    let ntm = transit_model::ntfs::read("tests/fixtures/ntfs2ntfs/lines-opening/input/").unwrap();
    test_in_tmp_dir(|output_dir| {
        transit_model::ntfs::write(&ntm, output_dir, get_test_datetime()).unwrap();
        compare_output_dir_with_expected(
            &output_dir,
            Some(vec!["lines.txt"]),
            "tests/fixtures/ntfs2ntfs/lines-opening/output",
        );
    });
}

#[test]
fn sanitize_frequencies() {
    let mut collections = Collections::default();
    let frequency = Frequency {
        vehicle_journey_id: String::from("vehicle_journey_id_which_doesn_t_exist"),
        start_time: Time::new(0, 0, 0),
        end_time: Time::new(0, 0, 0),
        headway_secs: 0,
    };
    collections.frequencies.push(frequency);
    collections.sanitize().unwrap();
    assert_eq!(collections.frequencies.len(), 0);
}

#[test]
fn sanitize_grid() {
    let mut collections = Collections::default();
    let grid_calendar = GridCalendar {
        id: String::from("grid_calendar_id"),
        name: String::from("Grid Calendar Name"),
        monday: true,
        tuesday: true,
        wednesday: true,
        thursday: true,
        friday: true,
        saturday: false,
        sunday: false,
    };
    let grid_exception_date = GridExceptionDate {
        grid_calendar_id: String::from("grid_calendar_id"),
        date: Date::from_ymd(2019, 1, 1),
        r#type: true,
    };
    let grid_period = GridPeriod {
        grid_calendar_id: String::from("grid_calendar_id"),
        start_date: Date::from_ymd(2019, 1, 1),
        end_date: Date::from_ymd(2019, 12, 31),
    };
    let grid_rel_calendar_line = GridRelCalendarLine {
        grid_calendar_id: String::from("grid_calendar_id"),
        line_id: String::from("A line which doesn't exist"),
        line_external_code: None,
    };
    collections.grid_calendars.push(grid_calendar).unwrap();
    collections.grid_exception_dates.push(grid_exception_date);
    collections.grid_periods.push(grid_period);
    collections
        .grid_rel_calendar_line
        .push(grid_rel_calendar_line);

    collections.sanitize().unwrap();
    assert_eq!(0, collections.grid_calendars.len());
    assert_eq!(0, collections.grid_exception_dates.len());
    assert_eq!(0, collections.grid_periods.len());
    assert_eq!(0, collections.grid_rel_calendar_line.len());
}

#[test]
fn sanitize_grid_with_line_external_code() {
    let mut collections = Collections::default();
    let grid_calendar = GridCalendar {
        id: String::from("grid_calendar_id"),
        name: String::from("Grid Calendar Name"),
        monday: true,
        tuesday: true,
        wednesday: true,
        thursday: true,
        friday: true,
        saturday: false,
        sunday: false,
    };
    let grid_rel_calendar_line = GridRelCalendarLine {
        grid_calendar_id: String::from("grid_calendar_id"),
        line_id: String::from(""),
        line_external_code: Some(String::from("some_line_id")),
    };
    collections.grid_calendars.push(grid_calendar).unwrap();
    collections
        .grid_rel_calendar_line
        .push(grid_rel_calendar_line);

    collections.sanitize().unwrap();
    assert_eq!(1, collections.grid_calendars.len());
    assert_eq!(1, collections.grid_rel_calendar_line.len());
}
