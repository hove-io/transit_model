# `gtfs2netexfr`

Command-Line Interface to convert [GTFS] data format into [NeTEx]-France data
format.

[GTFS]: https://gtfs.org/reference/static
[NeTEx]: http://netex-cen.eu

## Installation

To install, you first need to install [PROJ].\
See [PROJ installation instructions].

[PROJ]: https://proj.org/
[PROJ installation instructions]: ../README.md#proj-dependency

As `gtfs2netexfr` is not pushed to crates.io yet, you can install it by cloning `transit_model`.

```bash
git clone https://github.com/CanalTP/transit_model
cd transit_model
cargo install --path gtfs2netexfr
```

## Usage

```bash
gtfs2netexfr --input /path/to/gtfs/folder/ --output /path/to/netexfr/ --participant CanalTP
```

* `--input` is the path to a folder containing GTFS data format
* `--output` is the path to a folder where the NeTEx France will be exported
* `--participant` is an identifier for the instigator of this NeTEx France
  export; it is exported in each NeTEx file

Get more information about the available options with `gtfs2netexfr --help`.

Finally, it's possible to [check the output manually](../CONTRIBUTING.md#check-outputs-manually).

## Specifications

As NTFS is the pivot format for conversion, [common NTFS rules] is useful.\
For input, see [GTFS to NTFS specifications].\
For output, see [NTFS to NeTEx-France specifications].

[common NTFS rules]: ../documentation/common_ntfs_rules.md
[GTFS to NTFS specifications]: ../documentation/gtfs_to_ntfs_specs.md
[NTFS to NeTEx-France specifications]: ../documentation/ntfs_to_netex_france_specs.md
