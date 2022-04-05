# `ntfs2gtfs`

Command-Line Interface to convert [NTFS] data format into [GTFS] data
format.

[GTFS]: https://gtfs.org/reference/static
[NTFS]: https://github.com/hove-io/ntfs-specification/blob/master/ntfs_fr.md

## Installation

As `ntfs2gtfs` is not pushed to crates.io yet, you can install it by cloning `transit_model`.

```bash
git clone https://github.com/hove-io/transit_model
cd transit_model
cargo install --path ntfs2gtfs
```

## Usage

```bash
ntfs2gtfs --input /path/to/ntfs/folder/ --output /path/to/gtfs/
```

* `--input` is the path to a folder containing NTFS data format
* `--output` is the path to a folder where the GTFS will be exported
* `--mode-in-route-short-name` (optional) allows adding the commercial mode at the beginning of the route short name.

Get more information about the available options with `ntfs2gtfs --help`.

## Specifications

As NTFS is the pivot format for conversion, [common NTFS rules] is useful.\
For input and output, see [NTFS to GTFS specifications].

[common NTFS rules]: ../documentation/common_ntfs_rules.md
[NTFS to GTFS specifications]: ../documentation/ntfs_to_gtfs_specs.md
