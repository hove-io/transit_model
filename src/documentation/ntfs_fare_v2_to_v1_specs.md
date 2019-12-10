# Converting NTFS fares from V2 to V1 model
## Introduction
This document describes how [NTFS fare model](https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fare_extension.md) are converted to the [deprecated fare model](https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fare_extension_fr_deprecated.md).

This conversion is only used for a limited timeframe until main navitia product can read the new model. 
The only possible conversions are:
* an OD fare on a specific line
* a flat fare shared across several networks, allowing transitions within the networks with the same ticket.

## Conversion of an OD fare on a specific line
### Description in V2 model
An OD fare on a specific line is described in the V2 fare system as:
* A ticket in `tickets.txt` (with an ID) with:
  * At least one `ticket price` with a validity period, and only a euro currency. It is possible to have Different prices with different validity periods for the same ticket to handle changes in ticket prices.
  * At least one `ticket use` not allowing transfers, and empty bording and alighting times
  * At least one `ticket use perimeter` with only the specified line (and no exclusion)
  * At least one `ticket use restriction` of type `OD`

### Description of the transformation in V1 format

#### prices.csv
In this file, one line will be created per `ticket price` as follow:

prices.csv field | Source file | Source field | Notes/Mapping rule
--- | --- | --- | ---
"clef de ticket" | tickets.txt | ticket_id |
"date de début de validité" | ticket_prices.txt | ticket_validity_start |
"date de fin de validité" | ticket_prices.txt | ticket_validity_end | The day after `ticket_validity_end` (v1 format excludes the end date)
"prix" | ticket_prices.txt | ticket_price | The price has to be converted from euros to centimes.
"name" | tickets.txt | ticket_name |
"champ ignoré" | | |
"commentaire" | tickets.txt | ticket_comment |
"devise" | ticket_prices.txt | ticket_currency | fixed value `centime`, and only for a `EUR` value of `ticket_currency`

#### fares.csv

fares.csv field | Source file | Source field | Notes/Mapping rule
--- | --- | --- | ---
"avant changement" |  |  | fixed value `*`
"après changement" | ticket_use_perimeters.txt | object_id | the content of the field must follow the pattern : "line=line:[object_id]"
"début trajet" | ticket_use_restrictions.txt | use_origin | the content of the field must follow the pattern : "stoparea=stop_area:[use_origin]"
"fin trajet" | ticket_use_restrictions.txt | use_destination | the content of the field must follow the pattern : "stoparea=stop_area:[use_destination]"
"condition globale" |  |  | this field is empty
"clef ticket" | tickets.txt | ticket_id |

Note that more that one `ticket uses` are allowed for the same `ticket` specifying additional `ticket use perimeters` and/or `ticket use restrictions`. In this case, the associated transitions for the `ticket` should be generated accordingly.

#### od_fares.csv

This file must be created without any data.


## Conversion of a flat fare on specific networks

### Description in V2 model
A flat fare on a set of specific networks is described in the V2 fare system as:
* A ticket in `tickets.txt` (with an ID) with:
  * At least one `ticket price` with a validity period, and only a euro currency. It is possible to have Different prices with different validity periods for the same ticket to handle changes in ticket prices.
  * At least one `ticket use` without constraint on transfers, and empty boarding and alighting times
  * One `ticket use perimeter` for each specified network, all associated to the same `ticket_use` (and no exclusion)
  * No `ticket use restriction`

Warning: If different 2 tickets with several networks share a commun network, unexpected behaviour will arise (see chapter [`Limitations`](#limitations) below).

### Description of the transformation in V1 format

#### prices.csv
In this file, one line will be created per `ticket price` as follow:

prices.csv field | Source file | Source field | Notes/Mapping rule
--- | --- | --- | ---
"clef de ticket" | tickets.txt | ticket_id |
"date de début de validité" | ticket_prices.txt | ticket_validity_start |
"date de fin de validité" | ticket_prices.txt | ticket_validity_end | The day after `ticket_validity_end` (v1 format excludes the end date)
"prix" | ticket_prices.txt | ticket_price | The price has to be converted from euros to centimes.
"name" | tickets.txt | ticket_name |
"champ ignoré" | | |
"commentaire" | tickets.txt | ticket_comment |
"devise" | ticket_prices.txt | ticket_currency | fixed value `centime`, and only for a `EUR` value of `ticket_currency`

#### fares.csv
For each network of each `ticket_use`, a transition from anything to board on this network is firstly defined:

fares.csv field | Source file | Source field | Notes/Mapping rule
--- | --- | --- | ---
"avant changement" |  |  | fixed value `*`
"après changement" | ticket_use_perimeters.txt | object_id | the content of the field must follow the pattern : "network=network:[object_id]"
"début trajet" |  |  | this field is empty
"fin trajet" |  |  | this field is empty
"condition globale" |  |  | this field is empty
"clef ticket" | tickets.txt | ticket_id |

Then, another transition is defined for each combination of all the networks referenced by the current `ticket_use` (including every network with itself). As transitions are directional, both combination (A->B and B->A) are created.
Those new transitions have a `clef ticket` field empty to indicate the same ticket is to be used for the connection.

#### od_fares.csv
This file must be created without any data.

### Limitations
This conversion into V1 model has drawbacks if two tickets can be used to travel on multiple networks (allowing transfers) and with a common network.
In this case, some unexpected transfers will be possible.

For example, the simplest unexpected behaviour can occur with:
* a ticket `ticket1` usable on networks `network:01` and `network:02`
* a ticket `ticket2` usable on networks `network:02` and `network:03`

NTFS V1 modelization of such a case is represented by the following (inserting unvalid comments starting with # to explaint it): 
```
avant changement;après changement;début trajet;fin trajet;condition globale;clef ticket
# declaration of the starting use of the ticket ticket1
*;network=network:01;;;;ticket1
*;network=network:02;;;;ticket1
# declaration of the starting use of the ticket ticket2
*;network=network:02;;;;ticket2
*;network=network:03;;;;ticket2
# declaration of the transitions for ticket 1 without linking a ticket for allowing the use of the same ticket
network=network:01;network=network:01;;;;
network=network:01;network=network:02;;;;
network=network:02;network=network:01;;;;
network=network:02;network=network:02;;;;
# declaration of the transitions for ticket 2 without linking a ticket for allowing the use of the same ticket
network=network:02;network=network:02;;;;
network=network:02;network=network:03;;;;
network=network:03;network=network:02;;;;
network=network:03;network=network:03;;;;
```
In this case, following situations are possible:
* with a `ticket1`, boarding in the `network:02` and make a transfer to board a line on `network:03` is possible,
* with a `ticket2`, boarding in the `network:02` and make a transfer to board a line on `network:01` is possible.
