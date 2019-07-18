# transit_model [![Build Status](https://travis-ci.org/CanalTP/transit_model.svg?branch=master)](https://travis-ci.org/CanalTP/transit_model)

`transit_model` is a Rust crate managing transit data by implementing the NTFS
model `v0.9.1` (used  in [navitia](https://github.com/CanalTP/ntfs-specification)). See the
section [NTFS: Level of Support](#ntfs-level-of-support) for more details about the
level of support of the NTFS standard.

This repository also provides :
- (incomplete) [GTFS](http://gtfs.org/) to [NTFS](https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fr.md) and (soon) NTFS to GTFS conversion.
- (incomplete) Generation of transfers.
.
- Merge [NTFS] (https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fr.md).

## Compile

```bash
cargo build --release
```

### Compile for KV1
The KV1 format needs a additional dependency called [PROJ](https://proj.org/)
which allows the transformation of localization coordinates.
[crates.io](https://crates.io/) provides a
[`proj`](https://crates.io/crates/proj) crate which is a binding to the C
library (version 6.1.0). This means you need [PROJ](https://proj.org/) version
6.1.0 installed on your system.  See [PROJ installation
instructions](https://github.com/OSGeo/PROJ#installation).

[PROJ](https://proj.org/) is configured as a `feature` of the `transit_model`
crate.  Once [PROJ](https://proj.org/) is installed on your machine, you need a
few more dependencies for building `transit_model`.
```
apt install -y clang libssl-dev
cargo build --features=proj
```

Now, you should be able to use the converter `kv12ntfs`. Enjoy!

### Using PROJ
If you want to use [PROJ](https://proj.org/) in your code, you can if you
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
`transit_model` is partially supporting `v0.9.1` of NTFS (see [CHANGELOG in
French](https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_changelog_fr.md)).
From the standard, some of the functionalities are not fully supported:
- Management of pathways introduced in version `v0.8` is not supported
- No support for all periodic description (files `grid_calendars.txt`,
  `grid_exception_dates.txt`, `grid_periods.txt` and `grid_rel_calendar_line.txt`)
- No support for Line Groups (files `line_groups.txt` and `line_group_links.txt`)

## FAQ
**I'm having the following error when building the Docker image from
`Dockerfile`**
```
$> docker build --tag navitia_model:deb-proj --file Dockerfile .
...
 ---> Running in 74d8af8aa077
 gpg: directory '/root/.gnupg' created
 gpg: keybox '/root/.gnupg/pubring.kbx' created
 gpg: keyserver receive failed: Cannot assign requested address
 The command '/bin/sh -c gpg2 --receive-keys ${GPG_KEY}' returned a non-zero
 code: 2
```

It seems that `gpg2` sometimes fails to find a public key from key server.
Launch the `docker build` command again, the build should succeed eventually. If
not, please fill an issue.

## License

Licensed under [GNU General Public License v3.0](LICENSE)
