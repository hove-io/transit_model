# GTFS reading specification

## Purpose
This document aims to describe how the [GTFS format](https://developers.google.com/transit/gtfs/reference) is read in the Navitia Transit Model. To improve readability of this document, the specification will describe the transformation of a GTFS feed into a [NTFS feed](https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/) (which is a bunch of csv files accordingly to the memory Navitia Transit Model).

## Introduction
### Prepending data
The NTFS format introduce 2 objects to enable the manipulation of several datasets : contributors and datasets. Those two objects are not described here. The construction of NTFS objects IDs requires, for uniqueness purpose, that a unique prefix (specified for each source of data) needs to be included in every object's id.

In the following chapters, every id are prepend with this _prefix_ using this pattern : **\<prefix>:<object\_id>**.
The use of this specific pattern is shown explicitly using the value **ID** in the column _Constraint_ in the tables bellow.

### Defining how to read the GTFS file
In the GTFS, the route object may be considered as a Public Transport Line or as a Public Transport Route. In addition, a Route in NTFS is considered to be one way, so a GTFS Route may be transposed to either : 
* **ReadAsLine** : a NTFS Line, with 2 Routes (one going forward and on going backward, using the GTFS's trip direction_id property)
* **ReadAsRoute** : 1 or 2 NTFS Routes, depending on the GTFS's trip direction_id property. In this cas, a Line must be created.

The GTFS feed is read in the mode **ReadAsLine** if, for any agency_id, all GTFS routes contains unique `route_short_name`. If some GTFS Routes don't have `route_short_name`, they `route_long_name` property is considered instead.


## Mapping of objects between GTFS and NTFS
| GTFS object | NTFS object(s) |
| --- | --- |
| agency | network and company |
| route | line and route |
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


#### Loading Companies

| NTFS file | NTFS field | Constraint | GTFS file | GTFS field | Note |
| --- | --- | --- | --- | --- | --- |
| companies.txt | company_id | ID | agency.txt | agency_id | See above when not specified |
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
+ `stop_id` : the stop_id of the stop_point, with the followong pattern : **\<prefix>:Navitia:<stop_id of the stop_point>**
+ `stop_name` : the stop_name of the stop_point
+ `stop_lat` : the stop_lat of the stop_point
+ `stop_lon` : the stop_lon of the stop_point
+ `location_type` : fixed value "1" (to specify it's a stop_area)
The `parent_station` of the stop_point should then contain the generated `stop_area.id`.

(2) The `stop_code` field should be added as a complementary object_code with the following properties :
+ `object_type` : _stop_point_ or _stop_area_  accordingly to the `location_type` value
+ `object_id` : NTFS `stop_id` 
+ `object_system` : Fixed value "_gtfs_stop_code_"
+ `object_code` : value of the `stop_code` property

(3) The `comment` object is a complex type with additional properties : 
* `comment_id` : specify an identifier with the pattern **\<prefix>:stop:<stop_id of GTFS>**
* `comment_type` : specify the fixed value "Information"

(4) The `equipment` object is a complex type with additional properties : 
+ `equipment_id` : should be generated by the reader (with a **\<prefix>**). 
+ `wheelchair_boarding` : possible values are the same in both GTFS and NTFS
Be carefull to only create necessary equipments and avoid dupplicates.


### Reading routes.txt 
##### Mapping of route_type with modes
Only `route_type` existing in the GTFS feed are created. The priority is used to priorize the use of a commercial mode when creating Line grouping routes with different `route_type`s. This priorization follow the [Netex Specification](http://www.normes-donnees-tc.org/wp-content/uploads/2014/05/NF_Profil_NeTEx_pour_les_arrets-_F-_-_v2.pdf) in chapter 6.2.3 (and also indicated in the NTFS Specification).

| GTFS route_type | NTFS physical_mode ID (1) | NTFS commercial_mode ID (2) | NTFS commercial_mode name | Priority |
| --- | --- | --- | --- | --- |
| 0 | RailShuttle | 0 | Tram, Streetcar, Light rail | 3 |
| 1 | Metro | 1 | Subway, Metro | 4 |
| 2 | Train | 2 | Rail | 2 |
| 3 | Bus | 3 | Bus | 8 |
| 4 | Ferry | 4 | Ferry | 1 |
| 5 | Funicular | 5 | Cable car | 6 |
| 6 | Funicular | 6 | Gondola, Suspended cable car | 7 |
| 7 | Funicular | 7 | Funicular | 5 |

(1) THe physical_mode ID is a mapping with a specific value as described in the NTFS format specification. No prefix is expected.

(2) The commercial_mode ID has to be prefixed as stated in the Introduction chapter.

#### Loading Lines in ReadAsLine mode
_Warning :_ If the GTFS route has no trips, the Line should NOT be created and a warning is logged.


| NTFS file | NTFS field | Constraint | GTFS file | GTFS field | Note |
| --- | --- | --- | --- | --- | --- |
| lines.txt | network_id | Required |  |  | This field should contain the `network.id` corresponding to the `agency_id` of the routes. |
| lines.txt | line_id | ID | routes.txt | route_id |  |
| lines.txt | line_code | Optionnal | routes.txt | route_short_name |  |
| lines.txt | line_name | Required | routes.txt | route_long_name |  |
| lines.txt | line_color | Optionnal | routes.txt | route_color | if several values are available, a warning is logged and the color of the smallest `route_id` is used |
| lines.txt | line_text_color | Optionnal | routes.txt | route_text_color | same as line_color |
| lines.txt | line_sort_order | Optionnal | routes.txt | route_sort_order |  |
| lines.txt | commercial_mode_id | Required | routes.txt | route_type | See "Mapping of route_type with modes" chapter |
| comments.txt | comment_value | Optionnal | routes.txt | route_desc  | See (1) for additionnal properties |

(1) The `comment` object is a complex type with additional properties : 
* `comment_id` : specify an identifier with the pattern **\<prefix>:route:<route_id of GTFS>**
* `comment_type` : specify the fixed value "Information"

#### Loading Lines in ReadAsRoute mode
A Line is created to group the GTFS Routes when they have the same `agency_id` and the same `route_short_name` (or `route_long_name` if the latter is empty).

_Warning :_ If all the GTFS routes corresponding to the Line have no trips, the Line should NOT be created and a warning is logged.

| NTFS file | NTFS field | Constraint | GTFS file | GTFS field | Note |
| --- | --- | --- | --- | --- | --- |
| lines.txt | network_id | Required |  |  | This field should contain the `network.id` corresponding to the `agency_id` of the routes. |
| lines.txt | line_id | ID | routes.txt | route_id | (1) |
| lines.txt | line_code | Optionnal | routes.txt | route_short_name |  |
| lines.txt | line_name | Required | routes.txt | route_long_name | `route_long_name` of the merged Route with the smallest `route_id` |
| lines.txt | line_color | Optionnal | routes.txt | route_color | if several values are available, a warning is logged and the color of the smallest `route_id` is used |
| lines.txt | line_text_color | Optionnal | routes.txt | route_text_color | same as line_color |
| lines.txt | line_sort_order | Optionnal | routes.txt | route_sort_order |  |
| lines.txt | commercial_mode_id | Required | routes.txt | route_type | (2) |

(1) Construction of the line_id : 
* if `route_short_name` is specified, use the pattern **<agency_id>:<route_short_name>**
* if `route_short_name` is empty, use the pattern **<agency_id>:<route_id>** using the smallest `route_id` of the grouped routes

(2) When several `route_type`s are grouped together, the commercial_mode_id with the smallest priority should be used (as specified above).

#### Loading Routes in ReadAsLine mode
A Route is created for each direction of existing trips.
_Warning :_ If the GTFS route has no trips, the Route should NOT be created and a warning is logged.

| NTFS file | NTFS field | Constraint | GTFS file | GTFS field | Note |
| --- | --- | --- | --- | --- | --- |
| routes.txt | route_id | ID | routes.txt | route_id | postpend a `_R` suffix for the Route grouping trips with `direction_id` = 1 (no suffix for `0` or undefined `direction_id`) |
| routes.txt | route_name | Required |  |  | (1) | 
| routes.txt | direction_type | Optionnal |  |  | (2) |
| routes.txt | line_id | Required | routes.txt | route_id |  |
| routes.txt | destination_id | Optionnal |  |  | This field contains a stop_area.id of the most frequent destination of the contained trips (ie. the parent_station of the most frequent last stop of trips) |


(1) Construction of the `route_name` :  
+ If the current GTFS Route contains only one way trips, the `route_name` property is filled with the GTFS `route_long_name` property.  
+ If there are two directions, each `route_name` should be constructed with the following pattern : *"[Origin stop_area]-[Destination stop_area]"* accordingly to the `direction_id`

(2) the field `direction_type` contains `backward` when grouping GTFS Trips with `direction_id` = 1, `forward` otherwise

#### Loading Routes in ReadAsRoute mode
A Route is created for each direction of existing trips.
_Warning :_ If the GTFS route has no trips, the Route should NOT be created and a warning is logged.

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

