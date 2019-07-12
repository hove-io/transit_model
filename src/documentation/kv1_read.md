# KV1 reading specification
## Introduction
This document describes how a KV1 feed is read in Navitia Transit model (NTM) and transformed into a [NTFS feed](https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fr.md).

For the sake of simplicity, the following specification describes only those NTFS fields that are specified in the source data (e.g. the `network_url` is not specified and therefore not detailed.)

In order to guarantee that the NTFS objects identifiers are unique and stable, each object id is prefixed with a unique prefix (specified for each datasource), following the general pattern `<prefix>:<id>`.

## Mapping between KV1 and NTFS objects
### networks.txt
NTFS field | KV1 file | KV1 field | Mapping rule/Comment
--- | --- | --- | ---
network_id | LINEXXXXXX.TMI | *DataOwnerCode* | This field is prefixed.
network_name | LINEXXXXXX.TMI | *DataOwnerCode* |
network_timezone | | | Fixed value `Europe/Amsterdam`

### companies.txt
NTFS field | KV1 file | KV1 field | Mapping rule/Comment
--- | --- | --- | ---
company_id | LINEXXXXXX.TMI | *DataOwnerCode* | This field is prefixed.
company_name | LINEXXXXXX.TMI | *DataOwnerCode* |

### stops.txt
#### For stop_points

NTFS field | KV1 file | KV1 field | Mapping rule/Comment
--- | --- | --- | ---
stop_id | USRSTOPXXX.TMI | *UserstopCode* | This field is prefixed.
stop_name | USRSTOPXXX.TMI | *Name* |
location_type |  |  | Fixed value `0`.
stop_lat | POINTXXXXX.TMI | *LocationY_NS* | See below for the link of a Point with a UserStop.
stop_lon | POINTXXXXX.TMI | *LocationX_EW* | See below for the link of a Point with a UserStop.
parent_station | USRSTOPXXX.TMI | *UserStopAreaCode* |
platform_code | USRSTOPXXX.TMI | *StopSideCode* |

**Defining the coordinate of a stop_point :**

The latitude/longitude of a stop_point correspond to the fields *LocationX_EW*, *LocationY_NS* in the file POINTXXXXX.TMI of the point whose *PointCode* matches *UserstopCode* and *PointType* equals the value `SP`. The input coordinate system Amersfoort / RD New (EPSG:28992) should be converted to WGS84 (EPSG:4326).

#### For stop_areas

NTFS field | KV1 file | KV1 field | Mapping rule/Comment
--- | --- | --- | ---
stop_id | USRSTARXXX.TMI | *UserStopAreaCode* | This field is prefixed.
stop_name | USRSTARXXX.TMI | *Name* |
stop_lat |  |  | The coordinates of the stop_area are computed as the barycenter of all the associated stop_points.
stop_lon |  |  | The coordinates of the stop_area are computed as the barycenter of all the associated stop_points.
location_type |  |  | Fixed value `1`.

