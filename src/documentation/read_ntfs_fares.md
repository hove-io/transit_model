# Reading NTFS fares
## Introduction
This document describes how [NTFS fares](https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/ntfs_fare_extension_fr.md) are loaded in Navitia Transit Model.

In this initial version: 
- only tickets on origin-destination stops are taken into account
- the NTFS file *fares.csv* is ignored, as the NTM fares model does not yet support special conditions on tickets (e.g validity duration, number of transfers allowed, constraints on networks/lines) 
- tickets specified in a currency different than EUR are ignored
- only tickets on origin-destination stops are taken into account.

These limitations will be covered in later version, following the updates of the NTM fares model.

In the following, the NTM properties that are not specified are ignored and not detailed.

### Loading Tickets

NTM property | NTFS file | NTFS field | Note/mapping rule
--- | --- | --- | ---
id | prices.csv | \*clef de ticket\* | 
start_date | prices.csv | \*date de début de validité\* | 
end_date | prices.csv | \*date de fin de validité\* | The previous date of the specified date in the input.
currency_type | prices.csv | \*devise\* | The currency is set to `EUR` when the input value is `centime`. Otherwise, the ticket is ignored.
price | prices.csv | \*prix\* | The specified input value is converted into EUR.

### Loading OD Rules

NTM property | NTFS file | NTFS field | Note/mapping rule
--- | --- | --- | ---
id | prices.csv | \*clef de ticket\* | Id of the `OD Rule`. The id is prefixed with `OD:`.
ticket_id | | | Id of the `Ticket` to which this `OD Rule` is applied.
origin_stoparea_id | od_fares.csv | Origin ID | Id of the origin stop_area in Navitia when the value of Origin mode is set to `stop`. Otherwise, the rule and the corresponding ticket is ignored.
dest_stoparea_id | od_fares.csv | Destination ID | Id of the destination stop_area in Navitia when the value of Destination mode is set to `stop`. Otherwise, the rule and the corresponding ticket is ignored.