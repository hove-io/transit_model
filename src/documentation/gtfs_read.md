# GTFS reading specification

## Purpose
This document aims to describe how the [GTFS format](https://developers.google.com/transit/gtfs/reference) is read in the Navitia Transit Model. To improve readability of this document, the specification will describe the transformation of a GTFS feed into a [NTFS feed](https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/) (which is a bunch of csv files accordingly to the memory Navitia Transit Model).

## Introduction
### Prepending data
The NTFS format introduce 2 objects to enable the manipulation of several datasets : contributors and datasets. Those two objects are not described here. The construction of NTFS objects IDs requires, for uniqueness purpose, that a unique prefix (specified for each source of data) needs to be included in every object's id.

In the following chapters, identifiers may be prepend with this _prefix_ using this pattern : **\<prefix>:<object\_id>**.
The use of this specific pattern is shown explicitly using the value **ID** in the column _Constraint_ in the tables bellow.

## Mapping of objects between GTFS and NTFS
| GTFS object | NTFS object(s) |
| --- | --- |
| agency | network and company |
| route | line, route, physical_mode, commercial_mode |
| trip | route and trip |
| stop_time | stop_time |

## Detailed mapping of objects
### Reading agency.txt 
The field "agency_id" may not be provided in the GTFS as it's an optionnal field. 
* If there is only one agency, the "agency_id" is considered to be "1".
* If there are several agencies, the program will raise an exception as it won't be able to choose the the right agency for the routes.

#### Loading Networks

| NTFS file | NTFS field | Constraint | GTFS file | GTFS field | Note |
| --- | --- | --- | --- | --- | --- |
| networks.txt | network_id | ID | agency.txt | agency_id | See above when not specified |
| networks.txt | network_name | Required | agency.txt | agency_name |  |
| networks.txt | network_url | Optionnal | agency.txt | agency_url |  |
| networks.txt | network_timezone | Optionnal | agency.txt | agency_timezone | |
| networks.txt | network_lang | Optionnal | agency.txt | agency_lang |  |
| networks.txt | network_phone | Optionnal | agency.txt | agency_phone |  |

**_"Source" complementary code :_** 

A complementary object_code should be added to each network with the following properties : 
* `object_type` : the fixed value `network`
* `object_id` : the value of the `network_id` field
* `object_system` : the fixed value `source`
* `object_code` : the unmodified value of `agency_id` (or `1` if the value is not provided as stated above)


#### Loading Companies

| NTFS file | NTFS field | Constraint | GTFS file | GTFS field | Note |
| --- | --- | --- | --- | --- | --- |
| companies.txt | company_id | ID | agency.txt | agency_id | `1` if the value is not provided (same rule as networks) |
| companies.txt | company_name | Required | agency.txt | agency_name |  |
| companies.txt | company_url | Optionnal | agency.txt | agency_lang |  |
| companies.txt | company_phone | Optionnal | agency.txt | agency_phone |  |

### Reading stops.txt 
Like the GTFS, the NTFS group stop_points and stop_areas in on file : stops.txt.

| NTFS file | NTFS field | Constraint | GTFS file | GTFS field | Note |
| --- | --- | --- | --- | --- | --- |
| stops.txt | stop_id | ID | stops.txt | stop_id |  |
| object_codes.txt | object_code | Optionnal | stops.txt | stop_code | This GTFS property is stored as an associated code for this stop. See (2) for complementary properties. |
| stops.txt | stop_name | Required | stops.txt | stop_name |  |
| stops.txt | stop_lat | Required | stops.txt | stop_lat |  |
| stops.txt | stop_lon | Required | stops.txt | stop_lon |  |
| stops.txt | location_type | Optionnal | stops.txt | location_type |  |
| stops.txt | parent_station | Optionnal | stops.txt | parent_station | (1) |
| stops.txt | stop_timezone | Optionnal | stops.txt | stop_timezone |  |
| comments.txt | comment_value | Optionnal | stops.txt | stop_desc | See (3) for additionnal properties |
| equipments.txt | wheelchair_boarding | Optionnal | stops.txt | wheelchair_boarding | See (4) for detailed info. |


(1) If the `parent_station` field of a stop_point (`location_type` = 0 or empty) is missing or empty, then a stop_area should be created, using the following properties : 
+ `stop_id` : the stop_id of the stop_point, with the followong pattern : **\Navitia:<stop_id of the stop_point>**
+ `stop_name` : the stop_name of the stop_point
+ `stop_lat` : the stop_lat of the stop_point
+ `stop_lon` : the stop_lon of the stop_point
+ `location_type` : fixed value "1" (to specify it's a stop_area)
The `parent_station` of the stop_point should then contain the generated `stop_area.id`.

(2) The `stop_code` field should be added as a complementary object_code with the following properties :
+ `object_type` : _stop_point_ or _stop_area_  accordingly to the `location_type` value
+ `object_id` : NTFS `stop_id` 
+ `object_system` : Fixed value "source"
+ `object_code` : value of the `stop_code` property

(3) The `comment` object is a complex type with additional properties : 
* `comment_id` : specify an identifier with the pattern **\stop:<stop_id of GTFS>**
* `comment_type` : specify the fixed value "information"

(4) The `equipment` object is a complex type with additional properties : 
+ `equipment_id` : should be generated by the reader. 
+ `wheelchair_boarding` : possible values are the same in both GTFS and NTFS
Be carefull to only create necessary equipments and avoid dupplicates.

**_"Source" complementary code :_** 

A complementary object_code should be added to each stop with the following properties : 
* `object_type` : the fixed value `stop_point` or `stop_area` (depending on the object)
* `object_id` : the value of the `stop_id` field
* `object_system` : the fixed value `source`
* `object_code` : the unmodified value of `agency_id` (or `1` if the value is not provided as stated above)

### Reading routes.txt 
##### Mapping of route_type with modes
Only `route_type` existing in the GTFS feed are created. The priority is used to priorize the use of a commercial mode when creating Line grouping routes with different `route_type`s. This priorization follow the [Netex Specification](http://www.normes-donnees-tc.org/wp-content/uploads/2014/05/NF_Profil_NeTEx_pour_les_arrets-_F-_-_v2.pdf) in chapter 6.2.3 (and also indicated in the NTFS Specification).

| GTFS route_type | NTFS physical_mode ID (1) | NTFS commercial_mode ID | NTFS commercial_mode name | Priority |
| --- | --- | --- | --- | --- |
| 0 | RailShuttle | 0 | Tram, Streetcar, Light rail | 3 |
| 1 | Metro | 1 | Subway, Metro | 4 |
| 2 | Train | 2 | Rail | 2 |
| 3 | Bus | 3 | Bus | 8 |
| 4 | Ferry | 4 | Ferry | 1 |
| 5 | Funicular | 5 | Cable car | 6 |
| 6 | Funicular | 6 | Gondola, Suspended cable car | 7 |
| 7 | Funicular | 7 | Funicular | 5 |

(1) The physical_mode ID is a mapping with a specific value as described in the NTFS format specification. This value must not be prefixed.

#### Loading Routes
A Route is created for each direction of existing trips.
_Warning :_ If the GTFS route has no trips, the Navitia Route should NOT be created and a warning should be logged.

| NTFS file | NTFS field | Constraint | GTFS file | GTFS field | Note |
| --- | --- | --- | --- | --- | --- |
| routes.txt | route_id | ID | routes.txt | route_id | postpend a `_R` suffix for the Route grouping trips with `direction_id` = 1 (no suffix for `0` or undefined `direction_id`) |
| routes.txt | route_name | Required | routes.txt | route_long_name | if `route_long_name` is empty, use `route_short_name` | 
| routes.txt | direction_type | Optionnal |  |  | (1) |
| routes.txt | line_id | Required |  |  | corresponding `line.id` (see Line construction above) |
| routes.txt | destination_id | Optionnal |  |  | This field contains a stop_area.id of the most frequent destination of the contained trips (ie. the parent_station of the most frequent last stop of trips) |
| comments.txt | comment_value | Optionnal | routes.txt | route_desc  | See (2) for additionnal properties |

(1) the field `direction_type` contains `backward` when grouping GTFS Trips with `direction_id` = 1, `forward` otherwise

(2) The `comment` object is a complex type with additional properties : 
* `comment_id` : specify an identifier with the pattern **\<prefix>:route:<route_id of GTFS>**
* `comment_type` : specify the fixed value "Information"

#### Loading Lines 
A Navitia Line is created to group one or several Navitia Routes when they are created with the same gtfs `agency_id` and the same `route_short_name` (or `route_long_name` if the latter is empty).


| NTFS file | NTFS field | Constraint | GTFS file | GTFS field | Note |
| --- | --- | --- | --- | --- | --- |
| lines.txt | network_id | Required |  |  | This field should contain the `network.id` corresponding to the `agency_id` of the routes. |
| lines.txt | line_id | ID | routes.txt | route_id | Use the smallest `route_id` of the grouped gtfs Route |
| lines.txt | line_code | Optionnal | routes.txt | route_short_name |  |
| lines.txt | line_name | Required | routes.txt |  | The Navitia `route_name` of the Route with the smallest `route_id` (as a string) is used. |
| lines.txt | line_color | Optionnal | routes.txt | route_color | if several values are available, a warning is logged and the color of the smallest `route_id` is used |
| lines.txt | line_text_color | Optionnal | routes.txt | route_text_color | same as line_color |
| lines.txt | line_sort_order | Optionnal | routes.txt | route_sort_order |  |
| lines.txt | commercial_mode_id | Required | routes.txt | route_type | See "Mapping of route_type with modes" chapter (1). |

(1) When several GTFS Routes with different `route_type`s are grouped together, the commercial_mode_id with the smallest priority should be used (as specified in chapter "Mapping of route_type with modes").

### Reading trips.txt 

| NTFS file | NTFS field | Constraint | GTFS file | GTFS field | Note |
| --- | --- | --- | --- | --- | --- |
| trips.txt | route_id | Required | trips.txt | route_id | cf. NTFS `route_id` definition above to specify the propert reference. |
| trips.txt | service_id | Required | trips.txt | service_id |  |
| trips.txt | trip_id | Required | trips.txt | trip_id |  |
| trips.txt | trip_headsign | Optionnal | trips.txt |  | `trip_short_name`, of if empty `trip_headsign` |
| trips.txt | block_id | Optionnal | trips.txt | block_id |  |
| trips.txt | company_id | Required | routes.txt | agency_id | The company corresponding to the agency_id of the trip's route_id |
| trips.txt | physical_mode_id | Required |  |  | use the `route_type` See "Mapping of route_type with modes" chapter |
| trips.txt | trip_property_id | Optionnal | trips.txt |  | (1) |
| trips.txt | dataset_id | Required |  |  | The `dataset_id` provided (cf. [gtfs2ntfs.md](./gtfs2ntfs.md) ) |
| trips.txt | geometry_id | Optionnal | trips.txt | shape_id |  |

(1) The `trip_property` object is a complex type with additional properties : 
+ `trip_property_id`: should be generated by the reader. 
+ `wheelchair_accessible`: possible values are the same in both GTFS and NTFS  
+ `bike_accepted`: corresponding to the GTFS `bikes_allowed` property. Possible values are the same in both GTFS and NTFS.  
Be carefull to only create necessary trip_properties and avoid dupplicates.


### Reading stop_times.txt 

| NTFS file | NTFS field | Constraint | GTFS file | GTFS field | Note |
| --- | --- | --- | --- | --- | --- |
| stop_times.txt | trip_id | Required | stop_times.txt | trip_id |  |
| stop_times.txt | arrival_time | Optionnal | stop_times.txt | arrival_time | If not specified, see (1) |
| stop_times.txt | departure_time | Optionnal | stop_times.txt | departure_time | If not specified, see (1) |
| stop_times.txt | stop_id | Required | stop_times.txt | stop_id |  |
| stop_times.txt | stop_sequence | Required | stop_times.txt | stop_sequence |  |
| stop_times.txt | stop_headsign | Optionnal | stop_times.txt | stop_headsign |  |
| stop_times.txt | pickup_type | Optionnal | stop_times.txt | pickup_type |  |
| stop_times.txt | drop_off_type | Optionnal | stop_times.txt | drop_off_type |  |
| stop_times.txt | date_time_estimated | Optionnal | stop_times.txt | timepoint | GTFS and NTFS values are inverted. See (2) |

(1) GTFS `arrival_time` and `departure_time` should contain values.
* if both of them are empty :
    * if the stop_time is the first or the last of the trip, an error shoud be logged and the stop_time ignored
    * if not, the time should be interpolated (see below).     
* if one of them is empty, a warning should be logged and the value of the other field should be copied to the empty one.

**Interpolation**
If a stop_time needs to be interpolated : 
* collect the nearest preceding stop_time and the nearest following stop_time containing a valid time value
* apply a simple distribution for all the intermediate stop_times
For exemple : 

| GTFS passing time | NTFS Extrapolated time |
| --- | --- |
| 9:00 | 9:00 |
| - | 9h30 |
| - | 10:00 |
| 10:30 | 10:30 |

(2) The GTFS `timepoint` conversion tules for NTFS `date_time_estimated` are :
* if `timepoint` is unspecified => `date_time_estimated` equals 0
* if `timepoint` equals 1 => `date_time_estimated` equals 0
* if `timepoint` equals 0 => `date_time_estimated` equals 1
