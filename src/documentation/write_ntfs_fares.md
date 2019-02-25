# Writing NTFS fares
## Introduction
This document describes how fares specified in Navitia Transit Model are transformed into a [NTFS fare feed](https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/ntfs_fare_extension_fr.md).

In this initial version: 
- only the files [prices.csv](#pricescsv) and [od_fares.csv](#od_farescsv) are written, as the NTM fares model does not yet support special conditions for the tickets
- only tickets on origin-destination stops are taken into account.
- tickets specified in a currency different than EUR are ignored.

These limitations will be adressed in a later version.

### prices.csv
As a reminder, this file has no header and the order of the NTFS fields must be respected.
NTFS field | NTM object | NTM property | Notes/Mapping rule
--- | --- | --- | ---
\*clef de ticket\* | Ticket | ticket_id
\*date de début de validité\* | Ticket | start_date | Starting date of the validity period of the fare structure in the form YYYYMMDD.
\*date de fin de validité\* | Ticket | end_date | The date after the specified end date in the form YYYYMMDD.
\*prix\* | Ticket | price | The specified value is converted into euro cents.
\*name\* | | | Fixed value `Ticket Orgine-Destination`.
\*champ ignoré\* |  |  | This field is explicitly left empty.
\*commentaire\* | | | This field is explicitly left empty.
\*devise\* | Ticket | currency_type | The value is set to `centime` provided that the currency used for the ticket is EUR.

### od_fares.csv

NTFS field | NTM object | NTM property | Notes/Mapping rule
--- | --- | --- | ---
Origin ID | OD Rules | origin_stoparea_id | 
Origin name | | This field is explicitly left empty.
Origin mode | | Fixed value `stop`.
Destination ID | OD Rules | dest_stoparea_id | 
Destination name | | This field is explicitly left empty.
Destination mode | | Fixed value `stop`.
ticket_id | OD Rules | ticket_id | Link to the ticket specified in [prices](#pricescsv)

