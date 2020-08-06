# Shared specifications for all converters
This document explains the shared parts among all the converters when converting a
data set from a given format into a [NTFS] dataset.

[NTFS]: https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fr.md

## Data prefix
The construction of NTFS objects IDs requires, for uniqueness purpose, that a unique
prefix (specified for each source of data as an additional parameter to each converter)
needs to be included in every object's id.

Prepending all the identifiers with a unique prefix ensures that the NTFS identifiers are unique accross all the NTFS datasets. With this assumption, merging two NTFS datasets can be done without worrying about conflicting identifiers.

This prefix should be applied to all NTFS identifiers except for the physical mode identifiers that are standardized and fixed values. Fixed values are described in the [NTFS specifications](https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fr.md#physical_modestxt-requis)

To reinforce the uniqueness some objects have a sub-prefix in addition to their prefix.\
The pattern is the following **\<prefix>:<sub_prefix>:<object\_id>**.\
The sub-prefix is automatically generated from the first 6 characters of the `dataset_id`.

Adding a sub-prefix allows the merge of seasonal datasets; similar referencial (e.g. `networks`, `lines`, `stop areas`, `stop points`) but different schedules (e.g. `trips`, `dates`).

The objects that may be concerned by this sub-prefix are: `calendars`, `trips`, `trip_properties`, `frequencies`, `comments`, `comment_links`, `geometries`, `equipments` (see each connector's documentation for details).

## Configuration of each converter
A configuration file `config.json`, as it is shown below, is provided for each
converter and contains additional information about the data source as well as about
the upstream system that generated the data (if available). In particular, it provides the necessary information for:
* the required NTFS files `contributors.txt` and `datasets.txt`
* some additional metadata can also be inserted in the file `feed_infos.txt`.

```json
{
    "contributor": {
        "contributor_id": "DefaultContributorId",
        "contributor_name": "DefaultContributorName",
        "contributor_license": "DefaultDatasourceLicense",
        "contributor_website": "http://www.default-datasource-website.com"
    },
    "dataset": {
        "dataset_id": "DefaultDatasetId"
    },
    "feed_infos": {
        "feed_publisher_name": "DefaultContributorName",
        "feed_license": "DefaultDatasourceLicense",
        "feed_license_url": "http://www.default-datasource-website.com",
    }
}
```
The objects `contributor` and `dataset` are required, containing at least the
corresponding identifier (and the name for `contributor`), otherwise the conversion
stops with an error. The object `feed_infos` is optional.

The files `contributors.txt` and `datasets.txt` provide additional information about the data source.

### Loading Contributor

| NTFS file        | NTFS field          | key in `config.json` | Constraint | Note                    |
| ---------------- | ------------------- | -------------------- | ---------- | ----------------------- |
| contributors.txt | contributor_id      | contributor_id       | Required   | This field is prefixed. |
| contributors.txt | contributor_name    | contributor_name     | Required   |                         |
| contributors.txt | contributor_license | contributor_license  | Optional   |                         |
| contributors.txt | contributor_website | contributor_website  | Optional   |                         |

### Loading Dataset

| NTFS file    | NTFS field         | key in `config.json` | Constraint | Note                                           |
| ------------ | ------------------ | -------------------- | ---------- | ---------------------------------------------- |
| datasets.txt | dataset_id         | dataset_id           | Required   | This field is prefixed.                        |
| datasets.txt | contributor_id     | contributor_id       | Required   | This field is prefixed.                        |
| datasets.txt | dataset_start_date |                      |            | Smallest date of all the trips of the dataset. |
| datasets.txt | dataset_end_date   |                      |            | Greatest date of all the trips of the dataset. |

## CO2 emissions and fallback modes
Physical modes may not contain CO2 emissions. If the value is missing, we are
using default values (see below), mostly based on what is provided by
[ADEME](https://www.ademe.fr).

| Physical Mode     | CO2 emission (gCO<sub>2</sub>-eq/km) |
| ----------------- | ------------------------------------ |
| Air               | 144.6                                |
| Boat              | NC                                   |
| Bus               | 132                                  |
| BusRapidTransit   | 84                                   |
| Coach             | 171                                  |
| Ferry             | 279                                  |
| Funicular         | 3                                    |
| LocalTrain        | 30.7                                 |
| LongDistanceTrain | 3.4                                  |
| Metro             | 3                                    |
| RapidTransit      | 6.2                                  |
| RailShuttle       | NC                                   |
| Shuttle           | NC                                   |
| SuspendedCableCar | NC                                   |
| Taxi              | 184                                  |
| Train             | 11.9                                 |
| Tramway           | 4                                    |

The following fallback modes are also added to the model (they're usually not
referenced by any trip).

| Physical Mode      | CO2 emission (gCO<sub>2</sub>-eq/km) |
| ------------------ | ------------------------------------ |
| Bike               | 0                                    |
| BikeSharingService | 0                                    |
| Car                | 184                                  |

## Common practices
The following rules apply to every converter, unless otherwise explicitly specified.

### General rules
* When one or more stop_points in the input data are not attached to a
  stop_area, a stop_area is automatically created for each one. The name, the
  coordinates, the visibility, and the timezone of the new `stop_area` are the
  same as the corresponding stop_point, the identifier is the `stop_point`'s
  identifier prefixed with `Navitia:`.
* If a `stop_area` doesn't have coordinates, the barycenter of the contained `stop_points` is used.
* Unless otherwise specified, dates of service are transformed into a list of active dates as if using a single NTFS file `calendar_dates.txt`. Those list of dates are then transformed to `calendar` and `calendar_dates` automatically.
* Any `/` character in an identifier of an object is removed.
* If a trip doesn't have a `trip_headsign`, it is automatically generated based
  on the name of the last stop point of the trip
* If a route doesn't have a `direction_type` (or empty),
  the `direction_type` "forward" is assigned by default
* If a route doesn't have a name (or empty), `name` and `destination_id` are
  automatically generated:
  * the `route.name` is generated with the following rules:
    1. select the most frequent `stop_area` origin and most frequent `stop_area`
       destination of all the associated trips
    2. in case of equal frequencies, the biggest `stop_area`s (the most
       `stop_points`) are chosen
    3. in case of `stop_area` of equal sizes, the `stop_area` names are sorted
       alphabetically and the first ones are taken
    4. finally, the `route.name` is generated with:
       `[name of origin's stop area] - [name of destination's stop area]`
  * the `route.destination_id` is set (overridden if needed) with the
    destination's stop area selected with the above rule
* If a line has an empty opening or closing times, then they are both generated.
  * the `line.opening_time` is generated with the smallest departure time (at the first stop) of all journeys on the lines.
  * the `line.closing_time` is generated with the biggest arrival time (at the last stop) of all journeys on the lines (+ 24h if the end is earlier than the start time).
  * if a line has several periods without circulation in the day, only the main one (larger and earlier) is used to define the opening and closing times.
  * lines with continuous circulation are indicated by default with an opening at 00:00 and a closing at 23:59.

### Conflicting identifiers
The model will raise a critical error if identifiers of 2 objects of the same type are identical.
For example:
* if 2 datasets have the same identifier
* if 2 lines have the same identifier
* if 2 stop_points have the same identifier
* if 2 stop_areas have the same identifier
* if 2 routes have the same identifier
* if 2 trips have the same identifier

Please note that a stop_area and a stop_point can have the same identifier because they are considered as different types of objects.

### Incoherences
Dangling references are cleaned up:
* if a transfer refers a stop which doesn't exist (`from_stop_id` and
  `to_stop_id`)
* if a trip refers to a route which doesn't exist
* if a trip refers to a commercial mode which doesn't exist
* if a trip refers to a dataset which doesn't exist
* if a trip refers to a company which doesn't exist
* if a trip refers to a calendar which doesn't exist
* if a line refers to a network which doesn't exist
* if a line refers to a commercial mode which doesn't exist
* if a route refers to a line which doesn't exist
* if a stop_point refers to a stop_area which doesn't exist
* if a dataset refers to a contributor which doesn't exist

### Unnecessary objects
Objects that are not relevant are cleaned up:
* `datasets` which are not referenced by `trips`
* `contributors` which are not referenced by `datasets`
* `companies` which are not referenced by `trips`
* `networks` containing no `line`
* `lines` containing no `route`
* `routes` containing no `trips`
* `trips` containing no `stop_time` or with empty `calendars`
* `stop_points` which are not referenced by `stop_times`
* `stop_areas` which are not referenced by `stop_points` or `routes`
* `calendars` which doesn't contain any active date
* `geometries` which are not referenced
* `equipments` which are not referenced by `stop_points`
* `frequencies` which are not referenced by `trips`
* `physical_modes` which are not referenced by `trips`
* `commercial_modes` which are not referenced by `lines`
* `trip_properties` which are not referenced by `trips`
* `comments` which are not referenced
* `grid_calendar` which refers to a `line` which does not exist (through the relation
  in the file `grid_rel_calendar_line.txt`); **Exception**: when the
  `line_external_code` is used and the `line_id` is empty, the `grid_calendar` is
  kept
* `grid_exception` date which refers to a `grid_calendar` which does not exist
* `grid_period` which refers to a `grid_calendar` which does not exist
