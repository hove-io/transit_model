# transit_model [![Build Status](https://travis-ci.org/CanalTP/transit_model.svg?branch=master)](https://travis-ci.org/CanalTP/transit_model)

`transit_model` is a Rust crate managing transit data by implementing the NTFS
model `v0.9` (used  in [navitia](https://github.com/CanalTP/navitia)). See the
section [NTFS: Level of Support](#ntfs-level-of-support) for more details about the
level of support of the NTFS standard.

This repository also provides :
- (incomplete) [GTFS](http://gtfs.org/) to [NTFS](https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/ntfs_fr.md) and (soon) NTFS to GTFS conversion.
- (incomplete) Generation of transfers.
.
- Merge [NTFS] (https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/ntfs_fr.md).

## Compile

```bash
cargo build --release
```

### Compile for KV1
The KV1 format needs a additional dependency called [Proj.4](https://proj4.org/)
which allows the transformation of localization coordinates.
[crates.io](https://crates.io/) provides a
[`proj`](https://crates.io/crates/proj) crate which is a binding to the C
library (version 6.1.0). This means you need [Proj.4](https://proj4.org/)
version 6.1.0 installed on your system.  See [Proj.4 installation
instructions](https://github.com/OSGeo/proj.4#installation) or take a look at
our [Dockerfile](https://github.com/CanalTP/transit_model/blob/kv1/Dockerfile).
```

[Proj.4](https://proj4.org/) is configured as a `feature` of the `transit_model`
crate.  Once [Proj.4](https://proj4.org/) is installed, you need a few more
dependencies for building `transit_model`.
```
apt install -y clang libssl-dev
cargo build --features=proj
```

Now, you should be able to use the converter `kv12ntfs`. Enjoy!

### Using Proj.4
If you want to use [Proj.4](https://proj4.org/) in your code, you can if you
activate the `proj` feature (`cargo build --features=proj`). Then don't forget
to protect your code with `#[cfg(feature="proj")]`.

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

## NTFS: Level of Support
`transit_model` is partially supporting `v0.9` of NTFS (see [CHANGELOG in
French](https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/ntfs_changelog_fr.md)). From the standard, some of the functionalities are not fully supported:
- Management of pathways introduced in version `v0.8` is not supported
- No support for all periodic description (files `grid_calendars.txt`,
  `grid_exception_dates.txt`, `grid_periods.txt` and `grid_rel_calendar_line.txt`)
- No support for Line Groups (files `line_groups.txt` and `line_group_links.txt`)

## License

Licensed under [GNU General Public License v3.0](LICENSE)
