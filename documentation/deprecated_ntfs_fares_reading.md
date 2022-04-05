# Reading deprecated NTFS fares
## Introduction
This document describes how [NTFS fares](https://github.com/hove-io/ntfs-specification/blob/master/ntfs_fare_extension_fr_deprecated.md) are loaded in Navitia Transit Model.

In this initial version:
* only tickets on origin-destination stops are taken into account
* only transitions based on physical modes are taken into account, all other constraints are ignored (e.g validity duration, number of transfers allowed, constraints on networks/lines)
* tickets specified in a currency different than EUR are ignored
* only tickets on origin-destination stops are taken into account.

These limitations will be covered in later version, following the updates of the NTM fares model.

In the following, the NTM properties that are not specified are ignored and not detailed.

### Loading Tickets

| NTM property  | NTFS file  | NTFS field                    | Note/mapping rule                                                                                 |
| ------------- | ---------- | ----------------------------- | ------------------------------------------------------------------------------------------------- |
| id            | prices.csv | \*clef de ticket\*            |                                                                                                   |
| name          | prices.csv | \*name\*                      |                                                                                                   |
| start_date    | prices.csv | \*date de début de validité\* |                                                                                                   |
| end_date      | prices.csv | \*date de fin de validité\*   | The previous date of the specified date in the input.                                             |
| currency_type | prices.csv | \*devise\*                    | The currency is set to `EUR` when the input value is `centime`. Otherwise, the ticket is ignored. |
| price         | prices.csv | \*prix\*                      | The specified input value is converted into EUR.                                                  |

### Loading OD Rules

| NTM property             | NTFS file    | NTFS field         | Note/mapping rule                                                                                                                                              |
| ------------------------ | ------------ | ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| id                       | prices.csv   | \*clef de ticket\* | Id of the `OD Rule`. The id is prefixed with `OD:`.                                                                                                            |
| ticket_id                |              |                    | Id of the `Ticket` to which this `OD Rule` is applied.                                                                                                         |
| origin_stop_area_id      | od_fares.csv | Origin ID          | Id of the origin stop_area in Navitia when the value of "Origin mode" is set to `stop`. Otherwise, the rule and the corresponding ticket is ignored.           |
| destination_stop_area_id | od_fares.csv | Destination ID     | Id of the destination stop_area in Navitia when the value of "Destination mode" is set to `stop`. Otherwise, the rule and the corresponding ticket is ignored. |
| physical_mode_id         | fares.csv    | après changement   | Id of the physical mode in Navitia associated to this `OD Rule` when the value of "condition globale" is set to `with_changes`.                                |
