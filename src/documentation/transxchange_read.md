# TransXChange reading specification
## Introduction
This document describes how a TransXChange feed is read in Navitia Transit model (NTM) 
and transformed into a [NTFS feed](https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/ntfs_fr.md).

For the sake of simplicity, the NTM properties that are not specified in the source 
data are not described below.

In order to guarantee that the NTFS objects identifiers are unique and stable, each 
object id is prefixed with a unique prefix (specified for each datasource), following 
the general pattern `<prefix>:<id>`.

## Input data description
Each file of a TransXChange dataset represents a transit line for a *specific 
operating period*. Several files of the same archive might need to be read in order 
to consolidate all the trips associated to a transit NTFS line.

An additional data source is necessary in order to retrieve the information relative to the 
stops used in the TransXChange feed. The National Public Transport Access Nodes ([NaPTAN](http://naptan.app.dft.gov.uk/DataRequest/Naptan.ashx?format=csv)) database 
is a UK nationwide system for uniquely identifying all the points of access to public transport in the UK.

## Mapping between TransXChange elements and NTFS objects
### networks.txt

NTFS field | TransXChange element | Mapping rule/Comment
--- | --- | ---
network_id | *Operators/Operator/OperatorCode* | This field is prefixed.
network_name | *Operators/Operator/TradingName* | If the element is not present, the *Operators/Operator/OperatorShortName* is used instead.
network_timezone | | Fixed value `Europe/London`.

### companies.txt
NTFS field | TransXChange element | Mapping rule/Comment
--- | --- | ---
company_id | *Operators/Operator/OperatorCode* | This field is prefixed.
company_name | *Operators/Operator/OperatorShortName* |

### stops.txt
For each *AnnotatedStopPointRef* identified in the TransXChange feed, the coordinates 
of the stop_point are retrieved from the **Stops.csv** file of the NaPTAN dataset.

The stop_areas are referenced in the **StopsInArea.csv** file and then detailed in 
the **StopAreas.csv** file of the NaPTAN dataset.

#### For stop_points

NTFS field | TransXChange element | NaPTAN file | NaPTAN field | Mapping rule/Comment
--- | --- | --- | --- | ---
stop_id | *StopPoints/AnnotatedStopPointRef/StopPointRef* | Stops.csv | *ATCOCode* | This field is prefixed.
stop_name | | Stops.csv | *CommonName* | The stop name is also available in TransXChange *StopPoints/AnnotatedStopPointRef/CommonName*, but the NaPTAN value is considered to be the reference.
location_type | | Stops.csv | | Fixed value `0`.
stop_lat | | Stops.csv | *Latitude* | 
stop_lon | | Stops.csv | *Longitude* | 
parent_station | | StopsInArea.csv | *StopAreaCode* | This field is prefixed. The field *AtcoCode* is used as a matching key with the *ATCOCode* of the associated stop_point.
platform_code | | Stops.csv | *Indicator* |

In addition, if the *NaptanCode* field is defined for a stop_point in the NaPTAN dataset, this code is added as a complementary code for the associated stop_point with the value `NaptanCode` as the name of the identification system.

#### For stop_areas

NTFS field | NaPTAN file | NaPTAN field | Mapping rule/Comment
--- | --- | --- | ---
stop_id | StopsInArea.csv | *StopAreaCode* | This field is prefixed.
stop_name | StopAreas.csv | *Name* |
stop_lat | StopAreas.csv | *Northing* | The input coordinate system Easting/Northing (EPSG:27700) should be converted to WGS84 (EPSG:4326). In case the coordinates of the stop_area are not specified, they are computed as the barycenter of all the associated stop_points.
stop_lon | StopAreas.csv | *Easting* | The input coordinate system Easting/Northing (EPSG:27700) should be converted to WGS84 (EPSG:4326). In case the coordinates of the stop_area are not specified, they are computed as the barycenter of all the associated stop_points.
location_type | | | Fixed value `1`.

### lines.txt

NTFS field | TransXChange element | Mapping rule/Comment
--- | --- | ---
line_id | *Services/Service/ServiceCode* | This field is prefixed.
line_code | *Services/Service/Lines/Line/LineName* |
line_name | *Services/Service/Description* | If no *Description* is found, then this field is computed using the name of the first associated Route in the forward direction. If several forward routes exist, the one with the smallest `route_id` is used.
forward_line_name | *Services/Service/StandardService/Destination* | 
forward_direction |  | This field should have the same value as the `destination_id` of the first associated Route in the forward direction. If several forward routes exist, the one with the smallest `route_id` is used.
backward_line_name | *Services/Service/StandardService/Origin* | 
backward_direction |  | This field should have the same value as the `destination_id` of the first associated Route in the backward direction. If several backward routes exist, the one with the smallest `route_id` is used.
network_id | *Services/Service/RegisteredOperatorRef* | The referenced *Operators/Operator/OperatorCode* value is taken into account. This field is prefixed. Link to the file [networks.txt](#networkstxt).
commercial_mode_id | *Services/Service/Mode* | This field is not prefixed. Link to the file commercial_modes.txt of the NTFS. See the mapping rule below.

#### Defining modes
*Services/Service/Mode* in TransXChange | physical_mode_id in NTFS | physical_mode_name in NTFS | commercial_mode_id in NTFS | commercial_mode_name in NTFS
--- | --- | --- | --- | ---
air | Air | Air | Air | Air
bus | Bus | Bus | Bus | Bus
coach | Coach | Coach | Coach | Coach
ferry | Ferry | Ferry | Ferry | Ferry
metro/underground | Metro | Metro | Metro | Metro
rail | Train | Train | Train | Train
tram | Tramway | Tramway| Tramway| Tramway
trolleyBus | Shuttle | Shuttle | Shuttle | Shuttle

### routes.txt
A Route is created from a line and a direction of journey patterns (*StandardService/JourneyPattern/Direction*).

NTFS field | TransXChange element | Mapping rule/Comment
--- | --- | ---
route_id | *Services/Service/ServiceCode*, *StandardService/JourneyPattern/Direction* | This field is prefixed and formed by the concatenation of *Services/Service/ServiceCode* and *StandardService/JourneyPattern/Direction* separated by a `:`. Ex. "\<prefix>:1_58_BC:inbound".
route_name |  | "[first stop of the first trip] - [last stop of the first trip]" (1)
direction_type | *StandardService/JourneyPattern/Direction* | The value is set to `inbound` or `clockwise` when the specified value for the direction is `inboundAndOutbound` or `circular`, respectively.
line_id | *Services/Service/ServiceCode* | This field is prefixed. Link to the file [lines.txt](#linestxt).
destination_id | *???* | `stop_id` of the stop_area of the last stop of the first trip. (1)

(1) The first trip of a route is the one with the smallest `trip_id` value.

### calendar_dates.txt
The validity period of a service is stated in *Services/Service/OperatingPeriod*. In case the validity period is open ended (the *EndDate* is not specified), the default value [*StartDate* + 180 days] should be used.

Service days are calculated from the *VehicleJourneys/VehicleJourney/OperatingProfile* (if not specified, the operation days are inherited from *Service/OperatingProfile*). The corresponding days of the week are activated according to the pattern given by *RegularDayType/DaysOfWeek*. If no particular day of the week is explicitly specified, all days of the week (Monday to Sunday) are considered by default.

The element *SpecialDaysOperation* may also be present specifying a *DateRange* with the specific dates of (non) operation. The days on which the service does (*DaysOfOperation*) or does not (*DaysOfNonOperation*) run are specified separately. Note that special days of operation are additional to the regular operating period (inclusion); inversely, special days of non operation further restrict the regular operating period (exclusion).

Similarly, the element *BankHolidaysOperation* may be also be present, specifying how the service operates on a bank holiday. The possible values are the following: `AllBankHolidays`, `AllHolidaysExceptChristmas`, `ChristmasDay`, `Christmas`, `BoxingDay`, `NewYearsDay`, `Jan2ndScotland`, `GoodFriday`, `EasterMonday`, `MayDay`, `SpringBank`, `AugustBankHolidayScotland`, `LateSummerBankHolidayNotScotland`, `StAndrewsDay`.

Note that special days override any Bank holiday day types.

### trips.txt
A trip is to be created from a VehicleJourney and then link to the JourneyPattern that specifies the sequence of stops and time intervals for the trip.

JourneyPattern or VehicleJourney/DestinationDisplay = trip_headsign

### stop_times.txt

NTFS field | TransXChange element | Mapping rule/Comment
--- | --- | --- 
trip_id | *???* | This field is prefixed. Link to the file [trips.txt](#tripstxt).
arrival_time | | *VehicleJourney/DepartureTime* + *JourneyPatternSection/RunTime*
departure_time | | *VehicleJourney/DepartureTime* + *JourneyPatternSection/RunTime* +  *JourneyPatternSection/WaitingTime* (if a waiting time is defined, otherwise it is considered 0)
stop_id | *???* | This field is prefixed. Link to the file [stops.txt](#stopstxt).
stop_sequence | *???* |
