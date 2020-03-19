# Netex IDFM reading specification
## Introduction
This document describes how a Netex feed provided by Ile-de-France Mobilités (IDFM) is read in Navitia Transit model (NTM)
and transformed into an [NTFS feed](https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fr.md).

For the sake of simplicity, the NTM properties that are not specified in the source
data are not described below.

In order to guarantee that the NTFS objects identifiers are unique and stable, each
object id is prefixed with a unique prefix (specified for each datasource), following
the general pattern `<prefix>:<id>`. This prefix is uniquely defined for all data_sources 
to make possible safe data aggregations.

### Versions of this document:
Version | Date | Modification
--- | --- | ---
1.0 | 2019-09-13 | Initial redaction
1.1 | 2019-09-30 | Adding fare zones, MIP access on lines and stop_points, the use of source coordinates of a stop_area (if available), complementary properties of a line (commercial_mode, line_color, line_text_color, comment on line)
1.2 | 2019-10-07 | Using new source specifications for Netex IDFM lines and stops, reading of `lignes.xml` and `arrets.xml` is reworked (no more complementary codes on stops and lines, associations between objects changed, etc.).<br>For `stop_points`, lowest level of `Quay` will be used (ZDEp).

## Input data description
This specification assumes that all the required data (time tables for all the lines, stop points and stop areas, transfers, etc.) are provided in one ZIP archive (aka "FICHIERS OFFRE") described in the private specification documents provided by IDFM:
+  *NT60-A150701-v1.11-BO-STIF_-_Specification_Technique_d_Interface_NeTEx_pour_la_publication_20190624.docx*
+ *ATD-TDI-LZR-069-DINT WS-NETEX-01 v1.0.9_20190805.docx* (section 4.3)
+ *DINT-LIGNE_publication_1.7.3_20180306.docx* (section 4, using the WebService version 2)

The ZIP archive contains: 
- one **arrets.xml** file
- one **lignes.xml** file, containing the description of lines, networks and companies
- one **correspondances.xml** file (not yet described)
- one folder for each operator (or company) containing:
  + a **calendriers.xml** file, containing the calendars (or validity patterns) used by trips in files **offre_**
  + a **commun.xml** optional file, containing the comments referenced by the operator objects (if needed)
  + several files starting with **offre_** describing the time tables of a specific line in each file

In this document, versions of objects are not handled. The first encountered object description is considered when creating an object. 

Each XML file contains a `PublicationDelivery` node, containing a `dataObjects` node. Descriptions below is considering nodes inside this `dataObjects` node. Furthermore, brackets (`[]`) in an XML tag indicates this tag can appear more than one time.

## Reading of the "arrets.xml" file into stops.txt file
In the Netex-IDFM feed: 
- a `Quay` object defined at the operator level is referenced by a `Quay` object defined by the PTA (IDFM),
- this PTA `Quay` is included in a monomodal `StopPlace`,
- this monomodal `StopPlace` is included in a multimodal `StopPlace`.

### For stop_areas
`stop_area` objects in the output NTFS are top level `StopPlace` (ie. not included in an other `StopPlace`).
`stop_area` are defined by `CompositeFrame/frames/GeneralFrame[]/StopPlace[]` nodes. Only the `GeneralFrame` with the `TypeOfFrameRef/@ref` attribut set to `FR100:TypeOfFrame:NETEX_ARRET_STIF:` contains the stop_areas.

A stop_area can therefore be:

1. a multimodal `StopPlace`
2. a monomodal `StopPlace` without a multimodal `StopPlace` containing it

A `stop_area` is created for each `StopPlace` not containing a `StopPlace/ParentSiteRef` tag or referencing by `StopPlace/ParentSiteRef/@ref` a `StopPlace` that does not exist in the feed.

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
stop_id | StopPlace/@id | This field is prefixed. The technical part of the NeTEx identifier is used. For example, in `FR::multimodalStopPlace:69406:FR1`, the `stop_area` identifier is `<prefix>:69406` (fourth field with colon `:` separator). In the case of a `stop_area` created from a monomodal `StopPlace`, identifier of the `stop_area` is `<prefix>:monomodalStopPlace:411396` (third and fourth fields with a colon `:` separator)
stop_name | StopPlace/Name | 
location_type | | Fixed value `1` (stop_area)
stop_lat | StopPlace/Centroid/Location | see (1) below
stop_lon | StopPlace/Centroid/Location | see (1) below

