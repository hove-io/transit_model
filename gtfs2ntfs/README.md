# `gtfs2ntfs`

Command-Line Interface to convert [GTFS] data format into [NTFS] data
format.

[GTFS]: https://developers.google.com/transit/gtfs/reference/
[NTFS]: https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fr.md

## Installation

As `gtfs2ntfs` is not pushed to crates.io yet, you can install it by cloning `transit_model`.

```bash
git clone https://github.com/CanalTP/transit_model
cd transit_model
cargo install --path gtfs2ntfs
```

## Usage

```bash
gtfs2ntfs --input /path/to/gtfs/folder/ --output /path/to/ntfs/
```

- `--input` is the path to a folder containing GTFS data format
- `--output` is the path to a folder where the NTFS will be exported

Get more information about the available options with `--help`.
