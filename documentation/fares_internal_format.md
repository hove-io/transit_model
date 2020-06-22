# Representing fares in Navitia Transit Model
## Overview
This document describes how fare structures are modeled in Navitia Transit Model. The internal model is composed of the followning objects:
* Ticket
* OD Rules

## Model details
### Ticket
This object contains the necessary information to specify a ticket.

| Property          | Type    | Required | Description                                                                                                                           |
| ----------------- | ------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| id                | String  | Yes      | Ticket identifier                                                                                                                     |
| name              | String  | No       | Ticket name                                                                                                                           |
| start_date        | Date    | Yes      | Start date for the validity period of the ticket price.                                                                               |
| end_date          | Date    | Yes      | End date for the validity period of the ticket price. This date is included in the validity period interval.                          |
| currency_type     | String  | Yes      | The currency used to pay the ticket. The [ISO 4217](https://en.wikipedia.org/wiki/ISO_4217#Active_codes) currency codes are used.     |
| price             | Float   | Yes      | The total price of the ticket in the currency specified by `currency_type`, including any additional fee, if any (e.g. boarding fee). |
| validity_duration | Integer | No       | Validity duration of the ticket in seconds. If this field is empty or set to 0, no duration limit is applied for the `Ticket`.        |
| transfers         | Integer | No       | Number of transfers allowed for the ticket. If this field is empty, unlimited transfers are allowed with the same `Ticket`.           |

### OD Rules
This object specifies how a `Ticket` depending on origin and destination stations is used and applied in a Navitia journey.

| Property                 | Type   | Required | Description                                                        |
| ------------------------ | ------ | -------- | ------------------------------------------------------------------ |
| id                       | String | Yes      | Rule identifier                                                    |
| ticket_id                | String | Yes      | Id of the `Ticket` to which this `OD Rule` is applied.             |
| origin_stop_area_id      | String | Yes      | Id of the origin stop_area in Navitia.                             |
| destination_stop_area_id | String | Yes      | Id of the destination stop_area in Navitia.                        |
| line_id                  | String | No       | Id of the line in Navitia for which the `Ticket` is applicable.    |
| network_id               | String | No       | Id of the network in Navitia for which the `Ticket` is applicable. |
| physical_mode_id         | String | No       | Id of the physical mode in Navitia applicable for this OD.         |