**(1) Definition of stop_lat et stop_lon:**

The node `StopPlace/Centroid/Location` declares the position (X, Y) of the stop_area with an explicit EPSG always set to 2154. This coordinate will be converted to the NTFS WGS84 projection (EPSG:4326).
Example of Netex declaration:
><gml:pos srsName="EPSG:2154">662233.0 6861519.0</gml:pos>

If the Netex feed does not provide coordinates for a stop_area, the stop_lat et stop_lon fields will be set with the coordinate of the centroid of all included stop_points.

**Complementary object_properties**

No complementary properties on `stop_area`.

**Complementary object_codes**

The `stop_area` has a complementary code `source` with the identifier of the original associated `StopPlace`.

### For stop_points
`stop_point` objects in the output NTFS are the lowest level `Quays` (those provided by operators).
`stop_points` are defined by `CompositeFrame/frames/GeneralFrame[]/Quay[]` nodes, considering:
- only the `GeneralFrame` with the `TypeOfFrameRef/@ref` attribut set to `FR100:TypeOfFrame:NETEX_ARRET_STIF:` contains the stop_points,
- only the `Quay` nodes having the `Quay/@dataSourceRef` property set to **_anything different_** from `FR1-ARRET_AUTO`

The `Quay` nodes with a `FR1-ARRET_AUTO` value in `Quay/@dataSourceRef` property are the PTA (IDFM) defined Quays.

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
stop_id | Quay/@id | This field is prefixed. The technical part of the NeTEx identifier is used. For example, in `FR::Quay:50117139:FR1`, the `stop_point` identifier is `<prefix>:50117139` (fourth field with colon `:` separator).
stop_name | Quay/Name | 
location_type | | Fixed value `0` (stop_point)
stop_lat | Quay/Centroid/Location | see (1) below
stop_lon | Quay/Centroid/Location | see (1) below
parent_station | | stop_id of the corresponding `stop_area`, see (2) below.
stop_timezone | | fixed value `Europe/Paris`
equipment_id | | This value will be self generated by the connector if an equipment object is necessary. See (2) below.
fare_zone_id | Quay/tariffZones/TariffZoneRef/@ref | If the `tariffZones` does not exist or is empty, this field will be empty. If more than one fare zone are available, the first one will be considered. The fare zone needs to be extract from the content of the `@ref` attribute, using the 3rd field using a `:` separator. If this 3rd field is not an integer, the resulting `fare_zone_id` will be empty.

**(1) Definition of stop_lat et stop_lon:**

The node `Quay/Centroid/Location` declares the position (X, Y) of the stop_point with an explicit EPSG always set to 2154. This coordinate will be converted to the NTFS WGS84 projection (EPSG:4326).
Example of Netex declaration:
><gml:pos srsName="EPSG:2154">662233.0 6861519.0</gml:pos>

**(2) Definition of the parent_station:**
Linking a stop_area to a stop_point requires to navigate from lower `Quay` to upper `StopPlace` nodes:
- the operator `Quay` node (aka ZDEp) references the PTA `Quay` node (aka ZDEr) with the attribute `Quay/@derivedFromObjectRef`,
- the PTA `Quay` (aka ZDEr) references the monomodal `StopPlace` node (aka ZDL) with the attribute `Quay/ParentZoneRef/@ref`
- the monomodal `StopPlace` node (aka ZDL) references the multimodal `StopPlace` (aka LDA, the actual `stop_area`) with the attribute `StopPlace/ParentSiteRef/@ref` (unless there is no valid multimodal parent, then the monomodal StopPlace is the stop_area).

If no parent_station is available (neither multimodal nor monomodal), a stop_area will be created using the stop_point properties.

**Definition of the Accessibility of a stop_point**

