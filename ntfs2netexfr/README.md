# `ntfs2netexfr`

Command-Line Interface to convert [NTFS] data format into [NeTEx]-France data
format.

[NTFS]: https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fr.md
[NeTEx]: http://netex-cen.eu

## Installation

To install, you first need to install [PROJ] version 6.3.0.  See [PROJ
installation instructions].

[PROJ]: https://proj.org/
[PROJ installation instructions]: https://github.com/OSGeo/PROJ#installation

You also need the following dependencies to be installed.

```bash
apt install -y clang libssl-dev
```

As `ntfs2netexfr` is not pushed to crates.io yet, you can install it by cloning `transit_model`.

```bash
git clone https://github.com/CanalTP/transit_model
cd transit_model
cargo install --path ntfs2netexfr
```

## Usage

```bash
ntfs2netexfr --input /path/to/ntfs/folder/ --output /path/to/netexfr/ --participant CanalTP
```

* `--input` is the path to a folder containing NTFS data format
* `--output` is the path to a folder where the NeTEx France will be exported
* `--participant` is an identifier for the instigator of this NeTEx France
  export; it is exported in each NeTEx file

Get more information about the available options with `--help`.
