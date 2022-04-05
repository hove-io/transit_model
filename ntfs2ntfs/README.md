# `ntfs2ntfs`

Command-Line Interface to check and clean a [NTFS] dataset.

[NTFS]: https://github.com/hove-io/ntfs-specification/blob/master/ntfs_fr.md

## Installation

As `ntfs2ntfs` is not pushed to crates.io yet, you can install it by cloning `transit_model`.

```bash
git clone https://github.com/hove-io/transit_model
cd transit_model
cargo install --path ntfs2ntfs
```

## Usage

```bash
ntfs2ntfs --input /path/to/ntfs/folder/ --output /path/to/ntfs/
```

* `--input` is the path to a folder containing NTFS data format
* `--output` is the path to a folder where the NTFS will be exported

Get more information about the available options with `ntfs2ntfs --help`.

## Specifications

As NTFS is the pivot format for data processing, [common NTFS rules] is useful.

[common NTFS rules]: ../documentation/common_ntfs_rules.md
