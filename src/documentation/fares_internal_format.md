# Representing fares in Navitia Transit Model
## Overview
This document describes how fare structures are modeled in Navitia Transit Model. The internal model is composed of the followning objects:
- Ticket
- Rules

## Model details
### Ticket
This object contains the necessary information to specify a ticket.

Property | Type | Required | Description
--- | --- | --- | ---
id | UUID | Yes | Unique identifier
input_data_format | Enum | Yes | Source of the fare data, fixed value `netex_fares_nl`.
input_ticket_id | String | No | Id of the ticket found in the source data.
start_date | Date | Yes | Start date for the validity period of the ticket price.
end_date | Date | Yes | End date for the validity period of the ticket price. This date is included in the validity period interval.
currency_type | String | Yes | The currency used to pay the ticket. The [ISO 4217](https://en.wikipedia.org/wiki/ISO_4217#Active_codes) currency codes are used.
price | Float | Yes | The total price of the ticket in the currency specified by `currency_type`, including any additional fee, if any (e.g. boarding fee).
validity_duration | Integer | No | Validity duration of the ticket in seconds.
transfers | Integer | No | Number of transfers allowed for the ticket.

### Rules
This object specifies how a `Ticket` is used and applied in a Navitia trip.

Property | Type | Required | Description
--- | --- | --- | ---
id | UUID | Yes | Unique identifier
ticket_id | UUID | Yes | Id of the `Ticket` to which this `Condition` is applied.
origin_stoppoint_id | String | No | Id of the origin stop_point in Navitia.
origin_line_id | String | No | Id of the line in Navitia used in the first section of the trip for which the `Ticket` is applicable.
origin_network_id | String | No | Id of the network in Navitia used in the first section of the trip for which the `Ticket` is applicable.
dest_stoppoint_id | String | No | Id of the destination stop_point in Navitia.
dest_line_id | String | No | Id of the line in Navitia used in the last section of the trip for which the `Ticket` is applicable.
dest_network_id | String | No | Id of the network in Navitia used in the last section of the trip for which the `Ticket` is applicable.
