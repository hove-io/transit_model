# transit_model [![Build Status](https://travis-ci.org/CanalTP/transit_model.svg?branch=master)](https://travis-ci.org/CanalTP/transit_model)

`transit_model` is a Rust crate managing transit data by implementing the NTFS
model (used  in [navitia](https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fr.md)). See the
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

### Compile for KV1, NeTEx IDF and TransXChange
The KV1 format needs a additional dependency called [PROJ](https://proj.org/)
which allows the transformation of localization coordinates.
[crates.io](https://crates.io/) provides a
[`proj`](https://crates.io/crates/proj) crate which is a binding to the C
library (version 6.3.0). This means you need [PROJ](https://proj.org/) version
6.3.0 installed on your system.  See [PROJ installation
instructions](https://github.com/OSGeo/PROJ#installation).

[PROJ](https://proj.org/) is configured as a `feature` of the `transit_model`
crate.  Once [PROJ](https://proj.org/) is installed on your machine, you need a
few more dependencies for building `transit_model`.
```
apt install -y clang libssl-dev
cargo build --features=proj
```

Now, you should be able to use the readers `kv1::read`, `netex_idf::read` and
`transxchange::read`. Enjoy!

### Using PROJ
If you want to use [PROJ](https://proj.org/) in your code, you can if you
activate the `proj` feature (`cargo build --features=proj`). Then don't forget
to protect your code with `#[cfg(feature="proj")]`.

### Feature `xmllint`
`transit_model` is capable of exporting NeTEx France format. In the tests, we're
automatically verifying that the produced files are matching the NeTEx
specification.  For that, we're using the tool `xmllint` which can be install
on Debian with the package `libxml2-utils`. Therefore, these tests are run only
if you activate them. We also depend on NeTEx specification that are imported as
a git submodule.

```bash
git submodule update --init --recursive
apt install libxml2-utils
cargo test --features xmllint
```

## Benchmarking
A few benchmarks are available if you want to compare performance of a new
feature or of an optimization. Benchmarking functionality is only available in
Rust Nightly so to run them, you can do the following.

```
cargo +nightly bench --all-features
```

Of course, if you need to run one specific bench, you can refer to a specific
bench name in `benches/`.

```
rustup run nightly cargo bench read_kv1 --features proj
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

## Tests

```bash
cargo test
```

## NTFS: Level of Support
`transit_model` is partially supporting `v0.11.2` of NTFS (see [CHANGELOG in
French](https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_changelog_fr.md)).
From the standard, some of the functionalities are not fully supported:
- No support for Line Groups (files `line_groups.txt` and `line_group_links.txt`)
- The field `trip_short_name_at_stop` in `stop_times.txt` introduced in version
  `v0.10.0` is not supported

## License

Licensed under [GNU Affero General Public License v3.0](LICENSE)
