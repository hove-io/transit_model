# transit_model

[![GitHub release (latest by date)](https://img.shields.io/github/v/release/hove-io/transit_model?color=4baea0&style=flat-square&logo=github)](https://github.com/hove-io/transit_model/releases)
[![API documentation](https://img.shields.io/badge/docs.rs-transit_model-66c2a5?style=flat-square&color=769ECB&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K)](https://docs.rs/transit_model)
[![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/hove-io/transit_model/continuous-integration-deb11.yml?logo=github&style=flat-square)](https://github.com/hove-io/transit_model/actions/workflows/continuous-integration-deb11.yml)
[![Codecov Code Coverage](https://img.shields.io/codecov/c/github/hove-io/transit_model?color=f01f7a&style=flat-square&logo=codecov)](https://codecov.io/gh/hove-io/transit_model)
[![License: AGPL v3.0](https://img.shields.io/github/license/hove-io/transit_model?color=9873b9&style=flat-square)](./LICENSE)

**`transit_model`** is a Rust crate to manage, convert and enrich transit
data.\
This is done by implementing the [NTFS] model (used in [navitia]).

This repository regroups crates that offer enabler-libraries and binaries to
convert and enrich transit data.

Additionally, `transit_model` is itself a library providing various
functionalities. Please refer to the code, [examples](examples) and
[documentation](https://docs.rs/transit_model) to discover them.

Please check documentation attached to each crate:

* binary [**gtfs2netexfr**](gtfs2netexfr/README.md) converts [GTFS] data format
  into [NeTEx]-France data format.
* binary [**gtfs2ntfs**](gtfs2ntfs/README.md) converts [GTFS] data format into
  [NTFS] data format.
* binary [**ntfs2gtfs**](ntfs2gtfs/README.md) converts [NTFS] data format into
  [GTFS] data format.
* binary [**ntfs2netexfr**](ntfs2netexfr/README.md) converts [NTFS] data format
  into [NeTEx]-France data format.
* binary [**ntfs2ntfs**](ntfs2ntfs/README.md) checks and cleans a [NTFS]
  dataset.
* binary [**restrict-validity-period**](restrict-validity-period/README.md)
  restricts the validity period of a [NTFS] dataset and purges out-of-date data.

## Usage with Docker

For all the binaries mentioned above, it is also possible to use them with
Docker. All the binaries are part of the image [`navitia/transit_model`] which
is tagged alongside the crate [`transit_model`].  Let's use `gtfs2ntfs` as an
example.

```shell
mkdir output-ntfs/
docker run \
	--volume "${PWD}/tests/fixtures/gtfs:/gtfs" \
	--volume "${PWD}/output-ntfs:/ntfs" \
	navitia/transit_model \
	gtfs2ntfs \
	--input /gtfs \
	--output /ntfs
```

## Setup Rust environment

`transit_model` is developed in [Rust].

If you want to contribute or install binaries, you need to install a [Rust] environment: see [rustup.rs]

[Rust]: https://www.rust-lang.org
[rustup.rs]: https://rustup.rs

## [PROJ] dependency

Based on [PROJ], the [`proj` crate] allows the transformation of
localization coordinates.

Some `transit_model`'s crates (see each documentation) use [PROJ].\
So it must be installed on the system to compile and use those crates.

### [PROJ] for binaries

The [`proj` crate] requires [PROJ].

If your system has `pkg-config` and a sufficiently new version of [PROJ] installed, it will be used.
Otherwise, the crate falls back to building [PROJ] from source, which requires some build time dependencies.

To install [PROJ] build time dependencies, you can execute the following command (On Debian systems):

```sh
make install_proj_deps
```

You can also install the required [PROJ] version system-wide to avoid full rebuild (ex: `cargo clean`):

```sh
make install_proj
```

[PROJ installation instructions](https://github.com/OSGeo/PROJ#installation)
may help, too.

### Using [PROJ] and transit_model as a developer

[`proj` crate] is a binding to the C library.

[PROJ] is configured as a `feature` of the `transit_model` crate.\
So to use it for coding, the `proj` feature must be activated
(`cargo build --features=proj`).\
Then specific code should be conditionally enabled with
`#[cfg(feature="proj")]`.

## NTFS Level of Support

`transit_model` is supporting most of [NTFS] format.\
From the standard, some of the functionalities are not fully supported:

* No support for Line Groups (files `line_groups.txt` and `line_group_links.txt`).
* The field `trip_short_name_at_stop` in `stop_times.txt` introduced in version
  `v0.10.0` (see [NTFS changelog in French]) is not supported.

## Contributing

Please see [CONTRIBUTING](CONTRIBUTING.md) to know more about the code or how
to test, contribute, report issues.

## License

Licensed under [GNU Affero General Public License v3.0](LICENSE)

[GTFS]: https://gtfs.org/reference/static
[navitia]: https://github.com/hove-io/navitia
[NeTEx]: http://netex-cen.eu
[NTFS]: https://github.com/hove-io/ntfs-specification/blob/master/ntfs_fr.md
[NTFS changelog in French]: https://github.com/hove-io/ntfs-specification/blob/master/ntfs_changelog_fr.md
[PROJ]: https://proj.org
[`proj` crate]: https://crates.io/crates/proj
[`navitia/transit_model`]: https://hub.docker.com/r/navitia/transit_model
[`transit_model`]: https://crates.io/crates/transit_model
