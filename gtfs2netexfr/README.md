`gtfs2netexfr`
=====

Command-Line Interface to convert [GTFS] data format into [NeTEx] France data
format.

[GTFS]: https://developers.google.com/transit/gtfs/reference/
[NeTEx]: http://netex-cen.eu/

# Installation
To install, you first need to install [PROJ] version 6.3.0.  See [PROJ
installation instructions].

[PROJ]: https://proj.org/
[PROJ installation instructions]: https://github.com/OSGeo/PROJ#installation

You also need the following dependencies to be installed.
```bash
apt install -y clang libssl-dev
```

Finally, you can install `gtfs2netexfr` with
```bash
cargo install gtfs2netexfr
```

# Usage

```bash
gtfs2netexfr --input /path/to/gtfs/folder/ --output /path/to/netexfr/ --participant CanalTP
```

- `--input` is the path to a folder containing GTFS data format
- `--output` is the path to a folder where the NeTEx France will be exported
- `--participant` is an identifier for the instigator of this NeTEx France
  export; it is exported in each NeTEx file

Get more information about the available options with `--help`.
