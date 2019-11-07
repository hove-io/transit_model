# Shared specifications for all converters
This document explains the shared parts among all the converters when converting a 
data set from a given format into a [NTFS](https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fr.md) dataset.

## Data prefix
The construction of NTFS objects IDs requires, for uniqueness purpose, that a unique 
prefix (specified for each source of data as an additional parameter to each converter)
needs to be included in every object's id.

Prepending all the identifiers with a unique prefix ensures that the NTFS identifiers are unique accross all the NTFS datasets. With this assumption, merging two NTFS datasets can be done without worrying about conflicting identifiers.

This prefix should be applied to all NTFS identifiers except for the physical mode identifiers that are standardized and fixed values. Fixed values are described in the [NTFS specifications](https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fr.md#physical_modestxt-requis)

## Configuration of each converter
A configuration file `config.json`, as it is shown below, is provided for each 
converter and contains additional information about the data source as well as about 
the upstream system that generated the data (if available). In particular, it provides the necessary information for:
- the required NTFS files `contributors.txt` and `datasets.txt`
- some additional metadata can also be inserted in the file `feed_infos.txt`.

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

| NTFS file | NTFS field | key in `config.json` | Constraint | Note |
| --- | --- | --- | --- | ---
| contributors.txt | contributor_id | contributor_id | Required | This field is prefixed.
| contributors.txt | contributor_name | contributor_name | Required | 
| contributors.txt | contributor_license | contributor_license | Optional | 
| contributors.txt | contributor_website | contributor_website | Optional | 

### Loading Dataset

| NTFS file | NTFS field | key in `config.json` | Constraint | Note |
| --- | --- | --- | --- | ---
| datasets.txt | dataset_id | dataset_id | Required | This field is prefixed.
| datasets.txt | contributor_id | contributor_id | Required | This field is prefixed.
| datasets.txt | dataset_start_date |  |  | Smallest date of all the trips of the dataset.
| datasets.txt | dataset_end_date |  |  | Greatest date of all the trips of the dataset.

## CO2 emissions and fallback modes
Physical modes may not contain CO2 emissions. If the value is missing, we are
using default values (see below), mostly based on what is provided by
[ADEME](https://www.ademe.fr).

Physical Mode     | CO2 emission (gCO<sub>2</sub>-eq/km)
---               | ---
Air               | 144.6
Boat              |  NC
Bus               | 132
BusRapidTransit   |  84
Coach             | 171
Ferry             | 279
Funicular         |   3
LocalTrain        |  30.7
LongDistanceTrain |   3.4
Metro             |   3
RapidTransit      |   6.2
RailShuttle       |  NC
Shuttle           |  NC
SuspendedCableCar |  NC
Taxi              | 184
Train             |  11.9
Tramway           |   4

The following fallback modes are also added to the model (they're usually not
referenced by any Vehicle Journey).

Physical Mode      | CO2 emission (gCO<sub>2</sub>-eq/km)
---                | ---
Bike               |   0
BikeSharingService |   0
Car                | 184

## Common practices
The following rules apply to every converter, unless otherwise explicitly specified.
- When one or more stop_points in the input data are not attached to a stop_area, 
a stop_area is automatically created. The coordinates of the new stop_area are 
computed as the barycenter of the associated stop_points. (The same rule applies 
in case the stop_area exists in the input data, but the coordinates are not specified.)
- Unless otherwise specified, dates of service are trasnformed into a list of active dates as if using a single NTFS file `calendar_dates.txt`.

| NTFS file | NTFS field | Constraint | Note |
| --- | --- | --- | --- |
| calendar_dates.txt | service_id | Required | All slashes `/` are removed
| calendar_dates.txt | date | Required | 
| calendar_dates.txt | exception_type | Required | Fixed value `1`.

## Sanitizer
The sanitizer checks for incoherences in the model and also cleans up all dangling
objects (for example, a line which is not referred by any route). This process 
is explained below in details.

### Incoherences
This part of the process will check for model incoherences and will raise an
error if one is found.  The first category is about duplicate identifiers:
- if 2 datasets have the same identifier
- if 2 lines have the same identifier
- if 2 stop points have the same identifier
- if 2 stop areas have the same identifier
- if 2 routes have the same identifier
- if 2 vehicle journeys have the same identifier

The second category is about dangling references:
- if a transfer refers a stop which doesn't exist (`from_stop_id` and
  `to_stop_id`)
- if a vehicle journey refers to a route which doesn't exist
- if a vehicle journey refers to a commercial mode which doesn't exist
- if a vehicle journey refers to a dataset which doesn't exist
- if a vehicle journey refers to a company which doesn't exist
- if a vehicle journey refers to a calendar which doesn't exist
- if a line refers to a network which doesn't exist
- if a line refers to a commercial mode which doesn't exist
- if a route refers to a line which doesn't exist
- if a stop point refers to a stop area which doesn't exist
- if a dataset refers to a contributor which doesn't exist

### Dangling objects
After multiple processes applied to a NTFS, some objects might not be referenced
anymore. This part of the process remove all of these objects:
- datasets which are not referenced
- contributors which are not referenced
- companies which are not referenced
- networks which are not referenced
- lines which are not referenced
- routes which are not referenced
- vehicle journeys which are not referenced
- stop points which are not referenced
- stop areas which are not referenced
- services which doesn't contain any date
- geometries which are not referenced
- equipments which are not referenced
- transfers which are not referenced
- frequencies which are not referenced
- physical modes which are not referenced
- commercial modes which are not referenced
- trip properties which are not referenced
- comments which are not referenced
- grid calendar which refers a line which does not exist (through the relation
  in the file `grid_rel_calendar_line.txt`); **Exception**: when the
  `line_external_code` is used and the `line_id` is empty, the grid calendar is
  kept
- grid exception date which refers to a grid calendar which does not exist
- grid period which refers to a grid calendar which does not exist
