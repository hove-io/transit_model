# gtfs2ntfs 
gtfs2ntfs is the use of navitia_model to convert a GTFS dataset into a NTFS dataset.

## How to use
### Basic Use
A basic usage of gtfs2ntfs is the following : 
`./gtfs2ntfs -i path/to/gtfs.zip -o path/to/dest/ntfs.zip`

This will convert the GTFS into a valid NTFS, using the conversion method described in [gtfs_read.md](./gtfs_read.md) specifications. See next chapter to details about this use.

### Use with a config file
A mode complexe read method is to provide a config file that will contain details about :
* the dataset beeing converted, 
* the provider of the data (ie. `contributor`),
* and a prefix specific to the provider to be applied to all GTFS identifiers.

The command line is the following : 
`./gtfs2ntfs -i path/to/gtfs.zip -c path/to/config.json -o path/to/dest/ntfs.zip`

**Warning**
The prefix, that must be unique for a contributor, ensure that the NTFS identifiers are unique accross all the NTFS datasets. With this assumption, merging two NTFS datasets can be done without worrying about conflicting identifiers.   


**Details about config file**
The config file is a JSON file with :
* a `contributor` object containing :
 * a `contributor_id` : default value is `default_contributor`
 * a `contributor_name` : default value is `Default contributor`
 * a `contributor_license` : default is `Unknown license`
 * a `contributor_website` : default is <not provided>
* a dataset object : 
 * a `dataset_desc` : a description of the content of the dataset. Default value is <not provided>
 * a `dataset_system` : a description of the system providing the dataset. Default value is <not provided>
* a `prefix` : default is <not provided>



