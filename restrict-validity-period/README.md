# `restrict-validity-period`

Command-Line Interface to restrict the validity period of a [NTFS] dataset and purge out-of-date data.

[NTFS]: https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fr.md

## Installation

As `restrict-validity-period` is not pushed to crates.io yet, you can install it by cloning `transit_model`.

```bash
git clone https://github.com/CanalTP/transit_model
cd transit_model
cargo install --path restrict-validity-period
```

## Usage

```bash
# One-day restriction
restrict-validity-period --input /path/to/ntfs/folder/ --output /path/to/ntfs/ --start-validity-date 2019-01-01 --end-validity-date 2019-01-01
```

* `--input` is the path to a folder containing NTFS data format
* `--output` is the path to a folder where the NTFS will be exported
* `--start-validity-date` is the start of the desired validity period (included)
* `--end-validity-date` is the end of the desired validity period (included)

Get more information about the available options with `--help`.
