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
