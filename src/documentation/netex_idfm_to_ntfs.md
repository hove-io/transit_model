# Netex IDFM reading specification
## Introduction
This document describes how a Netex feed provided by Ile-de-France Mobilités is read in Navitia Transit model (NTM)
and transformed into a [NTFS feed](https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fr.md).

For the sake of simplicity, the NTM properties that are not specified in the source
data are not described below.

In order to guarantee that the NTFS objects identifiers are unique and stable, each
object id is prefixed with a unique prefix (specified for each datasource), following
the general pattern `<prefix>:<id>`.

## Input data description
This specifications assumes that all the requeried data (time tables for all the lines, stop points and stop areas, transfers, etc.) are provided in one ZIP archive (aka "FICHIERS OFFRE") described in the specification document "NT60-A150701-v1.11-BO-STIF_-_Specification_Technique_d_Interface_NeTEx_pour_la_publication_20190624.docx".

The ZIP archive contains contains: 
- a **lignes.xml** file, containing the description of lines, networks and companies
- a **correspondances.xml** file
- a **arrets.xml** file
- a folder for each operator (or company) containing:
  + a **calendriers.xml** file, containing the calendars (or validity patterns) used by trips in files **offre_**
  + a **commun.xml** file optionnal, containing the comments referenced by the operator objects (if needed)
  + a file starting with **offre_** describing the time tables of a specific line

In this document, versions of objects are not handled. The first encountered object description is considered when creating an object.

## Mapping between Netex-IDFM elements and NTFS objects
Each XML files contains a `PublicationDelivery` node, containing a `dataObjects` node. Descriptions below is considering nodes inside this `dataObjects` node.

### networks.txt
`networks` are provided in **lignes.xml** file in the nodes **CompositeFrame/frames/ServiceFrame/Network**. There could be multiple `ServiceFrame` to read.

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
network_id | *Network/@id* | This field is prefixed. 
network_name | *Network/Name* | 
network_timezone | | Fixed value `Europe/Paris`.