### lines.txt
NTFS field | KV1 file | KV1 field | Mapping rule/Comment
--- | --- | --- | ---
line_id | LINEXXXXXX.TMI | *LinePlanningNumber* | This field is prefixed.
line_code | LINEXXXXXX.TMI | *LinePublicNumber* |
line_name |  |  | This field is computed using the name of the first associated Route in the forward direction. If several forward routes are available, the one with the smallest `route_id` is used.
forward_line_name |  |  | Same value as `line_name`
forward_direction |  |  | This field should have the same value as the `destination_id` of the route used to provide the `line_name` value.
backward_line_name |  |  | This field is computed using the name of the first associated Route in the backward direction. If several backward routes are available, the one with the smallest `route_id` is used.
backward_direction |  |  | This field should have the same value as the `destination_id` of the route used to provide the `backward_line_name` value.
line_color | LINEXXXXXX.TMI | *LineColor* |
network_id | LINEXXXXXX.TMI | *DataOwnerCode* | This field is prefixed. Link to the file [networks.txt](#networkstxt).
commercial_mode_id | LINEXXXXXX.TMI | *TransportType* | This field is not prefixed. Link to the file [commercial_modes.txt](#commercialmodestxt).

### routes.txt
A Route is created from a line and a direction stated in the JOPAXXXXXX.TMI file.

NTFS field | KV1 file | KV1 field | Mapping rule/Comment
--- | --- | --- | ---
route_id | JOPAXXXXXX.TMI | *LinePlanningNumber*, *Direction* | This field is prefixed. Concatenation of *LinePlanningNumber* and *Direction* separated by a `:`. Ex: "\<prefix\>:2029:2"
route_name | JOPAXXXXXX.TMI |  | "[first stop of the first trip] - [last stop of the first trip]" (1)
direction_type | JOPAXXXXXX.TMI | *Direction* | `forward` value when *Direction* is `1` or `A`. `backward` in all other cases.
line_id | JOPAXXXXXX.TMI | *LinePlanningNumber* | This field is prefixed. Link to the file [lines.txt](#linestxt).
destination_id |  |  | `stop_id` of the stop_area of the last stop of the first trip.  (1)

(1) The first trip of a route is the one with the smallest `trip_id` value.

### calendar.txt and calendar_dates.txt
This document specifies a straightforward NTFS conversion using only `calendar_dates.txt` file. The resulting files may be different with the use of an optimizing processing, but the result should be functionnaly identical.

NTFS field | KV1 file | KV1 field | Mapping rule/Comment
--- | --- | --- | ---
service_id | OPERDAYXXX.TMI | *OrganizationalUnitCode*, *ScheduleCode*, *ScheduleTypeCode* | This field is prefixed. Concatenation of the 3 specified fields separated by a ':'. Ex: "\<prefix\>:2029:1:1"
date | OPERDAYXXX.TMI | ValidDate | Service date to be transformed into the YYYYMMDD format.
exception_type |  |  | Fixed value `1`.

### trips.txt
Trips are described in the `PUJOPASSXX.TMI` file. The `Direction` field of the `JOPAXXXXXX.TMI` is required to link a trip to it's corresponding route. The mapping between the two file is made by the fields `JourneyPatternCode` and `LinePlanningNumber`.

NTFS field | KV1 file | KV1 field | Mapping rule/Comment
--- | --- | --- | ---
route_id | JOPAXXXXXX.TMI | *LinePlanningNumber*, *Direction* | This field is prefixed. Link to the files [routes.txt](#routestxt). Concatenation of *LinePlanningNumber* and *Direction* separated by a `:`. Ex: "\<prefix\>:2029:2"
service_id | PUJOPASSXX.TMI | *OrganizationalUnitCode*, *ScheduleCode*, *ScheduleTypeCode* | This field is prefixed. Link to the file [calendar_dates.txt](#calendardatestxt). Concatenation of the 3 specified fields separated by a ':'. Ex: "\<prefix\>:2029:1:1"
trip_id | JOPAXXXXXX.TMI, PUJOPASSXX.TMI | *LinePlanningNumber*, *JourneyPatternCode*, *JourneyNumber*, *ScheduleCode* | This field is prefixed. Concatenation of the 4 specified fields separated by a `:`. Ex: "\<prefix\>:2029:9001:23366:1"
company_id | JOPAXXXXXX.TMI | *DataOwnerCode* | This field is prefixed. Link to the file [companies.txt](#companiestxt).
physical_mode_id | LINEXXXXXX.TMI | *TransportType* | This field is not prefixed. Link to the file physical_modes.txt of the NTFS. It is computed using the *TransportType* specified for the associated line of the trip.
trip_properties.wheelchair_accessible | PUJOPASSXX.TMI | *WheelChairAccessible* | The trip is considered accessible if the value is `ACCESSIBLE` for all stop_times of the trip. The trip is considered not accessible if the value is `NOTACCESSIBLE` for all stop_times of the trip. The information on a trip's accessibility is considered unknown if the value is `UNKNOWN` for at least one stop_time of the trip or in case the value is the same for all stop_times of the trip.

#### Mapping of TransportType with NTFS modes

The possible values of the *TransportType* field are directly mapped to the NTFS modes according to the following table. Note that neither the physical_mode_id nor the commercial_mode_id fields are prefixed.

TransportType in KV1 | physical_mode_id in NTFS | physical_mode_name in NTFS | commercial_mode_id in NTFS | commercial_mode_name in NTFS
--- | --- | --- | --- | ---
BUS | Bus | Bus | Bus | Bus
TRAIN | Train | Train | Train | Train
METRO | Metro | Metro | Metro | Metro
TRAM | Tramway | Tramway | Tramway | Tramway
BOAT | Ferry | Ferry | Ferry | Ferry

### stop_times.txt
NTFS field | KV1 file | KV1 field | Mapping rule/Comment
--- | --- | --- | ---
trip_id | PUJOPASSXX.TMI | *LinePlanningNumber*, *JourneyPatternCode*, *JourneyNumber* | This field is prefixed. Link to the file [trips.txt](#tripstxt). Concatenation of the 3 specified fields separated by a `:`. Ex: "\<prefix\>:2029:9001:23366"
arrival_time | PUJOPASSXX.TMI | *TargetArrivalTime* |
departure_time | PUJOPASSXX.TMI | *TargetDepartureTime* |
stop_id | PUJOPASSXX.TMI | *UserStopCode* | This field is prefixed. Link to the file [stops.txt](#stopstxt).
stop_sequence | PUJOPASSXX.TMI | *StopOrder* |

### comments.txt
NTFS field | KV1 file | KV1 field | Mapping rule/Comment
--- | --- | --- | ---
comment_id | NOTICEXXXX.TMI | *Notice coder* | This field is prefixed.
comment_name | NOTICEXXXX.TMI | *Notice (content)* |

### comment_links.txt
Only comments on trips (`Object` field specified with `PUJOPASS`) will be handled.

NTFS field | KV1 file | KV1 field | Mapping rule/Comment
--- | --- | --- | ---
object_id | NTCASSGNMX.TMI | *LinePlanningNumber*, *TripNumber* | This field is prefixed. Link to the file [trips.txt](#tripstxt) using the *TripNumber* as a *JourneyNumber* field (the *JourneyPatternCode* is not available).
object_type |  |  | Fixed value `trip`.
comment_id | NTCASSGNMX.TMI | *Notice code* | This field is prefixed. Link to the file [comments.txt](#commentstxt).