If the `Quay` node corresponding to the stop_point contains a `AccessibilityAssessment/MobilityImpairedAccess` node, an equipment will be generated (in the NTFS **equipments.txt** file) with properties described below. The resulting **equipments.txt** file will contain as few lines as possible to describe all possible sets of values.

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
equipment_id |  | Auto-incremented value. This field is prefixed. 
wheelchair_boarding | Quay/AccessibilityAssessment/MobilityImpairedAccess | If the source value is `false`, `wheelchair_boarding` value is set to `2` (non accessible). <br>If the source value is `true`, `wheelchair_boarding` value is set to `1` (accessible)<br> In any other case (other value or missing node) the value is set to `0` (unknown accessibility)
visual_announcement | Quay/AccessibilityAssessment/ limitations/AccessibilityLimitation/VisualSignsAvailable | same rule as `wheelchair_boarding`
audible_announcement | Quay/AccessibilityAssessment/ limitations/AccessibilityLimitation/AudibleSignalsAvailable | same rule as `wheelchair_boarding`

**Complementary object_codes**

The `stop_point` has a complementary code `source` with the identifier of the original associated `Quay`.

## Reading of the "lignes.xml" file

### networks.txt
`network` objects are provided in the nodes **CompositeFrame/frames/ServiceFrame[]/Network**, each `ServiceFrame` node containing only one `Network` node.

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
network_id | Network/@id | This field is prefixed. The technical part of the NeTEx identifier is used. For example, in `FR1:Network:1046:LOC`, the `network` identifier is `<prefix>:1046` (third field with colon `:` separator).
network_name | Network/Name | 
network_timezone | | Fixed value `Europe/Paris`.

The `network` has a complementary code `source` with the identifier of the original associated `Network`.

### companies.txt
`companies` (aka operators) are provided in the nodes **CompositeFrame/frames/ResourceFrame/organisations/Operator[]**. There is only one `ResourceFrame` in the file.

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
company_id | Operator/@id | This field is prefixed. The technical part of the NeTEx identifier is used. For example, in `FR1:Operator:800:LOC`, the `company` identifier is `<prefix>:800` (third field with colon `:` separator). 
company_name | Operator/Name | 


### commercial_modes.txt and physical_modes.txt
The transport modes in Netex-IDFM are only defined at the Line level in the node **CompositeFrame/frames/ServiceFrame[]/lines/Line[]/TransportMode**. The only `ServiceFrame` containing the lines has an id property set to `STIF:CODIFLIGNE:ServiceFrame:lineid`.

Be careful, `physical_mode_id` and `commercial_mode_id` are **not** prefixed.

Here is the mapping table between a TransportMode value and the corresponding `physical_mode` and `commercial_mode`:

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
water | Boat | Navette maritime / fluviale | Boat | Navette maritime / fluviale
cableway | Tramway | Tramway | CableWay | CableWay
funicular | Funicular | Funiculaire | Funicular | Funiculaire
lift | Bus | Bus | Bus | Bus
other | Bus | Bus | Bus | Bus

