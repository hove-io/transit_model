# NTFS to GTFS conversion

## Introduction

This document describes how a [NTFS] is transformed into a [GTFS] feed in Navitia Transit Model.

The resulting GTFS feed is composed of the following objects:

* [agency](#agencytxt)
* [routes](#routestxt)
* [stops](#stopstxt)
* [trips](#tripstxt)
* [stop_times](#stop_timestxt)
* [calendar_dates](#calendar_datestxt): only this file is provided instead of the calendar.txt file.
* [attributions](#attributionstxt)

The following additional files are generated only if the corresponding objects are present in the NTFS.

* [transfers](#transferstxt)
* [shapes](#shapestxt)
* [stop_extensions](#stop_extensionstxt): additional information providing the complementary stop codes used in external systems.

[GTFS]: https://gtfs.org/reference/static
[NTFS]: https://github.com/hove-io/ntfs-specification/blob/master/ntfs_fr.md

## Mapping between NTFS and GTFS objects

### agency.txt

| GTFS field      | Required | NTFS file    | NTFS field       | Note                                                   |
| --------------- | -------- | ------------ | ---------------- | ------------------------------------------------------ |
| agency_id       | yes      | networks.txt | network_id       |                                                        |
| agency_name     | yes      | networks.txt | network_name     |                                                        |
| agency_url      | yes      | networks.txt | network_url      | `http://www.navitia.io/` if the value is not provided. |
| agency_timezone | yes      | networks.txt | network_timezone | `Europe/Paris` if the value is not provided.           |
| agency_lang     | no       | networks.txt | network_lang     |                                                        |
| agency_phone    | no       | networks.txt | network_phone    |                                                        |
| agency_fare_url | no       | networks.txt | network_fare_url |                                                        |

### routes.txt

Each line of this file corresponds to a transit line modeled in the NTFS feed. In case a transit line uses more than one modes of transportation, it should be modeled separately for each different mode, according to the mapping of modes presented below. The priorities follow the [NeTex Specification](http://www.normes-donnees-tc.org/wp-content/uploads/2014/05/NF_Profil_NeTEx_pour_les_arrets-_F-_-_v2.pdf) (cf. chapter 6.2.3).

| GTFS field       | Required | NTFS file | NTFS field      | Note                                                                                                                                                               |
| ---------------- | -------- | --------- | --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| route_id         | yes      | lines.txt | line_id         | See below for lines containing trips with different modes                                                                                                          |
| agency_id        | no       | lines.txt | network_id      | (link to the [agency.txt](#agencytxt) file)                                                                                                                        |
| route_short_name | yes      | lines.txt | line_code       | empty string "" if the value is not provided. If `--mode-in-route-short-name` is provided to the binary, the commercial mode will be added to the route short name |
| route_long_name  | yes      | lines.txt | line_name       |                                                                                                                                                                    |
| route_type       | yes      |           |                 | The corresponding physical mode of the trips of the line. See the table below for the mapping of modes.                                                            |
| route_color      | no       | lines.txt | line_color      |                                                                                                                                                                    |
| route_text_color | no       | lines.txt | line_text_color |                                                                                                                                                                    |
| route_sort_order | no       | lines.txt | line_sort_order |                                                                                                                                                                    |

**Mapping of `route_type` with physical modes**

| physical_mode_id in the NTFS | route_type in the GTFS | extended GTFS route_type | Priority w.r.t. NeTex | Absolute order |
| ---------------------------- | ---------------------- | ------------------------ | --------------------- | -------------- |
| Tramway                      | 0                      | 900                      | 5                     | 1              |
| RailShuttle                  | 0                      | 900                      | 3                     | 2              |
| Metro                        | 1                      | 400                      | 4                     | 3              |
| LocalTrain                   | 2                      | 100                      | 3                     | 4              |
| LongDistanceTrain            | 2                      | 100                      | 3                     | 5              |
| RapidTransit                 | 2                      | 100                      | 3                     | 6              |
| Train                        | 2                      | 100                      | 3                     | 7              |
| BusRapidTransit              | 3                      | 700                      | 7                     | 8              |
| Bus                          | 3                      | 700                      | 7                     | 9              |
| Coach                        | 3                      | 200                      | 7                     | 10             |
| Boat                         | 4                      | 1200                     | 2                     | 11             |
| Ferry                        | 4                      | 1200                     | 2                     | 12             |
| Funicular                    | 7                      | 1400                     | 6                     | 13             |
| Shuttle                      | 7                      | 1400                     | 6                     | 14             |
| SuspendedCableCar            | 6                      | 1300                     | 7                     | 15             |
| Air                          | 3                      | 1100                     | 1                     | 16             |
| Taxi                         | 3                      | 1500                     | 7                     | 17             |

The `physical_modes` Air and Taxi are not available in standard GTFS `route_type`s and should be considered as unknown for the GTFS (see below).
However, these `physical_modes` are available in extended mode.
If the physical_mode is unknown, trips should be considered as Bus (route_type = 3) and with a priority of 18 .

**Export of NTFS lines containing trips with different modes**
A GTFS `route` can only contains trips with one mode (ie. `route_type`).
If a NTFS `line` contains `trip`s that should be associated with different gtfs `route_type`s, 2 different GTFS `route`s must be generated:

* The trips using the physical mode with the lowest priority are modeled by a GTFS `route` with the field `route_id` matching the value of the NTFS `line_id`.
* The trips using other physical modes are modeled by a separate GTFS `route` for each corresponding `route_type`, adding the suffix ":<physical_mode_id>" to the value of `route_id` and assigning the corresponding physical mode to the field `route_type`.

### stops.txt

Stop zones (NTFS stops having `location_type` = 2) are ignored in the current version.

| GTFS field     | Required | NTFS file                       | NTFS field     | Note                                                                                                                                                                                                                                                                                                                |
| -------------- | -------- | ------------------------------- | -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| stop_id        | yes      | stops.txt                       | stop_id        |                                                                                                                                                                                                                                                                                                                     |
| stop_code      | no       | stops.txt                       | stop_code        |                                                                                                                                                                                                                                                                                                                     |
| stop_name      | yes      | stops.txt                       | stop_name      |                                                                                                                                                                                                                                                                                                                     |
| stop_lat       | yes      | stops.txt                       | stop_lat       |                                                                                                                                                                                                                                                                                                                     |
| stop_lon       | yes      | stops.txt                       | stop_lon       |                                                                                                                                                                                                                                                                                                                     |
| zone_id        | no       | stops.txt                       | fare_zone_id   |                                                                                                                                                                                                                                                                                                                     |
| location_type  | no       | stops.txt                       | location_type  | The value is set to `0` if the input value is `0` or invalid or unspecified, `1` if the input value is `1`, `2` if the input value is `3`, `3` if the input value is `4` and `4` if the input value is `5`.                                                                                                         |
| parent_station | no       | stops.txt                       | parent_station |                                                                                                                                                                                                                                                                                                                     |
| timezone       | no       | stops.txt                       | stop_timezone       |                                                                                                                                                                                                                                                                                                                     |
| stop_desc      | no       | comments.txt, comment_links.txt | comment_name   | The value of `comment_name` referenced by the `comment_id` having an `object_type` = `stop_point` or `object_type` = `stop_area` and an `object_id` equal to the corresponding `stop_id`. In case of more than one comments linked to the same trip, the first comment in alphabetical order is taken into account. |
| wheelchair_boarding  | no       | equipments.txt             | wheelchair_boarding       | The value of `wheelchair_boarding` referenced by the `equipment_id` of this stop.                                                                                                                                                                                                                                                                                                                 |
| platform_code  | no       | stops.txt                       | platform_code       |                                                                                                                                                                                                                                                                                                                     |

### trips.txt

| GTFS field            | Required | NTFS file           | NTFS field            | Note                                                                                    |
| --------------------- | -------- | ------------------- | --------------------- | --------------------------------------------------------------------------------------- |
| route_id              | yes      | trips.txt           | route_id              |                                                                                         |
| service_id            | yes      | trips.txt           | service_id            |                                                                                         |
| trip_id               | yes      | trips.txt           | trip_id               |                                                                                         |
| trip_headsign         | no       | trips.txt           | trip_headsign         |                                                                                         |
| trip_short_name       | no       | trips.txt           | trip_short_name       |                                                                                         |
| direction_id          | no       | routes.txt          | direction_type        | `0` if the corresponding value is `forward`, `clockwise` or `inbound`. `1` otherwise.   |
| block_id              | no       | trips.txt           | block_id              |                                                                                         |
| shape_id              | no       | trips.txt           | geometry_id           | (link to the [shapes.txt](#shapestxt) file)                                             |
| wheelchair_accessible | no       | trip_properties.txt | wheelchair_accessible | The value of `wheelchair_accessible` referenced by the `trip_property_id` of this trip. |
| bikes_allowed         | no       | trip_properties.txt | bike_accepted         | The value of `bike_accepted` referenced by the `trip_property_id` of this trip.         |

### stop_times.txt

| GTFS field     | Required | NTFS file                       | NTFS field     | Note                                                                                                                                                                                                                                                                                |
| -------------- | -------- | ------------------------------- | -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| trip_id        | yes      | stop_times.txt                  | trip_id        | (link to the [trips.txt](#tripstxt) file)                                                                                                                                                                                                                                           |
| arrival_time   | yes      | stop_times                      | arrival_time   |                                                                                                                                                                                                                                                                                     |
| departure_time | yes      | stop_times.txt                  | departure_time |                                                                                                                                                                                                                                                                                     |
| stop_id        | yes      | stop_times.txt                  | stop_id        | (link to the [stops.txt](#stopstxt) file)                                                                                                                                                                                                                                           |
| stop_sequence  | yes      | stop_times.txt                  | stop_sequence  |                                                                                                                                                                                                                                                                                     |
| stop_headsign  | no       | stop_times.txt                  | stop_headsign  |                                                                                                                                                                                                                                                                                     |
| pickup_type    | no       | stop_times.txt                  | pickup_type    |                                                                                                                                                                                                                                                                                     |
| drop_off_type  | no       | stop_times.txt                  | drop_off_type  |                                                                                                                                                                                                                                                                                     |
| stop_time_desc | no       | comments.txt, comment_links.txt | comment_name   | The value of `comment_name` referenced by the `comment_id` having an `object_type` = `stop_point`and an `object_id` equal to the corresponding `trip_id`. In case of more than one comments linked to the same stop, the first comment in alphabetical order is taken into account. |
| local_zone_id  | no       | stop_times.txt                  | local_zone_id  |                                                                                                                                                                                                                                                                                     |

### calendar_dates.txt

This file is the same as the NTFS calendar_dates.txt file. All dates of service are included in this file (no calendar.txt file provided).

### transfers.txt

| GTFS field        | Required | NTFS file     | NTFS field        | Note                                      |
| ----------------- | -------- | ------------- | ----------------- | ----------------------------------------- |
| from_stop_id      | yes      | transfers.txt | from_stop_id      | (link to the [stops.txt](#stopstxt) file) |
| to_stop_id        | yes      | transfers.txt | to_stop_id        | (link to the [stops.txt](#stopstxt) file) |
| transfer_type     | yes      |               |                   | `2`                                       |
| min_transfer_time | no       | transfers.txt | min_transfer_time |                                           |

### shapes.txt

| GTFS field        | Required | NTFS file      | NTFS field   | Note                                                                                   |
| ----------------- | -------- | -------------- | ------------ | -------------------------------------------------------------------------------------- |
| shape_id          | yes      | geometries.txt | geometry_id  |                                                                                        |
| shape_pt_lat      | yes      | geometries.txt | geometry_wkt | Latitude of the stop in the shape                                                      |
| shape_pt_lon      | yes      | geometries.txt | geometry_wkt | Longitude of the stop in the shape                                                     |
| shape_pt_sequence | yes      |                |              | Integer starting at 0 and increase by an increment of one for every point in the shape |

### stop_extensions.txt

This file contains the complementary stop codes from the NTFS object_codes.txt file. If no additional stop code is specified, this file is not generated.
If N complementary codes are specified for a stop, there will be N separate lines in the file for the different stop_id/system_name pairs.

| GTFS field  | Required | NTFS file        | NTFS field    | Note                                                                                                  |
| ----------- | -------- | ---------------- | ------------- | ----------------------------------------------------------------------------------------------------- |
| stop_id     | yes      | object_codes.txt | object_id     | `stop_id` of the stop having a complementary code specified (link to the [stops.txt](#stopstxt) file) |
| system_name | yes      | object_codes.txt | object_system |                                                                                                       |
| system_code | yes      | object_codes.txt | object_code   |                                                                                                       |

### attributions.txt

If all the trips on a route are operated by the same company, then the allocation applies to the route, otherwise it applies to each trip.

| GTFS field        | Required               | NTFS file     | NTFS field        | Note                                      |
| ----------------- | ---------------------- | ------------- | ----------------- | ----------------------------------------- |
| route_id          | conditionally required | routes.txt    | route_id          | (link to the [routes.txt](#routestxt) (1) |
| trip_id           | conditionally required | trips.txt     | trip_id           | (link to the [trips.txt](#tripstxt) (1)   |
| is_operator       | yes                    | companies.txt |                   | `1`                                       |
| organization_name | yes                    | companies.txt | company_name      |                                           |
| attribution_url   | no                     | companies.txt | company_url       |                                           |
| attribution_email | no                     | companies.txt | company_mail      |                                           |
| attribution_phone | no                     | companies.txt | company_phone     |                                           |

(1) If either route_id or trip_id is defined, the other must be empty. 