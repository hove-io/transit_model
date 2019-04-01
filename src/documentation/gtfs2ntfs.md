# gtfs2ntfs
gtfs2ntfs is the use of transit_model to convert a GTFS dataset into a NTFS dataset.

## How to use
### Basic Use
A basic usage of gtfs2ntfs is the following:
`./gtfs2ntfs -i path/to/gtfs.zip -o path/to/dest/ntfs.zip`

This will convert the GTFS into a valid NTFS, using the conversion method described in [gtfs_read.md](./gtfs_read.md) specifications. See next chapter to details about this use.

### Adding a prefix to all identifiers
Specifying a prefix will change all the identifiers of the generated NTFS file.
For exemple: `./gtfs2ntfs -i path/to/gtfs.zip -p my_prefix -o path/to/dest/ntfs.zip`
The previous command will prepend all the identifiers (line_id, route_id, etc.) with the string `my_prefix:`.

Prepending all the identifiers with a unique prefix ensures that the NTFS identifiers are unique accross all the NTFS datasets. With this assumption, merging two NTFS datasets can be done without worrying about conflicting identifiers.

Note that the prefix is also applied to `contributor_id` and `dataset_id` (see below), but not to physical_modes (see [data_prefix.md](./data_prefix.md))

### Specifying contributor and dataset properties
Details about the contributor and the dataset to be converted can be provided with a config file containing:
* details about the dataset beeing converted,
* details about the provider of the data (ie. `contributor`),

The command line is the following:
`./gtfs2ntfs -i path/to/gtfs.zip -c path/to/config.json -o path/to/dest/ntfs.zip`

**Details about config file**
The config file is a JSON file with :
* a `contributor` object containing :
  * a `contributor_id` : default value is `default_contributor`
  * a `contributor_name` : default value is `Default contributor`
  * a `contributor_license` : default is `Unknown license`
  * a `contributor_website` : default is <not provided>
* a dataset object :
  * a `dataset_id` : default value is `default_dataset`
  * a `dataset_desc` : a description of the content of the dataset. Default value is <not provided>
  * a `dataset_system` : a description of the system providing the dataset. Default value is <not provided>

The config file could contains only one or several properties. In this case, default values are to be applied as decribed above.

**Applying prefix on contributor and dataset**
If the `-p my_prefix` is used, the provided prefix is also applied to the `contributor_id` and `dataset_id` properties. Default values for thoses properties are also prefixed. If the provided prefix is `my_prefix`:
* default `contributor_id` becomes `my_prefix:default_contributor`
* default `dataset_id` becomes `my_prefix:default_dataset`


### Use of all parameters
Call exemple:  `./gtfs2ntfs -i path/to/gtfs.zip -p my_prefix -c path/to/config.json -o path/to/dest/ntfs.zip`
