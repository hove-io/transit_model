# Converting NTFS fares from V2 to V1 model
## Introduction
This document describes how [NTFS fare model](https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/ntfs_fare_extension.md) are converted to the [deprecated fare model](https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/ntfs_fare_extension_fr_deprecated.md).

This conversion is only used for a limited timeframe until main navitia product can read the new model.
The only possible conversions are:
* an OD fare on a specific line
* a flat fare on a specific network

## Conversion of an OD fare on a specific line
### Description in V2 model
An OD fare on a specific line is described in the V2 fare system as:
* A ticket in `tickets.txt` (with an ID) with:
  * A `ticket price` with a validity period, and only a euro currency
  * One `ticket use` not allowing transfers, and empty bording and alighting times
  * One `ticket use perimeter` with only the specified line (and no exclusion)
  * One `ticket use restriction` of type `OD`

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

prices.csv field | Source file | Source field | Notes/Mapping rule
--- | --- | --- | ---
"avant changement" |  |  | fixed value `*`
"après changement" | ticket_use_perimeter.txt | object_id | the content of the field must follow the pattern : "line=line:[object_id]"
"début trajet" | ticket_use_restriction.txt | use_origin | the content of the field must follow the pattern : "stoparea=stop_area:[use_origin]"
"fin trajet" | ticket_use_restriction.txt | use_destination | the content of the field must follow the pattern : "stoparea=stop_area:[use_destination]"
"condition globale" |  |  | this field is empty
"clef ticket" | tickets.txt | ticket_id |

#### od_fares.csv

This file must be created without any data.


## Conversion of a flat fate on a specific network

### Description in V2 model
An flat fare on a specific network is described in the V2 fare sytem as:
* A ticket in `tickets.txt` (with an ID) with:
  * A `ticket price` with a validity period, and only a euro currency
  * One `ticket use` without constrain on transfers, and empty bording and alighting times
  * One `ticket use perimeter` with only the specified network (and no exclusion)
  * No `ticket use restriction`

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

First, a transition from anything to board in the network is defined:

prices.csv field | Source file | Source field | Notes/Mapping rule
--- | --- | --- | ---
"avant changement" |  |  | fixed value `*`
"après changement" | ticket_use_perimeter.txt | object_id | the content of the field must follow the pattern : "network=network:[object_id]"
"début trajet" |  |  | this field is empty
"fin trajet" |  |  | this field is empty
"condition globale" |  |  | this field is empty
"clef ticket" | tickets.txt | ticket_id |

Then a transition allowing a transfert from one section of the network to another section of the same network:

prices.csv field | Source file | Source field | Notes/Mapping rule
--- | --- | --- | ---
"avant changement" | ticket_use_perimeter.txt | object_id | the content of the field must follow the pattern : "network=network:[object_id]"
"après changement" | ticket_use_perimeter.txt | object_id | the content of the field must follow the pattern : "network=network:[object_id]"
"début trajet" |  |  | this field is empty
"fin trajet" |  |  | this field is empty
"condition globale" |  |  | this field is empty
"clef ticket" | tickets.txt | ticket_id |

#### od_fares.csv

This file must be created without any data.
