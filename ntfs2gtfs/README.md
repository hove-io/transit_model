`ntfs2gtfs`
=====

Command-Line Interface to convert [NTFS] data format into [GTFS] data
format.

[GTFS]: https://developers.google.com/transit/gtfs/reference/
[NTFS]: https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fr.md

# Installation
To install, use

```bash
cargo install ntfs2gtfs
```

# Usage

```bash
ntfs2gtfs --input /path/to/ntfs/folder/ --output /path/to/gtfs/
```

- `--input` is the path to a folder containing NTFS data format
- `--output` is the path to a folder where the GTFS will be exported

Get more information about the available options with `--help`.
