# Writing NTFS fares
## Introduction
This document describes how fares specified in Navitia Transit Model are transformed into a [NTFS fare feed](https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/ntfs_fare_extension_fr.md).

In this initial version: 
- only tickets on origin-destination stops are taken into account
- if a ticket is specified for a specific line or network, this information is ignored
- the validity duration and the transfers allowed for a ticket, if specified, are ignored
- tickets specified in a currency different than EUR are ignored.

These limitations will be adressed in a later version.

In the following, the NTFS fields that are not specified are ignored and not detailed.

### prices.csv
As a reminder, this file has no header and the order of the NTFS fields must be respected.

NTFS field | NTM object | NTM property | Notes/Mapping rule
--- | --- | --- | ---
\*clef de ticket\* | Ticket | ticket_id |
\*date de début de validité\* | Ticket | start_date | Starting date of the validity period of the fare structure in the form YYYYMMDD.
\*date de fin de validité\* | Ticket | end_date | The date after the specified end date in the form YYYYMMDD.
\*prix\* | Ticket | price | The specified value is converted into euro cents.
\*name\* | | | Fixed value `Ticket Orgine-Destination`.
\*champ ignoré\* | | | This field is explicitly left empty.
\*commentaire\* | | | This field is explicitly left empty.
\*devise\* | Ticket | currency_type | The value is set to `centime` provided that the currency used for the ticket is EUR.

### od_fares.csv

NTFS field | NTM object | NTM property | Notes/Mapping rule
--- | --- | --- | ---
Origin ID | OD Rules | origin_stoparea_id | 
Origin mode | | | Fixed value `stop`.
Destination ID | OD Rules | dest_stoparea_id | 
Destination mode | | | Fixed value `stop`.
ticket_id | OD Rules | ticket_id | Link to the ticket specified in [prices](#pricescsv)

### fares.csv
As the NTM fares model does not yet support special conditions for tickets, no content is written in this file (empty file with a header row).