All `physical_mode` are enhanced with CO2 emission and fallback modes, following
the documentation in [common.md](common.md#co2-emissions-and-fallback-modes).

### lines.txt
`lines` are provided in the nodes **CompositeFrame/frames/ServiceFrame[]/lines/Line[]**. The only `ServiceFrame` containing the lines has an id property set to `STIF:CODIFLIGNE:ServiceFrame:lineid`.

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
line_id | Line/@id | This field is prefixed. The technical part of the NeTEx identifier is used. For example, in `FR1:Line:C01738:LOC`, the `line` identifier is `<prefix>:C01738` (third field with colon `:` separator).
network_id | Line/RepresentedByGroupRef/@ref | This field is prefixed. If this attribute or if the referenced network does not exists, this line is not created.
commercial_mode_id | Line/TransportMode | corresponding commercial_mode_id (see mapping above, this field is **not** prefixed). 
line_code | Line/PublicCode or Line/ShortName | Use the `PublicCode` value if available and not empty, else the `ShortName` should be used.
line_name | Line/Name | 
line_color | Line/Presentation/Colour | If the value is not available or is not a valid hexa RGB, the value `000000` (black) is used.
line_text_color | Line/Presentation/TextColour | If the value is not available or is not a valid hexa RGB, the value `FFFFFF` (white) is used.

The `line` has a complementary code `source` with the identifier of the original associated `Line`.

If the node `Line/PrivateCode` is available, the content of this node is added as an `object_code` for this line with `object_system` set at `Netex_PrivateCode`.

The accessibility of the line, described by `Line/AccessibilityAssessment` node is used at the trip level (see below).

**Comments on line:**
If at least one `Line/noticeAssignments/NoticeAssignment/NoticeRef` exists and references a valid notice described in one of the **commun.xml** files of the `OFFRE` folders, a link between those notices and the line is described in the NTFS `comment_links.txt` file.

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
object_id | Line/@id | This field is prefixed. 
object_type |  | fixed value `line`
comment_id | Line/noticeAssignments/NoticeAssignment/NoticeRef | This field is prefixed. 


## Reading of each folder
In a **offre_*.xml** file, 2 **GeneralFrame** are expected in a **CompositeFrame/frames** node:
* one with a `TypeOfFrameRef/@ref` containing the string `NETEX_STRUCTURE`
* one with a `TypeOfFrameRef/@ref` containing the string `NETEX_HORAIRE`

### routes.txt
`routes` are provided in each **offre_** file in the nodes **CompositeFrame/frames/GeneralFrame[]/members/Route[]**. The `GeneralFrame` to be used is the one with the `@ref` containing `NETEX_STRUCTURE`.

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
route_id | Route/@id | This field is prefixed. The source and technical part of the NeTEx identifier are used. For example, in `SNCF:Route:937-C01738-9c749775-ca06-350a-9726-f27b7265ea34:LOC`, the `route` identifier is `<prefix>:SNCF:937-C01738-9c749775-ca06-350a-9726-f27b7265ea34` (first and third field with colon `:` separator).
line_id | Route/LineRef/@ref | This field is prefixed. 
route_name | Route/Name | See [`common.md`](common.md#general-rules) to generate the `name`
direction_type | Route/DirectionType | The value of this field is used without transformation.
destination_id |  | The `DirectionRef` of the Route doesn't link to a stop (neither stop_point nor stop_area), thus its value is not used. See [`common.md`](common.md#general-rules) to generate the `destination_id`

The `route` has a complementary code `source` with the identifier of the original associated `Route`.

**ServiceJourneyPattern references**

All ServiceJourneyPattern of a `route` are stored as complementary `object_codes`. ServiceJourneyPattern nodes are listed in the nodes `CompositeFrame/frames/GeneralFrame[]/members/ServiceJourneyPattern[]`. The `GeneralFrame` to be used is the one with the `@ref` containing `NETEX_STRUCTURE`.

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
object_type |  | fixed value `route` 
object_id | ServiceJourneyPattern/RouteRef/@ref | This field is prefixed. 
object_system |  | fixed value `Netex_ServiceJourneyPattern`
object_code | ServiceJourneyPattern/@id | The value of this field is used without transformation.


### trips.txt
`trips` are described in each **offre_*.xml** file. A `trip` combines information of:
- `ServiceJourneyPattern` listed in **CompositeFrame/frames/GeneralFrame[]/members/ServiceJourneyPattern[]**  (the `GeneralFrame` is the one with the `@ref` containing `NETEX_STRUCTURE`).
- `ServiceJourney` listed in **CompositeFrame/frames/GeneralFrame[]/members/ServiceJourney[]** (the `GeneralFrame` is the one with the `@ref` containing `NETEX_HORAIRE`),

The link between those 2 objects is made by `ServiceJourney/JourneyPatternRef/@ref`.

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
route_id | ServiceJourney/JourneyPatternRef/@ref | route_id of the Route containing the `JourneyPatternRef` as an object_code.
service_id |  | Defined using `ServiceJourney/DayTypeRef`, see (calendar.txt and calendar_dates.txt)[]
trip_id | ServiceJourney/@id | This field is prefixed. 
trip_headsign | ServiceJourneyPattern/DestinationDisplayRef | Content of the `DestinationDisplay/FrontText` node. If not available, the name of the `stop_point` of the last `stop_time` is used.
trip_short_name | ServiceJourneyPattern/DestinationDisplayRef | Content of the `DestinationDisplay/PublicCode` node. If not available, this field is empty.
company_id | ServiceJourney/OperatorRef | if `ServiceJourney/OperatorRef` is not defined, use `Line/OperatorRef` in [*lines.xml*](#linestxt) file. This field is prefixed.
physical_mode_id | Line/TransportMode | see physical_modes definition (this field is **not** prefixed)
trip_property_id |  | see [trip_properties.txt](#trip_propertiestxt)


**comment_links for a trip**

If one (or more) `noticeAssignments/NoticeAssignment/NoticeRef` is available in the `ServiceJourney`, a `comment_link` will be created as follow in the `comment_links.txt` file.

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
object_id | ServiceJourney/@id | This field is prefixed. 
object_type |  | Fixed value `trip`
comment_id | noticeAssignments/NoticeAssignment/NoticeRef | This field is prefixed. 

### trip_properties.txt
In the Line declaration (cf. `lines.txt`), if a node `Line/AccessibilityAssessment` exists, a `trip_property` will be specified for all the trips of the line.

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
trip_property_id | Line/AccessibilityAssessment/@id | The id of the line is used to create this object. This field is prefixed. 
wheelchair_accessible | Line/AccessibilityAssessment/MobilityImpairedAccess | See (1) below.
visual_announcement | Line/AccessibilityAssessment/limitations/AccessibilityLimitation/VisualSignsAvailable | See (1) below.
audible_announcement | Line/AccessibilityAssessment/limitations/AccessibilityLimitation/AudibleSignalsAvailable | See (1) below.

(1) setting accessibility value: 
* If source value is `false`, then this property is set to `2` (not accessible),
* If source value is `true`, then this property is set to `1` (accessible),
* else this property is set to `0` (unknown)


### stop_times.txt
`stop_times` of a trip are listed in the `ServiceJourney/passingTimes/TimetabledPassingTime[]` nodes. They are positioned in the same order as the `ServiceJourneyPattern/pointsInSequence` corresponding to the ServiceJourney.

NTFS field | Netex-IDFM element | Mapping rule/Comment
--- | --- | ---
trip_id | ServiceJourney/@id | This field is prefixed. 
stop_sequence | | Auto-incremented field starting with `0` for the first stop_time
stop_id | | See (1) below
arrival_time | TimetabledPassingTime/ArrivalTime | If `TimetabledPassingTime/DepartureDayOffset` value is >0, arrival_time is incremented 24 hours for each day offset (value will be greater than 24:00 for the stop_times on the next day). See (2) for hours passing midnight.
departure_time | TimetabledPassingTime/DepartureTime | If `TimetabledPassingTime/DepartureDayOffset` value is >0, departure_time is incremented 24 hours for each day offset (value will be greater than 24:00 for the stop_times on the next day). 
boarding_duration | | Fixed value `0`
alighting_duration | | Fixed value `0`
pickup_type | | See (3) below
drop_off_type | | See (4) below
local_zone_id | | See (4) below


(1) Definition of the stop_id of a stop_time:

1. Find the `ServiceJourneyPattern` referenced by `ServiceJourney/JourneyPatternRef/@ref` of the `TimetabledPassingTime`.
2. The `ServiceJourneyPattern/pointsInSequence/StopPointInJourneyPattern` node of the same position as the stop_time is used.
3. The `StopPointInJourneyPattern/ScheduledStopPointRef/@ref` attribute is searched in the `PassengerStopAssignment/ScheduledStopPointRef/@ref` attribute of all the `PassengerStopAssignment` nodes of the file
4. The `PassengerStopAssignment/QuayRef/@ref` is the stop_id of the stop_point (with a prefix).

(2) Passing midnight

When a trip stops around midnight (arriving at stop before midnight and departing after midnight),  `TimetabledPassingTime/ArrivalTime` is greater than `TimetabledPassingTime/DepartureTime`. 
In this particular situation, `arrival_time` is incremented with 1 day less than the `departure_time`.

Eg: `TimetabledPassingTime/ArrivalTime` = "23:50:00", `TimetabledPassingTime/DepartureTime` = "00:10:00",  `TimetabledPassingTime/DepartureDayOffset` = 1
The departure is the next day so arrival_time = "23:50:00" and departure_time = "24:10:00"

(3) Definition of pickup_type and drop_off_type:

In the `ServiceJourneyPattern/pointsInSequence/StopPointInJourneyPattern` corresponding to this `stop_time` (see `(1)`):
* if the `ForBoarding` node is existing and with a `false` value, `pickup_type` is set to "1" (no boarding)
* else `pickup_type` is set to "0" (regular boarding)

`drop_off_type` is set using the same method and using the `ForAlighting` node.

(4) Definition of local_zone_id:

The declaration of those zones is made in the nodes `RoutingConstraintZone` of the **offre_*.xml** file.
The `local_zone_id` is specified with an auto-incremented integer. Each `RoutingConstraintZone/@id` is associated with a new integer. 
This `RoutingConstraintZone` contains a list of `ScheduledStopPointRef` (in `RoutingConstraintZone/members/ScheduledStopPointRef/@ref`). If a stop_time corresponds to one of those `ScheduledStopPointRef`, the `local_zone_id` is set to the associated integer.

### calendar.txt and calendar_dates.txt
Active days of `trips` are decribed in **calendriers.xml** of each folder in the `GeneralFrame/members/DayType` node (with the use of `DayTypeAssignment` and `OperatingPeriod`).

A `trip` references one or several `DayType` in `ServiceJourney/dayTypes/DayTypeRef/@ref`.
The `service_id` property of the calendar in the NTFS is specified by an auto-incremented integer (this field is prefixed).

Active dates of the calendar are specified by:
- `DayType` nodes describing the active days of a week
  + days of the week are listed in `DayType/properties/PropertyOfDay[]/DaysOfWeek` nodes. 
    + Expected values MUST be one of `Monday`, `Tuesday`, `Wednesday`, `Thursday`, `Friday`, `Saturday`, `Sunday`.
  + `DayType/properties` node is optional, a `DayType` node may not have any active days of a week.
- `OperatingPeriod` nodes describing periods (basically a beginning date and an end date) referenced in `DayTypeAssignment`
- `DayTypeAssignment` nodes with 2 possible uses (they are applied in the order they appear in the file):
  + Activate or deactivate a specific day on a DayType (with the nodes `IsAvailable` and `Date`)
    + the node `IsAvailable` is optional. If not present the default value is `true`.
  + Apply the active days of the referenced DayType on an `OperatingPeriod`

All resulting calendars are to be restricted between `ValidBetween/FromDate` and `ValidBetween/ToDate` specified at the top level of the file in `GeneralFrame`.

Active dates are read in the following order:
- dates from operating periods
- dates added (`DayTypeAssignment/IsAvailable` = true)
- dates removed (`DayTypeAssignment/IsAvailable` = false)

Be careful: Definition of calendars and exceptions in calendar_dates may not be the same definition as the one in the Netex-IDFM files, but the resulting active dates will be the same.

Here is 3 possible modelizations in Netex-IDFM of a calendar running from 2016-07-01 to 2016-07-31 from monday to saturday except on 2016-07-14:
![](./netex_idfm_to_ntfs_calendars.png "Definition of calendars in Netex-IDFM specs")

### comments.txt
`comments` are provided in **commun.xml** file in the nodes **GeneralFrame/members/Notice[]**.

NTFS field | Netex IDFM element | Mapping rule/Comment
--- | --- | ---
comment_id | *Notice/@id* | This field is prefixed.
comment_name | *Notice/Text* |

### transfers.txt
Transfers are not yet provided in the Netex IDFM Data feed. Transfers need to be generated afterward to provide accurate trip planning.
