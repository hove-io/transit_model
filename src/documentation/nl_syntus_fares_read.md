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
Each *FareFrame* specified in the input fare data corresponds to several `Tickets` in NTM (as many as the elements of the *DistanceMatrix*). For each *DistanceMatrixElement*, one `Ticket` object with the corresponding `OD Rules` object is created, unless the origin/destination stops cannot be identified in the NTFS (see the mapping rule below).

The current version of the connector does not describe the NTM properties that are not specified in the source data (e.g. the validity duration of a ticket).

### Ticket
NTM Property | Source frame | Source element | Notes/Mapping rule
--- | --- | --- | ---
id | *FareFrame* | *DistanceMatrixElement{id}* |
start_date | *ResourceFrame* | *versions/Version/StartDate* |
end_date | *ResourceFrame* | *versions/Version/EndDate* |
currency_type | *FareFrame* | *FrameDefaults/DefaultCurrency* |
price | | | See the mapping rule below.

**Computing the ticket price**

The ticket price is calculated by adding the boarding fee to the price specified for the origin-destination pair:
- The boarding fee is equal to the value of *FareFrame/EntranceRateWrtCurrency*.
- The OD price is calculated differently based on the *FareStructure* type:
  - if the *FareStructure* type is a *DirectPriceMatrix*, the value of *DistanceMatrixElementPrice/Amount* is multiplied by the value of *DistanceMatrixElementPrice/Units*.
  - if the *FareStructure* type is a *UnitPrice*, the value of *DistanceMatrixElementPrice/Distance* is multiplied by the value of *GeographicalIntervalPrice/Amount* and then by the value of *GeographicalIntervalPrice/Units*.

If *FareFrame/RoundingWrtCurrencyRule* is specified, a rounding rule for the specified `currency_type` is applied to the computed ticket price. For example, if the value is set to `0.01` for the currency `EUR`, then the ticket price is rounded to the nearest euro cent.

If the computed ticket price exceeds the value of *FareFrame/CappingWrtCurrencyRule*, then the latter is taken into account.

### OD Rule
NTM property | Source frame | Source element | Notes/Mapping rule
--- | --- | --- | ---
id | *FareFrame* | *DistanceMatrixElement{id}* | The id is prefixed with `OD:`.
ticket_id | | | Id of the `Ticket` to which this `OD Rule` is applied.
origin_stop_area_id | *FareFrame* | *DistanceMatrixElement/StartStopPointRef{ref}* | See the mapping rule below.
destination_stop_area_id | *FareFrame* | *DistanceMatrixElement/EndStopPointRef{ref}* | See the mapping rule below.
physical_mode_id | | | Fixed value `Bus`. This field will be necessary for modeling transitions when writing the NTFS fares.

**Setting the origin_stop_area_id and the destination_stop_area_id of an OD Rule**

Finding the right stops in the NTFS to which a fare is applied is not straightforward. The stops in the *FareFrame* point to the *ScheduledStopPoint*s in the *ServiceFrame*. The *ScheduledStopPoint*s are composed of *PointProjection*s that are referenced in the NTFS.

The *origin_stop_area_id* should have an associated stop_point with a complementary code of type `gtfs_stop_code` that matches the value of *ProjectedPointRef{ref}* (without the network prefixe, if any) of the *ScheduledStopPoint* in the *ServiceFrame* whose *id* is referenced by *StartStopPointRef{ref}* in the *DistanceMatrixElement*.

The *destination_stop_area_id* should have an associated stop_point with a complementary code of type `gtfs_stop_code` that matches the value of *ProjectedPointRef{ref}* (without the network prefixe, if any) of the *ScheduledStopPoint* in the *ServiceFrame* whose *id* is referenced by *EndStopPointRef{ref}* in the *DistanceMatrixElement*.

If no matching is found for the origin or the destination stop, then the stop is ignored and no rule is created. In this case, the corresponding `Ticket` is discarded.

If the origin or the destination stop is matched more than once, then an OD rule is created for each possible combination of origin-destination pairs. In this case, the id of each rule is suffixed by an auto-incremental integer and all the rules correspond to the same `Ticket` (with the same price).

Note that multiple tickets (with a different price) are not allowed for a given origin-destination pair. In case an OD rule already exists for an origin-destination pair, all additional rules for the same pair will be ignored.