# KV1 reading specification
## Introduction
This document describes how a KV1 feed is read in Navitia Transit model (NTM) and transformed into a [NTFS feed](https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/ntfs_fr.md).

For the sake of simplicity, the follownig specification describes only those NTFS fields that are specified in the source data (e.g. the `network_url` is not specified and therefore not detailed.)

In order to guarantee that the NTFS objects identifiers are unique and stable, each object id is prefixed with a unique prefix (specified for each datasource), following the general pattern \<prefix>:\<id>.

## Mapping between KV1 and NTFS objects
### calendar_dates.txt
### commercial_modes.txt
### companies.txt
NTFS field | KV1 file | KV1 field | Mapping rule/Comment
--- | --- | --- | ---
company_id | LINEXXXXXX.TMI | *DataOwnerCode* | This field is prefixed.
company_name | LINEXXXXXX.TMI | *DataOwnerCode* | 

### feed_infos.txt
### lines.txt
NTFS field | KV1 file | KV1 field | Mapping rule/Comment
--- | --- | --- | ---
line_id | LINEXXXXXX.TMI | *LinePlanningNumber* | This field is prefixed.
line_code | LINEXXXXXX.TMI | *LinePublicNumber* | 
line_name |  |  | Name to be derived from the line origin-destination. See the mapping rule below. +++
forward_line_name |  |  | Name to be derived from the line destination. See the mapping rule below. ++++
forward_direction |  |  | Last stop (forward direction) +++
backward_line_name |  |  | Name to be derived from the line origin. See the mapping rule below. +++
backward_direction |  |  | Last stop (backward direction) +++
line_color | LINEXXXXXX.TMI | *LineColor* | 
network_id | LINEXXXXXX.TMI | *DataOwnerCode* | This field is prefixed. Link to the file [networks.txt](#networkstxt).
commercial_mode_id | LINEXXXXXX.TMI | *TransportType* | This field is not prefixed. Link to the file [commercial_modes.txt](#comercialmodestxt).
geometry_id |  |  | ???

### networks.txt
NTFS field | KV1 file | KV1 field | Mapping rule/Comment
--- | --- | --- | ---
network_id | LINEXXXXXX.TMI | *DataOwnerCode* | This field is prefixed.
network_name | LINEXXXXXX.TMI | *DataOwnerCode* | 

### physical_modes.txt
### routes.txt
### stops.txt
### stop_times.txt
### transfers.txt
cf. LINKXXXXXX.TMI 
### trips.txt