### companies.txt
`companies` are provided in the **lignes.xml** file in the node **CompositeFrame/frames/ResourceFrame/organisations/**. There could be multiple `Operator` to read.

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
company_id | *Operator/@id* | This field is prefixed. 
company_name | *Operator/Name* | 

### stops.txt
`stops` are provided in the **arrets.xml** file in the node **CompositeFrame/frames/GeneralFrame/** (only one **CompositeFrame** is expected). 
In this netex feed, a `Quay` in included in a "ZDL" `StopPlace`, this "ZDL" `StopPlace` could be included in a "LDA" `StopPlace`.
This connector assumes that all the "ZDL" `StopPlace` are included in one (and only one) "LDA" `StopPlace`.

#### For stop_areas
`stop_area` objects are `StopPlace` nodes of the **arrets.xml** file created in this order:
1. `StopPlace` with a `placeTypes/TypeOfPlace/@ref` attribute with a "LDA" value are used
2. Other `StopPlace` with no `ParentSiteRef` node (corresponding to "ZDL" stops not included in a LDA)

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
stop_id | *StopPlace/@id* | This field is prefixed. 
stop_name | *StopPlace/Name* | 
location_type | | Fixed value `1` (stop_area)
stop_lat | Quay/Centroid/Location | see (1) below
stop_lon | Quay/Centroid/Location | see (1) below

(1) Definition of stop_lat et stop_lon:
As this Netex feed does not provide coordinates for the stop_areas, the stop_lat et stop_lon fields will be set with the coordinate of the centroid of all included stop_points.

(2) Complementary object_properties
A complementary property `Netex_StopType` is added to the stop_area with either the "LDA" or "ZDL" value.

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
object_type |  | fixed value `stop` 
object_id | *StopPlace/@id* | This field is prefixed. 
object_property_name |  | fixed value `Netex_StopType`.
object_property_value | *StopPlace/placeTypes/TypeOfPlace/@ref* | 

(3) Complementary object_codes
A complementary code `Netex_ZDL` is added to "LDA" stop_area for each included "ZDL" (ie. `StopPlace` with `ParentSiteRef` node).

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
object_type |  | fixed value `stop` 
object_id | *StopPlace/ParentSiteRef/@ref* | This field is prefixed. 
object_system |  | fixed value `Netex_ZDL`.
object_code | *StopPlace/@id* | This field is prefixed. 


#### For stop_points
`stop_point` objects are `Quay` nodes ("ZDE objects") in the **arrets.xml** file in the node **CompositeFrame/frames/GeneralFrame/**.

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
stop_id | *Quay/@id* | This field is prefixed. 
stop_name | *Quay/Name* | 
location_type | | Fixed value `0` (stop_point)
stop_lat | Quay/Centroid/Location | see (1) below
stop_lon | Quay/Centroid/Location | see (1) below
parent_station | | stop_id of the corresponding `stop_area`, see (2) below.
stop_timezone | | fixed value `Europe/Paris`
equipment_id | | This value will be self generated by the connector if an equipment object is necessary. See (2) below.

(1) Definition of stop_lat et stop_lon:
The `Quay/Centroid/Location` node contains a position (X, Y) with an EPSG described, for exemple: 
`<gml:pos srsName="EPSG:2154">662233.0 6861519.0</gml:pos>`
This coordinate si to be converted to a WGS84 coordinate (EPSG:4326).

(2) Definition of the parent_station:
A ZDL `StopPlace` includes the referencies of it's ZDE in `StopPlace/quays/QuayRef` nodes. The parent_station of a `stop_point` can be either: 
* a ZDL `stop_area` referencing it's `Quay`
* a LDA `stop_area` contain it's ZDL referencies as complementary object_codes, each ZDL referencing it's `Quay`


**Definition of the Accessibility of a stop_point**
If the `Quay` node contains a `AccessibilityAssessment/MobilityImpairedAccess` node, an equipement will be generated with :
- a self generated and prefixed `equipment_id`,
- a `wheelchair_boarding` property set to: 
  + Fixed value `1` (accessible) if `MobilityImpairedAccess` has the value `true`,
  + Fixed value `2` (not accessible) if `MobilityImpairedAccess` has the value `true`,
  + Fixed value `0` (unknown) if `MobilityImpairedAccess` has any other value (`partial` or `unknown` for exemple).


### commercial_modes.txt and physical_modes.txt
The transport mode in Netex-IDFM is only defined at the Line level, in the **lines.xml** file in the node **CompositeFrame/frames/ServiceFrame/lines/Line/**.
`physical_mode_id` and `commercial_mode_id` are **not** prefixed.

TransportMode in Netex-IDFM | physical_mode_id | physical_mode_name | commercial_mode_id | commercial_mode_name 
--- | --- | --- | --- | ---
air | Air | Avion | Air | Avion
bus | Bus | Bus | Bus | Bus
coach | Coach | Autocar | Coach | Autocar
ferry | Ferry | Ferry | Ferry | Ferry
metro | Metro | Métro | Metro | Métro
rail | LocalTrain | Train régional / TER | LocalTrain | Train régional / TER
trolleyBus | Tramway | Tramway | TrolleyBus | TrolleyBus
tram | Tramway | Tramway | Tramway | Tramway
water | Boat | Navette maritime/fluviale | Boat | Navette maritime/fluviale
cableway | Tramway | Tramway | CableWay | CableWay
funicular | Funicular | Funiculaire | Funicular | Funiculaire
lift | Bus | Bus | Bus | Bus
other | Bus | Bus | Bus | Bus


### lines.txt
`lines` are provided in the **lignes.xml** file in the node **CompositeFrame/frames/ServiceFrame/lines/**. 

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
line_id | *Line/@id* | This field is prefixed. 
line_code | *Line/ShortName* | 
line_name | *Line/Name* | 

If the node `Line/PrivateCode` is available, the content of this node is added as an `object_code` for this line with `object_system` set at `PrivateCode`.

### routes.txt
`routes` are provided in each **offre_** file of each folder in the node **GeneralFrame/members/**. There could be multiple `Route` to read.

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
route_id | *Route/@id* | This field is prefixed. 
route_name | *Route/Name* | 
direction_type | *Route/DirectionType* | The value of this field is used without transformation.
destination_id |  | The `DirectionRef` of the Route doesn't link to a stop (neither stop_point nor stop_area), thus it's value is not used.
line_id | *Line/LineRef/@ref* | This field is prefixed. 

**ServiceJourneyPattern references**
All ServiceJourneyPattern of a `route` are stored as complementary `object_codes`. ServiceJourneyPattern nodes are listed in the same parent node as Route ndoes.
A ServiceJourneyPattern references a Route using the `ServiceJourneyPattern/RouteRef/@ref` attribute.

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
object_type |  | fixed value `route` 
object_id | *Route/@id* | This field is prefixed. 
object_system |  | fixed value `ServiceJourneyPattern`
object_code | *ServiceJourneyPattern/id* | The value of this field is used without transformation.


### trips.txt
A `trip` is described in a `ServiceJourney` node in **GeneralFrame/members/** of each **offre_** file of each folder. 

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
route_id | *ServiceJourney/JourneyPatternRef/@ref* | route_id of the Route containing the JourneyPatternRef as an object_code.
service_id |  | ??? Using DayTypeRef ??? 
trip_id | *ServiceJourney/@id* | This field is prefixed. 
trip_headsign |  | This field is not yet defined.
company_id | *ServiceJourney/OperatorRef* | if *ServiceJourney/OperatorRef* is not defined, use *Line/OperatorRef* in *lines.xml* file (cf. lines.txt). This field is prefixed.
physical_mode_id | *Line/TransportMode* | see physical_modes definition
trip_property_id |  | see trip_properties.txt

**comment_links for a trip**
If one (or more) `noticeAssignments/NoticeAssignment/NoticeRef` is available in the `ServiceJourney`, a `comment_link` will be created as follow.

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
object_id | *ServiceJourney/@id* | This field is prefixed. 
object_type |  | Fixed value `trip`
comment_id | *noticeAssignments/NoticeAssignment/NoticeRef* | This field is prefixed. 

### trip_properties.txt
In the Line declaration (cf. `lines.txt`), if a node `Line/keyList/keyValue/Key` contains the value `Accessibility`, a `trip_property` will be specified for all the trips of the line.

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
trip_property_id | *Line/@id* | The id of the line is used to create this object. This field is prefixed. 
wheelchair_accessible | `Line/keyList/keyValue/Value` | See (1) below.

wheelchair_accessible value: 
* If source value is "0", then this property is set to "2" (not accessible),
* If source value is "1", then this property is set to "1" (accessible),
* else his property is set to "0" (unknown)


### stop_times.txt
A stop_time is specified by a `TimetabledPassingTime` node in `ServiceJourney/passingTimes`.
NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
trip_id | *Line/@id* | The id of the line is used to create this object. This field is prefixed. 
stop_sequence | | Auto-incremented field 
starting with `0` for the first stop_time
stop_id | | See (1) below
arrival_time | *TimetabledPassingTime/ArrivalTime* | If `TimetabledPassingTime/DepartureDayOffset` value is >0, arrival_time is incremented 24 hours for each day offset. 
departure_time | *TimetabledPassingTime/DepartureTime* | If `TimetabledPassingTime/DepartureDayOffset` value is >0, departure_time is incremented 24 hours for each day offset. 
boarding_duration | | Fixed value `0`
alighting_duration | | Fixed value `0`
pickup_type | | RoutingConstraintZone in offre_* files
drop_off_type | | RoutingConstraintZone in offre_* files
local_zone_id | | RoutingConstraintZone in offre_* files



(1) Definition of the stop_id of a stop_time:
1. In the `ServiceJourneyPattern` referenced by `ServiceJourney/JourneyPatternRef/@ref` of the `TimetabledPassingTime`, the `pointsInSequence/StopPointInJourneyPattern` node of the same position as the stop_time is used.
2. The `ScheduledStopPointRef/@ref` attribute is the searched in the `PassengerStopAssignment` nodes of the same file (in the `PassengerStopAssignment/ScheduledStopPointRef/@ref`)
3. The `PassengerStopAssignment/QuayRef/@ref` is the stop_id of the stop_point (with a prefix).

### calendar.txt and calendar_dates.txt
Active days of `trips` are decribed in **calendriers.xml** of each folder in the `GeneralFrame` node.
A ValidityPattern is created for each `members/DayType` of the file.

* `ValidBetween/FromDate` and `ValidBetween/ToDate` provide start and end date of each calendars defined in this file (both dates are included). Those dates are restrinctions to be applied to all calendars of this file.
* A `members/DayType` nodes (referenced by trips), describing the basically active days of a week
* `members/DayTypeAssignment` nodes, describing exceptions (added or removed days or periods)
* `members/OperatingPeriod` ndoes, describing periodes referenced in `DayTypeAssignment`

Here is 3 possible modelization in Netex-IDFM of a calendar running from 2016-07-01 to 2016-07-31 except on sundays and on 2016-07-14:
![](./netex_idfm_to_ntfs_calendars.png "Definition of calendars in Netex-IDFM specs")

In the NTFS, the `service_id` is set to the `members/DayType/@id` attribute (this field is prefixed).
Active dates are defined by : 
- Active days described in `members/DayType/properties/PropertyOfDay/DaysOfWeek`. Expected values MUST be one of `Monday`, `Tuesday`, `Wednesday`, `Thursday`, `Friday`, `Saturday`, `Sunday`.
- One or several `members/DayTypeAssignment`
  + if the `DayTypeAssignment` contains an attribute `isAvailable` (that should be set at `False`), an inactive date is declared by `DayTypeAssignment/Date`
  + else an active period referenced by `DayTypeAssignment/OperatingPeriodRef/@ref`. This `OperatingPeriod` is to be used with `DayType` to list effective active dates
The result is to be restricted between `ValidBetween/FromDate` and `ValidBetween/ToDate` specified at the top level of the file in `GeneralFrame`.

Be careful : Definition of calendars and exceptions in calendar_dates may not be the same definition as the on in the Netex-IDFM files, but the resulting active dates will be the same.


### comments.txt
`comments` are provided in **commun.xml** files of each subfolder in the nodes **GeneralFrame/members/Notice**.

NTFS field | Netex IDFM element | Mapping rule/Comment
--- | --- | ---
comment_id | *Notice/@id* | This field is prefixed.
comment_name | *Notice/Text* |

### transfers.txt
Transfers are not yet provided in the Netex IDFM Data feed. Transfers need to be generated afterward to provide accurate trip planning.