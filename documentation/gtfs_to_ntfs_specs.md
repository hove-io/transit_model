# GTFS reading specification

## Purpose

This document aims to describe how the [GTFS] format is read in the Navitia Transit Model. To improve readability of this document, the specification will describe the transformation of a GTFS feed into a [NTFS] feed (which is a bunch of csv files accordingly to the memory Navitia Transit Model).

## Introduction

If at any time of the conversion, the GTFS is not conform to the [GTFS]
specification, the conversion should stop immediately with an error, unless
otherwise specified.

At the end of the conversion, a sanitizing operation is started on the final
model. See [common NTFS rules] for more information.

### Prepending data

As explained in [common NTFS rules], a prefix is added to all identifiers during the conversion in order to guarantee uniqueness among objects IDs.
In the following chapters, identifiers may be prepend with this _prefix_ using this pattern : **\<prefix>:<object\_id>**.
The use of this specific pattern is shown explicitly using the value **ID** in the column _Constraint_ in the tables below.

To reinforce the uniqueness some objects might have a sub-prefix (generated automatically) in addition to their prefix.\
The pattern is the following **\<prefix>:<sub_prefix>:<object\_id>**.\
Objects concerned by this sub-prefix in this connector are: `calendars`, `trips`, `trip_properties`, `frequencies`, `comments`, `comment_links`, `geometries`, `equipments`.

In addition, the NTFS format introduces 2 objects to enable the manipulation of several datasets: contributors and datasets. Those two objects are described in [common NTFS rules].

