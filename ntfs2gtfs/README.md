# `ntfs2gtfs`

Command-Line Interface to convert [NTFS] data format into [GTFS] data
format.

[GTFS]: https://gtfs.org/reference/static
[NTFS]: https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fr.md

## Installation

As `ntfs2gtfs` is not pushed to crates.io yet, you can install it by cloning `transit_model`.

```bash
git clone https://github.com/CanalTP/transit_model
cd transit_model
cargo install --path ntfs2gtfs
```

## Usage

```bash
ntfs2gtfs --input /path/to/ntfs/folder/ --output /path/to/gtfs/
```

- `--input` is the path to a folder containing NTFS data format
- `--output` is the path to a folder where the GTFS will be exported

Get more information about the available options with `--help`.
