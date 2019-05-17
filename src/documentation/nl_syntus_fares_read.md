# Syntus fares reading specification
## Introduction
This document describes how fares provided by Keolis Netherlands are read in Navitia Transit Model (NTM).

## Input data description
The fare data expected at the input of the connector comes in the form of XML files based on [Netex fare exchange format](http://www.normes-donnees-tc.org/wp-content/uploads/2014/07/BNTRA-CN03-GT7_N0064_prCEN_TS_278330_FV_E-part_3-v9-1.pdf). Each file is composed of 3 basic elements:
- a *ResourceFrame* specifying the data owner and the validity period of the provided data
- a *ServiceFrame* specifying the network, the lines and the stops to which the fare structure is applied
- one or more *FareFrame*s specifying the fare structures and the underlying conditions.

The supported fare structures depend on origin-destination (OD) stop pairs in two ways:
- the ticket price between an origin and a destination is directly specified (*DirectPriceMatrix*)
- the price for a fare distance unit is specified, as well as the fare distance between an origin and a destination (*UnitPrice* & *DirectMatrix*). The ticket price between the origin and destination stops is computed by multiplying the fare distance by the fare distance unit.

## Connector description
Each *FareFrame* specified in the input fare data corresponds to several `Tickets` in NTM (as many as the elements of the *DistanceMatrix*). For each *DistanceMatrixElement*, one `Ticket` object is created specifying the associated line and the origin/destination stops.

The NTM properties that are not specified in the source data (e.g. the validity duration of a ticket) are not described below.

### Ticket
NTM Property | Source frame | Source element | Notes/Mapping rule
--- | --- | --- | ---
ticket_id | *FareFrame* | *DistanceMatrixElement{id}* |
ticket_name | | | Fixed value `Ticket Origin-Destination`.

### Ticket_price
NTM Property | Source frame | Source element | Notes/Mapping rule
--- | --- | --- | ---
ticket_id | | | Id of the `Ticket` to which this `Ticket_price` is applied.
ticket_price | | | See the mapping rule below.
ticket_currency | *FareFrame* | *FrameDefaults/DefaultCurrency* |
ticket_validity_start | *ResourceFrame* | *versions/Version/StartDate* |
ticket_validity_end | *ResourceFrame* | *versions/Version/EndDate* |

#### Computing the ticket price
The ticket price is calculated by adding the boarding fee to the price specified for the origin-destination pair:
- The boarding fee is equal to the value of *FareFrame/EntranceRateWrtCurrency*.
- The OD price is calculated differently based on the *FareStructure* type:
  - if the *FareStructure* type is a *DirectPriceMatrix*, the value of *DistanceMatrixElementPrice/Amount* is multiplied by the value of *DistanceMatrixElementPrice/Units*.
  - if the *FareStructure* type is a *UnitPrice*, the value of *DistanceMatrixElementPrice/Distance* is multiplied by the value of *GeographicalIntervalPrice/Amount* and then by the value of *GeographicalIntervalPrice/Units*.

If *FareFrame/RoundingWrtCurrencyRule* is specified, a rounding rule for the specified `currency_type` is applied to the computed ticket price. For example, if the value is set to `0.01` for the currency `EUR`, then the ticket price is rounded to the nearest euro cent.

If the computed ticket price exceeds the value of *FareFrame/CappingWrtCurrencyRule*, then the latter is taken into account.

### Ticket_use
NTM property | Source frame | Source element | Notes/Mapping rule
--- | --- | --- | ---
ticket_id | | | Id of the `Ticket` to which this `Ticket_use` is applied.
ticket_use_id | *FareFrame* | *DistanceMatrixElement{id}* | The id is prefixed with `TU:`.
max_transfers | | | Fixed value `0`.

### Ticket_use_perimeter
NTM property | Source frame | Source element | Notes/Mapping rule
--- | --- | --- | ---
ticket_use_id | | | Id of the `Ticket_use` to which this `Ticket_use_perimeter` is applied.
object_type | | | Fixed value `line`.
object_id | *FareFrame* | *TriggerObjectRef{ref}* | This value references the associated line in the *ServiceFrame*, for a *TriggerObjectRef{nameOfRefClass}* matching the value `Line`.  See the mapping rule below.
perimeter_action | | | Fixed value `1`.

### Setting the object_id in the Ticket Use Perimeter
Finding the line in the NTFS to which a fare is applied is not straightforward.

The *TriggerObjectRef{ref}* whose value of *nameOfRefClass* equals to `Line` indicates the line used in a *DistanceMatrix* (and therefore associated with all the enclosed elements). This line points to a *Line* in the *ServiceFrame* with a *KeyValue* element. The *object_id* references the *line_id* found in the NTFS (ignoring the NTFS applied prefix) that matches the *Value* of the *Key* equal to `KV1PlanningLijnNummer`.

### Ticket_use_restriction
NTM property | Source frame | Source element | Notes/Mapping rule
--- | --- | --- | ---
ticket_use_id | | | Id of the `Ticket_use` to which this `Ticket_use_restriction` is applied.
restriction_type | | | Fixed value `OD`.
use_origin | *FareFrame* | *DistanceMatrixElement/StartStopPointRef{ref}* | This field points to a stop_area found in the NTFS. See the mapping rule below.
use_destination | *FareFrame* | *DistanceMatrixElement/EndStopPointRef{ref}* | This field points to a stop_area found in the NTFS. See the mapping rule below.

#### Setting the use_origin and use_destination in the Ticket Use Restriction
Finding the right stops in the NTFS to which a fare is applied is not straightforward. 

The stops in the *FareFrame* point to the *ScheduledStopPoint*s in the *ServiceFrame*. The *ScheduledStopPoint*s are composed of *PointProjection*s that are referenced in the NTFS as stop_points.

The *use_origin* refers to the stop_area found in the NTFS that has an associated stop_point whose *stop_id* (ignoring the NTFS applied prefix) matches the value of *ProjectedPointRef{ref}* (without the network prefix, if any) of the *ScheduledStopPoint* in the *ServiceFrame* having the *id* referenced by *StartStopPointRef{ref}* in the *DistanceMatrixElement*. 

The *use_destination* refers to the stop_area found in the NTFS that has an associated stop_point whose *stop_id* (ignoring the NTFS applied prefix) matches the value of *ProjectedPointRef{ref}* (without the network prefix, if any) of the *ScheduledStopPoint* in the *ServiceFrame* having the *id* referenced by *EndStopPointRef{ref}* in the *DistanceMatrixElement*.

If no matching is found for the origin or the destination stop, then the stop is ignored and the corresponding `Ticket` is discarded.

### Special NS Ticket
After loading the Tickets associated with the input data, a special ticket is loaded (only once) that will be necessary for modeling transitions when writing the NTFS fares. This special ticket has the following fixed properties:

NTM object | NTM property | Value 
--- | --- | --- 
Ticket | ticket_id | Fixed value `ticket_NS`.
Ticket | ticket_name | Fixed value `Ticket NS`.
Ticket_use | ticket_id | Fixed value `ticket_NS`.
Ticket_use | ticket_use_id | Fixed value `ticket_use_NS`.
Ticket_use | max_transfers | Fixed value `0`.
Ticket_use_perimeter | ticket_use_id | Fixed value `ticket_use_NS`.
Ticket_use_perimeter | object_type | Fixed value `network`.
Ticket_use_perimeter | object_id | The network_id found in the NTFS correspondinf to the network named `NS`.
Ticket_use_perimeter | perimeter_action | Fixed value `1`.
