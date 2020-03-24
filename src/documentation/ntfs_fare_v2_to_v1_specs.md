# Converting NTFS fares from V2 to V1 model
## Introduction

This document describes how [NTFS fare model](https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fare_extension.md) are converted to the [deprecated fare model](https://github.com/CanalTP/ntfs-specification/blob/master/ntfs_fare_extension_fr_deprecated.md).

This conversion is only used for a limited timeframe until main navitia product can read the new model. 

The conversion will be illustrated in two steps on the following example. 
On the first step, we will consider the case where there is nothing in ticket_use_restrictions.txt. 
The second step will take into account ticket_use_restrictions.txt. 

### Data Example

tickets.txt

ticket_id    | ticket_name    | ticket_comment
  ---        |   ---          |    ----
my_ticket_id | My Ticket Name | My Ticket Comment

ticket_prices.txt

ticket_id   | ticket_price| ticket_currency| ticket_validity_start| ticket_validity_end
---         | ---         | ---            | ---                  | ----
my_ticket_id| 1.13        | EUR            | 20190101             | 20191231


ticket_uses.txt

ticket_use_id | ticket_id    | max_transfers | boarding_time_limit | alighting_time_limit
---           | ---          | ---           | ---                 | ----
my_use_id     | my_ticket_id |   2           |   60                | 90 


ticket_use_perimeters.txt

ticket_use_id | object_type | object_id        | perimeter_action
 ---          |  ---        | ---              | ---  
my_use_id     | network     | my_network       | 1
my_use_id     | line        | my_line          | 1
my_use_id     | line        | excluded_line    | 2


### Price conversion

The above example will yield the following prices.csv file :

prices.csv

clef ticket | debut validite | fin validite | prix | nom               |     | commentaire                  | devise
  ---       |   ---          |  ---         | ---  | ---               | --- |   ----                       |  ---  
  my_use_id |   20190101     | 20200101     | 113  |  My Ticket Name   |     |  My Ticket Comment           | centime


There are several things to notice :
- the identifier for the ticket in prices.csv is the *ticket_use_id* and not the ticket_id. Indeed, the NTFS fare model
  allows for multiple ticket_use_id associated with the same ticket_id. Hence, to disambiguate, we will use the ticket_use_id
  as the reference identifier throughout the conversion
- the end of validity date is incremented by one day in prices.csv compared to ticket_prices.txt. This is because the deprecated 
  fare model, the end of validity date is interpreted as "the earliest unauthorized day", whereas the NTFS fare model specify it 
  as "the latest authorized day"
- the price value is converted to "centimes" because the NTFS fare model expect an integer value. Consequently, we support only prices in EUR
  in the NTFS fare model. Tickets with a price not given in EUR will be throwed away during the conversion.


### Without ticket_use_restrictions

#### Punching a new ticket

To model our example, we first add two lines to fares.csv in order to allow to punch the ticket.

fares.csv

| avant changement | apres changement   | debut trajet                                      | fin trajet    | condition globale | clef ticket |
| ---              |    ---             |  ---                                              | ---           |   ---             |  ---        |
|                  | network=my_network | line!=excluded_line & duration<61 & nb_changes<3  | duration<91   |                   | my_use_id   |
|                  | line=my_line       | line!=excluded_line & duration<61 & nb_changes<3  | duration<91   |                   | my_use_id   |

Each line allows entering one of the two included perimeters : the network my_network and the line my_line.
Note that the exluded perimeter (excluded line) is added as a condition "debut trajet".
The conditition "debut trajet" also encodes the contraints on the boarding duration and the number of transfers.
The condition "fin trajet" encodes the constraint on the alighting duration.
The value my_use_id in the "clef ticket" field means that we will punch (and thus pay for) a ticket my_use_id when using these transitions.

#### Transfers with a punched ticket

Now that we allowed punching a new ticket, we need to allow transfers with a ticket that has been punched.
This is the purpose of the following lines added to fares.csv

fares.csv (continued)

| avant changement    | apres changement   | debut trajet                                                         | fin trajet    | condition globale | clef ticket |
| ---                 |    ---             |  ---                                                                 | ---           |   ---             |  --- |
|  network=my_network | network=my_network | ticket=my_use_id & line!=excluded_line & duration<61 & nb_changes<3  | duration<91   |                   |      |
|  line=my_line       | line=my_line       | ticket=my_use_id & line!=excluded_line & duration<61 & nb_changes<3  | duration<91   |                   |      |
|  network=my_network | line=my_line       | ticket=my_use_id & line!=excluded_line & duration<61 & nb_changes<3  | duration<91   |                   |      |
|  line=my_line       | network=my_network | ticket=my_use_id & line!=excluded_line & duration<61 & nb_changes<3  | duration<91   |                   |      |
 
We add a line for each pair of included perimeters, so as to allow transfers between all perimeters.
We add a constraint "ticket=my_use_id" so as to allow these transfers only if a ticket my_use_id has already been punched (and not another one).
Note that these "transfers" lines will not be added when the maximum number of changes allowed is zero.


### With ticket_use_restrictions

We now enrich our example with the following data in ticket_use_restrictions.

ticket_use_restrictions.txt

ticket_use_id | restriction_type | use_origin     | use_destination
 ---          |    ---           |  ---           | --- 
my_use_id     |    OD            | my_origin      | my_destination
my_use_id     |    zone          | my_zone        | my_zone

This means that we can use the ticket my_use_id either between the stop areas my_origin and my_destination or between two stops in the zone my_zone, while satisfying the other constraints of the ticket.
In order to model this, we will put in fares.csv, for each restriction, a copy of the lines described in [the previous section](#without-ticketuserestrictions),
enhanced with the extra constraints corresponding to the restriction.


| avant changement    | apres changement   | debut trajet                                                                                | fin trajet                                | condition globale | clef ticket |
| ---                 |    ---             |  ---                                                                                        | ---                                       |   ---             |  ---        |
|                     | network=my_network | line!=excluded_line & duration<61 & nb_changes<3 & stoparea = my_origin                     | duration<91  & stoparea = my_destination  |                   | my_use_id   |
|                     | line=my_line       | line!=excluded_line & duration<61 & nb_changes<3 & stoparea = my_origin                     | duration<91  & stoparea = my_destination  |                   | my_use_id   |
|  network=my_network | network=my_network | ticket=my_use_id & line!=excluded_line & duration<61 & nb_changes<3 & stoparea = my_origin  | duration<91  & stoparea = my_destination  |                   |             |
|  line=my_line       | line=my_line       | ticket=my_use_id & line!=excluded_line & duration<61 & nb_changes<3 & stoparea = my_origin  | duration<91  & stoparea = my_destination  |                   |             |
|  network=my_network | line=my_line       | ticket=my_use_id & line!=excluded_line & duration<61 & nb_changes<3 & stoparea = my_origin  | duration<91  & stoparea = my_destination  |                   |             |
|  line=my_line       | network=my_network | ticket=my_use_id & line!=excluded_line & duration<61 & nb_changes<3 & stoparea = my_origin  | duration<91  & stoparea = my_destination  |                   |             |
|                     | network=my_network | line!=excluded_line & duration<61 & nb_changes<3 & zone = my_zone                           | duration<91  & zone = my_zone             |                   | my_use_id   |
|                     | line=my_line       | line!=excluded_line & duration<61 & nb_changes<3 & zone = my_zone                           | duration<91  & zone = my_zone             |                   | my_use_id   |
|  network=my_network | network=my_network | ticket=my_use_id & line!=excluded_line & duration<61 & nb_changes<3 & zone = my_zone        | duration<91  & zone = my_zone             |                   |             |
|  line=my_line       | line=my_line       | ticket=my_use_id & line!=excluded_line & duration<61 & nb_changes<3 & zone = my_zone        | duration<91  & zone = my_zone             |                   |             |
|  network=my_network | line=my_line       | ticket=my_use_id & line!=excluded_line & duration<61 & nb_changes<3 & zone = my_zone        | duration<91  & zone = my_zone             |                   |             |
|  line=my_line       | network=my_network | ticket=my_use_id & line!=excluded_line & duration<61 & nb_changes<3 & zone = my_zone        | duration<91  & zone = my_zone             |                   |             |



## Shortcomings
Do not put too many OD tickets, or the fare engine performance may noticeably decrease. 
Indeed, the fare engine loops over all lines of fares.csv
at least once for each public transport section of each journey computed.
This performance hit could be avoided by using instead the od_fares.csv file, but this prevents enforcing other use cases mandated by 
the NTFS fare format (in particular, it is not possible to restrict an od_fares to a specific line or network).

