`ntfs2ntfs`
=====

Command-Line Interface to check and clean a [NTFS] data format into data format.

[NTFS]: https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fr.md

# Installation
To install, use

```bash
cargo install ntfs2ntfs
```

# Usage

```bash
ntfs2ntfs --input /path/to/ntfs/folder/ --output /path/to/ntfs/
```

- `--input` is the path to a folder containing NTFS data format
- `--output` is the path to a folder where the NTFS will be exported

Get more information about the available options with `--help`.