Two parameters can be specified as CLI arguments of the converter in order to determine if on demand transport (ODT) data should be considered when reading the input GTFS (in particular, when [reading the stop_times.txt file](#reading-stop_timestxt)):

* a boolean parameter `--odt`, by default set to `false`, indicating if the GTFS should be considered as containing ODT information
* a string `--odt-comment "some message"` setting the message associated to an ODT comment. 

A third boolean CLI argument (`--read-as-line`) may affect the reading of the file [routes.txt](#reading-routestxt). If true, each GTFS "Route" will generate a different "Line" else we group the routes by "agency_id" and "route_short_name" (or "route_long_name" if the short name is empty) and create a "Line" for each group.


## Mapping of objects between GTFS and NTFS

| GTFS object | NTFS object(s)                              |
| ----------- | ------------------------------------------- |
| agency      | network and company                         |
| route       | line, route, physical_mode, commercial_mode |
| trip        | route and trip                              |
| stop_time   | stop_time                                   |
| transfer    | transfer                                    |
| shape       | geometry                                    |
| frequency   | trip and stop_time                          |

## Detailed mapping of objects

### Reading agency.txt

The field "agency_id" may not be provided in the GTFS as it's an optional field.

* If there is only one agency, the "agency_id" is considered to be "1".
* If there are several agencies, the program will raise an exception as it won't be able to choose the right agency for the routes.

#### Loading Networks

If 2 networks with the same ID are specified, the conversion should stop
immediately with an error.

| NTFS file    | NTFS field       | Constraint | GTFS file  | GTFS field      | Note                         |
| ------------ | ---------------- | ---------- | ---------- | --------------- | ---------------------------- |
| networks.txt | network_id       | ID         | agency.txt | agency_id       | See above when not specified |
| networks.txt | network_name     | Required   | agency.txt | agency_name     |                              |
| networks.txt | network_url      | Optional   | agency.txt | agency_url      |                              |
| networks.txt | network_timezone | Optional   | agency.txt | agency_timezone |                              |
| networks.txt | network_lang     | Optional   | agency.txt | agency_lang     |                              |
| networks.txt | network_phone    | Optional   | agency.txt | agency_phone    |                              |
| networks.txt | network_fare_url | Optional   | agency.txt | agency_fare_url |                              |

**_"Source" complementary code :_**

A complementary `object_code` is added to each network with the following properties:

* `object_type` : the fixed value `network`
* `object_id` : the value of the `network_id` field
* `object_system` : the fixed value `source`
* `object_code` : the unmodified value of `agency_id` (or `1` if the value is not provided as stated above)


#### Loading Companies

If 2 companies with the same ID are specified, the conversion should stop
immediately with an error.

| NTFS file     | NTFS field    | Constraint | GTFS file  | GTFS field   | Note                                                     |
| ------------- | ------------- | ---------- | ---------- | ------------ | -------------------------------------------------------- |
| companies.txt | company_id    | ID         | agency.txt | agency_id    | `1` if the value is not provided (same rule as networks) |
| companies.txt | company_name  | Required   | agency.txt | agency_name  |                                                          |
| companies.txt | company_url   | Optional   | agency.txt | agency_lang  |                                                          |
| companies.txt | company_phone | Optional   | agency.txt | agency_phone |                                                          |

**_"Source" complementary code :_**

A complementary `object_code` is added to each company with the following properties:

* `object_type` : the fixed value `company`
* `object_id` : the value of the `company_id` field
* `object_system` : the fixed value `source`
* `object_code` : the unmodified value of `agency_id` (or `1` if the value is not provided as stated above)


### Reading stops.txt

Like the GTFS, the NTFS group stop_points and stop_areas in on file : stops.txt.
If the stop_points have the same ID, the conversion should stop immediately with
an error. Likewise for the stop_areas.

| NTFS file      | NTFS field          | Constraint | GTFS file | GTFS field          | Note                                                                                                                                                                                                       |
| -------------- | ------------------- | ---------- | --------- | ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| stops.txt      | stop_id             | ID         | stops.txt | stop_id             | All slashes `/` will be removed                                                                                                                                                                            |
| stops.txt      | stop_code           | Optional   | stops.txt | stop_code           | Additionally, this GTFS property is stored as an associated code for this stop. See (2) for complementary properties.                                                                                      |
| stops.txt      | stop_name           | Required   | stops.txt | stop_name           |                                                                                                                                                                                                            |
| stops.txt      | stop_lat            | Required   | stops.txt | stop_lat            |                                                                                                                                                                                                            |
| stops.txt      | stop_lon            | Required   | stops.txt | stop_lon            |                                                                                                                                                                                                            |
| stops.txt      | location_type       | Optional   | stops.txt | location_type       | The value is set to `0` if the input value is `0` or unspecified or invalid, `1` if the input value is `1`, `3` if the input value is `2`, `4` if the input value is `3` and `5` if the input value is `4` |
| stops.txt      | parent_station      | Optional   | stops.txt | parent_station      | All slashes `/` are removed (1)                                                                                                                                                                            |
| stops.txt      | stop_timezone       | Optional   | stops.txt | stop_timezone       |                                                                                                                                                                                                            |
| stops.txt      | fare_zone_id        | Optional   | stops.txt | zone_id             | Only for stop_point (`location_type` = 0)                                                                                                                                                                  |
| comments.txt   | comment_value       | Optional   | stops.txt | stop_desc           | See (3) for additional properties                                                                                                                                                                          |
| equipments.txt | wheelchair_boarding | Optional   | stops.txt | wheelchair_boarding | If value is not one of `0`, `1` or `2`, then set to `0`. See (4) for detailed info.                                                                                                                        |

(1) If the `parent_station` field of a stop_point (`location_type` = 0 or empty) is missing or empty, then a stop_area should be created, using the following properties :

* `stop_id` : the stop_id of the stop_point, with the following pattern : **Navitia:<stop_id of the stop_point>**
* `stop_name` : the stop_name of the stop_point
* `stop_lat` : the stop_lat of the stop_point
* `stop_lon` : the stop_lon of the stop_point
* `location_type` : fixed value "1" (to specify it's a stop_area)
The `parent_station` of the stop_point should then contain the generated `stop_area.id`.

(2) The `stop_code` field is added as a complementary `object_code` with the following properties:

* `object_type` : `stop_point` or `stop_area`  accordingly to the `location_type` value
* `object_id` : NTFS `stop_id`
* `object_system` : Fixed value `gtfs_stop_code`
* `object_code` : value of the `stop_code` property
The `gtfs_stop_code` complementary `object_code` is kept here for backward
compatibility reasons. It will be removed in the future.

(3) The `comment` object is a complex type with additional properties :

* `comment_id` : specify an identifier with the pattern **<prefix>:stop:<stop_id of GTFS>**
* `comment_type` : specify the fixed value "information"

(4) The `equipment` object is a complex type with additional properties :

+ `equipment_id` : should be generated by the reader.
+ `wheelchair_boarding` : possible values are the same in both GTFS and NTFS.
Be careful to only create necessary equipments and avoid duplicates.

**_"Source" complementary code :_**

A complementary `object_code` is added to each stop with the following properties:

* `object_type` : the fixed value `stop_point` or `stop_area` (depending on the object)
* `object_id` : the NTFS value of the `stop_id` field
* `object_system` : the fixed value `source`
* `object_code` : the unmodified GTFS value of `stop_id`

### Reading routes.txt

##### Mapping of route_type with modes

The standard values of the `route_type` field are directly mapped to the NTFS modes. [Extended GTFS modes](https://developers.google.com/transit/gtfs/reference/extended-route-types) are read by categories mapping the most prominent mode. The priority is used to prioritize the use of a commercial mode when creating a Line grouping routes with different `route_type`s. This priorization follow the [Netex Specification](http://www.normes-donnees-tc.org/wp-content/uploads/2014/05/NF_Profil_NeTEx_pour_les_arrets-_F-_-_v2.pdf) in chapter 6.2.3 (and also indicated in the NTFS Specification).

| GTFS route_type  | NTFS physical_mode ID (1) | NTFS commercial_mode ID (2) | NTFS commercial_mode name | Priority |
| ---------------- | ------------------------- | --------------------------- | ------------------------- | -------- |
| 0, 9XX           | Tramway                   | Tramway                     | Tramway                   | 3        |
| 1, 4XX, 5XX, 6XX | Metro                     | Metro                       | Metro                     | 4        |
| 2, 1XX, 3XX      | Train                     | Train                       | Train                     | 2        |
| 3, 7XX, 8XX      | Bus                       | Bus                         | Bus                       | 8        |
| 4, 10XX, 12XX    | Ferry                     | Ferry                       | Ferry                     | 1        |
| 5                | Funicular                 | CableCar                    | Cable car                 | 6        |
| 6, 13XX          | SuspendedCableCar         | SuspendedCableCar           | Suspended cable car       | 7        |
| 7, 14XX          | Funicular                 | Funicular                   | Funicular                 | 5        |
| 2XX              | Coach                     | Coach                       | Coach                     | 8        |
| 11XX             | Air                       | Air                         | Airplane                  | 0        |
| 15XX             | Taxi                      | Taxi                        | Taxi                      | 8        |
| 16XX, 17XX       | Bus                       | UnknownMode                 | Unknown mode              | 8        |

(1) The physical_mode ID is a mapping with a specific value as described in the NTFS format specification. This value must not be prefixed.
(2) The commercial_mode ID are standardized when converting from GTFS. This value must not be prefixed.

All `physical_mode` are enhanced with CO2 emission and fallback modes, following
the documentation in [common NTFS rules](common_ntfs_rules.md#co2-emissions-and-fallback-modes).

#### Loading Routes

A Route is created for each direction of existing trips.  If 2 routes with the
same ID are specified, the conversion should stop immediately with an error.
_Warning :_ If the GTFS route has no trips, the Navitia Route should NOT be created and a warning should be logged.

| NTFS file    | NTFS field     | Constraint | GTFS file  | GTFS field      | Note                                                                                                                                                        |
| ------------ | -------------- | ---------- | ---------- | --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| routes.txt   | route_id       | ID         | routes.txt | route_id        | append a `_R` suffix for the Route grouping trips with `direction_id` = 1 (no suffix for `0` or undefined `direction_id`)                                   |
| routes.txt   | route_name     | Required   | routes.txt | route_long_name | (1)                                                                                                                                                         |
| routes.txt   | direction_type | Optional   |            |                 | (2)                                                                                                                                                         |
| routes.txt   | line_id        | Required   |            |                 | corresponding `line.id` (see Line construction above)                                                                                                       |
| routes.txt   | destination_id | Optional   |            |                 | This field contains a stop_area.id of the most frequent destination of the contained trips (ie. the parent_station of the most frequent last stop of trips) |
| comments.txt | comment_value  | Optional   | routes.txt | route_desc      | The comment is generated only when the parameter `read-as-line` is deactivated. See (3) for additional properties                                                                                                                           |

(1) if only one route is created (only one direction in included trips), use
`route_long_name` or, if empty, use `route_short_name`. In case of multiple
routes created (multiple directions in included trips), see [common NTFS rules]
for generating the `route_name`.

(2) the field `direction_type` contains `backward` when grouping GTFS Trips with `direction_id` = 1, `forward` otherwise

(3) The `comment` object is a complex type with additional properties :

* `comment_id` : specify an identifier with the pattern **\<prefix>:route:<route_id of GTFS>**
* `comment_type` : specify the fixed value "Information"

**_"Source" complementary code :_**

A complementary `object_code` is added to each route with the following properties:

* `object_type` : the fixed value `route`
* `object_id` : the NTFS value of the `route_id` field
* `object_system` : the fixed value `source`
* `object_code` : the unmodified GTFS value of `route_id`

#### Loading Lines

A Navitia Line is created to group one or several Navitia Routes when they are
created with the same gtfs `agency_id` and the same `route_short_name` (or
`route_long_name` if the latter is empty).  If 2 lines with the same ID are
specified, the conversion should stop immediately with an error.

| NTFS file | NTFS field         | Constraint | GTFS file  | GTFS field       | Note                                                                                                                                                                                                                                                                             |
| --------- | ------------------ | ---------- | ---------- | ---------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| lines.txt | network_id         | Required   |            |                  | This field should contain the `network.id` corresponding to the `agency_id` of the routes; if no `agency_id` is specified in the route, use the ID of the unique network; if no network or multiple networks are available, the conversion should stop immediately with an error |
| lines.txt | line_id            | ID         | routes.txt | route_id         | Use the smallest `route_id` of the grouped gtfs Route                                                                                                                                                                                                                            |
| lines.txt | line_code          | Optional   | routes.txt | route_short_name |                                                                                                                                                                                                                                                                                  |
| lines.txt | line_name          | Required   | routes.txt |                  | The Navitia `route_name` of the Route with the smallest `route_id` (as a string) is used.                                                                                                                                                                                        |
| lines.txt | line_color         | Optional   | routes.txt | route_color      | if several values are available, a warning is logged and the color of the smallest `route_id` is used; if color format is incorrect, the value is dropped                                                                                                                        |
| lines.txt | line_text_color    | Optional   | routes.txt | route_text_color | same as line_color; if color format is incorrect, the value is dropped                                                                                                                                                                                                           |
| lines.txt | line_sort_order    | Optional   | routes.txt | route_sort_order |                                                                                                                                                                                                                                                                                  |
| lines.txt | commercial_mode_id | Required   | routes.txt | route_type       | See "Mapping of route_type with modes" chapter (1).                                                                                                                                                                                                                              |
| comments.txt | comment_value | Optional | routes.txt | route_desc | The comment is generated only when the parameter `read-as-line` is activated. See (2) for additional properties. |

(1) When several GTFS Routes with different `route_type`s are grouped together, the commercial_mode_id with the smallest priority should be used (as specified in chapter "Mapping of route_type with modes").

(2) The `comment` object is a complex type with additional properties :

* `comment_id` : specify an identifier with the pattern **\<prefix>:line:<route_id of GTFS>**
* `comment_type` : specify the fixed value "Information"

A complementary `object_code` is added to each line with the following properties:

* `object_type` : the fixed value `line`
* `object_id` : the NTFS value of the `line_id` field
* `object_system` : the fixed value `source`
* `object_code` : the unmodified GTFS value of `route_id`


### Reading calendars.txt and calendar_dates.txt

GTFS services are transformed into lists of active dates as if using a single NTFS
file `calendar_dates.txt`. The resulting NTFS files might be different following an
optimization operation applied at the end of the conversion, but the result should be
functionally identical.

* In case both files `calendar.txt` and `calendar_dates.txt` are present in the input dataset, the days of the week of the specified services within the date range [`start_date` - `end_date`] are transformed into explicit active service dates, taking into account the dates when service exceptions occur. Note that the generated (`service_id`, `date`) pairs must be unique.
* In case the file `calendar.txt` is empty or not present in the input dataset, the active service dates are loaded as is.

### Reading trips.txt

If 2 trips with the same ID are specified, the conversion should stop
immediately with an error.


| NTFS file | NTFS field       | Constraint | GTFS file  | GTFS field | Note                                                                                                     |
| --------- | ---------------- | ---------- | ---------- | ---------- | -------------------------------------------------------------------------------------------------------- |
| trips.txt | route_id         | Required   | trips.txt  | route_id   | cf. NTFS `route_id` definition above to specify the proper reference.                                    |
| trips.txt | service_id       | Required   | trips.txt  | service_id |                                                                                                          |
| trips.txt | trip_id          | Required   | trips.txt  | trip_id    |                                                                                                          |
| trips.txt | trip_headsign    | Optional   | trips.txt  |            | `trip_short_name`, or if empty `trip_headsign`                                                           |
| trips.txt | block_id         | Optional   | trips.txt  | block_id   |                                                                                                          |
| trips.txt | company_id       | Required   | routes.txt | agency_id  | The company corresponding to the `agency_id` of the trip's `route_id`                                    |
| trips.txt | physical_mode_id | Required   |            |            | use the `route_type` See ["Mapping of route_type with modes"](#mapping-of-route_type-with-modes) chapter |
| trips.txt | trip_property_id | Optional   | trips.txt  |            | (1)                                                                                                      |
| trips.txt | dataset_id       | Required   |            |            | The `dataset_id` provided (cf. [gtfs2ntfs.md](./gtfs2ntfs.md) )                                          |
| trips.txt | geometry_id      | Optional   | trips.txt  | shape_id   | All slashes `/` are removed                                                                              |

(1) The `trip_property` object is a complex type with additional properties :

* `trip_property_id`: should be generated by the reader.
* `wheelchair_accessible`: possible values are the same in both GTFS and NTFS; if value is not one of `0`, `1` or `2`, then set to `0`.
* `bike_accepted`: corresponding to the GTFS `bikes_allowed` property. Possible values are the same in both GTFS and NTFS; if value is not one of `0`, `1` or `2`, then set to `0`.
Be careful to only create necessary `trip_properties` and avoid duplicates.

**_"Source" complementary code :_**

A complementary `object_code` is added to each vehicle journey with the following properties:

* `object_type` : the fixed value `trip`
* `object_id` : the value of the `trip_id` field
* `object_system` : the fixed value `source`
* `object_code` : the unmodified GTFS value of `trip_id`

### Reading stop_times.txt

| NTFS file      | NTFS field          | Constraint | GTFS file      | GTFS field     | Note                                                                                                                          |
| -------------- | ------------------- | ---------- | -------------- | -------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| stop_times.txt | trip_id             | Required   | stop_times.txt | trip_id        | All slashes `/` are removed; if the corresponding trip doesn't exist, the conversion should stop immediately with an error    |
| stop_times.txt | arrival_time        | Optional   | stop_times.txt | arrival_time   | If not specified, see (1)                                                                                                     |
| stop_times.txt | departure_time      | Optional   | stop_times.txt | departure_time | If not specified, see (1)                                                                                                     |
| stop_times.txt | stop_id             | Required   | stop_times.txt | stop_id        | If the corresponding stop doesn't exist, the conversion should stop immediately with an error                                 |
| stop_times.txt | stop_sequence       | Required   | stop_times.txt | stop_sequence  |                                                                                                                               |
| stop_times.txt | stop_headsign       | Optional   | stop_times.txt | stop_headsign  |                                                                                                                               |
| stop_times.txt | pickup_type         | Optional   | stop_times.txt | pickup_type    | If invalid unsigned integer, default to `0`. If `2`, see (3) for the generation of comments.                                  |
| stop_times.txt | drop_off_type       | Optional   | stop_times.txt | drop_off_type  | If invalid unsigned integer, default to `0`. If `2`, see (3) for the generation of comments.                                  |
| stop_times.txt | stop_time_precision | Optional   | stop_times.txt | timepoint      | GTFS and NTFS values are inverted when no ODT information is considered. See (2). If invalid unsigned integer, default to `1` |

(1) GTFS `arrival_time` and `departure_time` should contain values.

* if both of them are empty :
  * if the stop_time is the first or the last of the trip, an error is returned
  * if not, the time should be interpolated (see below).
* if one of them is empty, a warning should be logged and the value of the other field should be copied to the empty one.

**Interpolation**
If a stop_time needs to be interpolated :

* collect the nearest preceding stop_time and the nearest following stop_time containing a valid time value
* apply a simple distribution for all the intermediate stop_times
For exemple :

| GTFS passing time | NTFS Extrapolated time |
| ----------------- | ---------------------- |
| 9:00              | 9:00                   |
| -                 | 9:30                   |
| -                 | 10:00                  |
| 10:30             | 10:30                  |

(2) Depending of the value of the parameter `odt`, the GTFS `timepoint` conversion rules for NTFS `stop_time_precision` are :

* if `odt` is set to `false` or empty:
  * if `timepoint` is unspecified => `stop_time_precision` equals 0 (Exact)
  * if `timepoint` equals 1 => `stop_time_precision` equals 0 (Exact)
  * if `timepoint` equals 0 => `stop_time_precision` equals 1 (Approximate)
* if `odt` is set to `true`:
  * if `timepoint` is unspecified => `stop_time_precision` equals 0 (Exact)
  * if `timepoint` equals 1 => `stop_time_precision` equals 0 (Exact)
  * if `timepoint` equals 0 => `stop_time_precision` equals 2 (Estimated, the bus may not even pass through this point)

(3) A comment associated to the stop_time is created in the files comments.txt and comment_links.txt as follows:

| NTFS file         | NTFS field   | Constraint | Value/Note                                                                                                                                                                                                                             |
| ----------------- | ------------ | ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| comments.txt      | comment_id   | Required   | The value of stop_time_id is used as the concatenation of trip_id and stop_sequence separated by `-`. Note that this field is prefixed as explained in [common NTFS rules].                                                            |
| comments.txt      | comment_type | Optional   | `on_demand_transport`                                                                                                                                                                                                                  |
| comments.txt      | comment_name | Required   | The message set for the parameter `odt_comment`.                                                                                                                                                                                       |
| comment_links.txt | object_id    | Required   | The value of stop_time_id is used as the concatenation of trip_id and stop_sequence separated by `-`. Note that this field is prefixed as explained in [common NTFS rules].                                                            |
| comment_links.txt | object_type  | Required   | `stop_time`                                                                                                                                                                                                                            |
| comment_links.txt | comment_id   | Required   | The value of stop_time_id is used as the concatenation of trip_id and stop_sequence separated by `-`. Note that, as this field references the comment in file comments.txt, it should be prefixed as explained in [common NTFS rules]. |

### Reading transfers.txt

* If 2 transfers with the same ID are specified, the conversion should stop
  immediately with an error
* If a line of the file is not conform to the specification, then the line is
  ignored

| NTFS file     | NTFS field             | Constraint | GTFS file     | GTFS field   | Note                                                                                                |
| ------------- | ---------------------- | ---------- | ------------- | ------------ | --------------------------------------------------------------------------------------------------- |
| transfers.txt | from_stop_id           | Required   | transfers.txt | from_stop_id | All slashes `/` are removed; if the `stop_id` doesn't exist in `stops.txt`, the transfer is ignored |
| transfers.txt | to_stop_id             | Required   | transfers.txt | to_stop_id   | All slashes `/` are removed; if the `stop_id` doesn't exist in `stops.txt`, the transfer is ignored |
| transfers.txt | min_transfer_time      | Optional   | transfers.txt |              | see (1)                                                                                             |
| transfers.txt | real_min_transfer_time | Optional   | transfers.txt |              | see (1)                                                                                             |
| transfers.txt | equipment_id           | Optional   | transfers.txt |              |                                                                                                     |

(1) NTFS `min_transfer_time` and `real_min_transfer_time` are calculated as
follows. Note that if value is not one of `0`, `1`, `2` or `3`, then set to `0`.

| GTFS `transfer_type` | NTFS `min_transfer_time`   | NTFS `real_min_transfer_time`          | Note                                                                                                                                                          |
| -------------------- | -------------------------- | -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0                    | time between 2 stop points | time between 2 stop points + 2 minutes | The time is calculated with the distance as the crow flies and a walking speed of 0.785 m/s. Speed value is lowered because effective transit is not straight |
| 1                    | 0                          | 0                                      |                                                                                                                                                               |
| 2                    | GTFS `min_transfer_time`   | GTFS `min_transfer_time`               | Log a warning message if the `min_transfer_time` is empty                                                                                                     |
| 3                    | 86400                      | 86400                                  |                                                                                                                                                               |

### Reading shapes.txt

| NTFS file      | NTFS field   | Constraint | GTFS file  | GTFS field                                    | Note                                                          |
| -------------- | ------------ | ---------- | ---------- | --------------------------------------------- | ------------------------------------------------------------- |
| geometries.txt | geometry_id  | ID         | shapes.txt | shape_id                                      | All slashes `/` are removed                                   |
| geometries.txt | geometry_wkt | Required   | shapes.txt | shape_pt_lat, shape_pt_lon, shape_pt_sequence | A WKT LINESTRING geometry is created from the 3 input fields. |

### Reading frequencies.txt

Frequencies are transformed into explicit passing times by creating new trips that operate on regular times within the specified period. For each line of the GTFS frequencies.txt file, the referenced trip and its stop_times are used as a sample to create the new trips whose stop_times are calculated based on the given headway.

A new trip is created, departing from the first stop every `headway_secs` seconds within the time period between `start_time` and `end_time`. Stop times of the referenced trip are used to calculate the time interval between two stop departures.
The departure time at the first stop of the last trip should not be later than the `end_time` value. In case both values for `start_time` and `end_time` are equal or `end_time` is smaller than `start_time`, the frequency is ignored (no new trip is created).

Note that the referenced trip (and its stop_times) is only used as a sample and is deleted in the resulting data. In case the referenced trip and/or its associated stop_times do not exist, the frequency is ignored (no new trip is created).

The identifier for each new trip is generated using the following pattern: \<trip_id>:<auto-incrimented integer\> and maintains the rest of the attributes of the sample trip. That is, all new trips are assigned to the same route as the route of the sample trip, have the same service_id, etc.

A complementary `object_code` is added to each new trip with the following properties:

* `object_type` : the fixed value `trip`
* `object_id` : the value of the `trip_id` field
* `object_system` : the fixed value `source`
* `object_code` : the unmodified initial GTFS value of `trip_id`

[GTFS]: https://gtfs.org/reference/static
[NTFS]: https://github.com/hove-io/ntfs-specification/blob/master/ntfs_fr.md
[common NTFS rules]: common_ntfs_rules.md
