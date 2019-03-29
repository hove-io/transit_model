# transit_model [![Build Status](https://travis-ci.org/CanalTP/transit_model.svg?branch=master)](https://travis-ci.org/CanalTP/transit_model)

transit_model is a rust crate to manage transit data. Its model corresponds to the one used in [navitia](https://github.com/CanalTP/navitia). This repository also provides :
- (incomplete) [GTFS](http://gtfs.org/) to [NTFS](https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/ntfs_fr.md) and (soon) NTFS to GTFS conversion.
- (incomplete) Generation of transfers.
- (incomplete) [Netex](http://netex-cen.eu/) to [NTFS](https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/ntfs_fr.md) and (soon) Netex to GTFS conversion. Conversion fixtures comes from official [Github repository](https://github.com/NeTEx-CEN/NeTEx/).
- Merge [NTFS] (https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/ntfs_fr.md).

## Compile

```bash
cargo build --release
```

## Converting from GTFS to NTFS

NTFS needs a `Dataset` and a `Contributor`.
Default ones are provided by the command but you can pass a json file that contains some information for creating a `Dataset` and a `Contributor` as explained in the [documentation](src/documentation/gtfs2ntfs.md).

```json
{
    "contributor": {
        "contributor_id" : "your_contributor_id",
        "contributor_name" : "your_contributor_name",
        "contributor_license" : "your_contributor_license",
        "contributor_website" : "your_contributor_website"
    },
    "dataset": {
        "dataset_id" : "your_dataset_id",
        "dataset_desc" : "optional_dataset_desc",
        "dataset_system" : "optional_dataset_system"
    }
}
```

### Usage

```bash
target/release/gtfs2ntfs -h
```

### Example

```bash
target/release/gtfs2ntfs -i path/to/input/directory -c path/to/config.json -p PREFIX -o path/to/output/directory
```

### With docker

```bash
docker run --rm -v path/to/input:/app/input -v path/to/output:/app/output navitia/transit_model gtfs2ntfs -i /app/input -o /app/output -c /app/input/config.json -p PREFIX
```

## Tests

```bash
cargo test
```

## License

Licensed under [GNU General Public License v3.0](LICENSE)
