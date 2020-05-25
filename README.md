# transit_model

[![GitHub release (latest by date)](https://img.shields.io/github/v/release/CanalTP/transit_model?color=4baea0&style=flat-square&logo=github)](https://github.com/CanalTP/transit_model/releases)
[![Crates.io](https://img.shields.io/crates/v/transit_model?color=f1935c&logo=rust&style=flat-square)](https://crates.io/crates/transit_model)
[![API documentation](https://img.shields.io/badge/docs.rs-transit_model-66c2a5?style=flat-square&color=769ECB&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K)](https://docs.rs/transit_model)
[![GitHub Workflow Status](https://img.shields.io/github/workflow/status/CanalTP/transit_model/Continuous%20Integration?logo=github&style=flat-square)](https://github.com/CanalTP/transit_model/actions?query=workflow%3A%22Continuous+Integration%22)
[![License: AGPL v3.0](https://img.shields.io/github/license/CanalTP/transit_model?color=9873b9&style=flat-square)](../blob/master/LICENSE)

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

### Compile for NeTEx IDF
Some formats needs an additional dependency called [PROJ](https://proj.org/)
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

Now, you should be able to use a full-fledge `transit_model`. Enjoy!

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
