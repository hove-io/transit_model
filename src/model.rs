// Copyright 2017 Kisio Digital and/or its affiliates.
//
// This program is free software: you can redistribute it and/or
// modify it under the terms of the GNU General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see
// <http://www.gnu.org/licenses/>.

//! Definition of the navitia transit model.

use crate::{
    collection::{Collection, CollectionWithId, Id, Idx},
    objects::*,
    relations::{IdxSet, ManyToMany, OneToMany, Relation},
    Error, Result,
};
use chrono::NaiveDate;
use derivative::Derivative;
use failure::format_err;
use serde::{Deserialize, Serialize};
use std::cmp;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::ops;
use std::result::Result as StdResult;
use transit_model_procmacro::*;

/// The set of collections representing the model.
#[derive(Derivative, Serialize, Deserialize, Debug)]
#[derivative(Default)]
#[allow(missing_docs)]
pub struct Collections {
    pub contributors: CollectionWithId<Contributor>,
    pub datasets: CollectionWithId<Dataset>,
    pub networks: CollectionWithId<Network>,
    pub commercial_modes: CollectionWithId<CommercialMode>,
    pub lines: CollectionWithId<Line>,
    pub routes: CollectionWithId<Route>,
    pub vehicle_journeys: CollectionWithId<VehicleJourney>,
    pub physical_modes: CollectionWithId<PhysicalMode>,
    pub stop_areas: CollectionWithId<StopArea>,
    pub stop_points: CollectionWithId<StopPoint>,
    pub feed_infos: BTreeMap<String, String>,
    pub calendars: CollectionWithId<Calendar>,
    pub companies: CollectionWithId<Company>,
    pub comments: CollectionWithId<Comment>,
    pub equipments: CollectionWithId<Equipment>,
    pub transfers: Collection<Transfer>,
    pub trip_properties: CollectionWithId<TripProperty>,
    pub geometries: CollectionWithId<Geometry>,
    pub admin_stations: Collection<AdminStation>,
    #[serde(skip)]
    pub stop_time_headsigns: HashMap<(Idx<VehicleJourney>, u32), String>,
    #[serde(skip)]
    pub stop_time_ids: HashMap<(Idx<VehicleJourney>, u32), String>,
    #[serde(skip)]
    pub stop_time_comments: HashMap<(Idx<VehicleJourney>, u32), Idx<Comment>>,
    pub prices_v1: Collection<PriceV1>,
    pub od_fares_v1: Collection<ODFareV1>,
    pub fares_v1: Collection<FareV1>,
    pub tickets: CollectionWithId<Ticket>,
    pub ticket_uses: CollectionWithId<TicketUse>,
    pub ticket_prices: Collection<TicketPrice>,
    pub ticket_use_perimeters: Collection<TicketUsePerimeter>,
    pub ticket_use_restrictions: Collection<TicketUseRestriction>,
}

impl Collections {
    /// Merge the `Collections` parameter into the current `Collections` by consecutively merging
    /// each collections representing the model.  Fails in case of id collision.
    pub fn try_merge(&mut self, c: Collections) -> Result<()> {
        let Collections {
            contributors,
            datasets,
            networks,
            commercial_modes,
            lines,
            routes,
            mut vehicle_journeys,
            physical_modes,
            stop_areas,
            stop_points,
            calendars,
            companies,
            comments,
            equipments,
            transfers,
            trip_properties,
            geometries,
            admin_stations,
            stop_time_headsigns,
            stop_time_ids,
            stop_time_comments,
            prices_v1,
            od_fares_v1,
            fares_v1,
            tickets,
            ticket_uses,
            ticket_prices,
            ticket_use_perimeters,
            ticket_use_restrictions,
            ..
        } = c;
        self.contributors.try_merge(contributors)?;
        self.datasets.try_merge(datasets)?;
        self.networks.try_merge(networks)?;
        self.commercial_modes.merge(commercial_modes);
        self.lines.try_merge(lines)?;
        self.routes.try_merge(routes)?;
        self.physical_modes.extend(physical_modes);
        self.stop_areas.try_merge(stop_areas)?;
        self.prices_v1.merge(prices_v1);
        self.od_fares_v1.merge(od_fares_v1);
        self.fares_v1.merge(fares_v1);
        self.tickets.try_merge(tickets)?;
        self.ticket_uses.try_merge(ticket_uses)?;
        self.ticket_prices.merge(ticket_prices);
        self.ticket_use_perimeters.merge(ticket_use_perimeters);
        self.ticket_use_restrictions.merge(ticket_use_restrictions);

        fn get_new_idx<T>(
            old_idx: Idx<T>,
            old_idx_to_id: &HashMap<Idx<T>, String>,
            merged_collection: &CollectionWithId<T>,
        ) -> Option<Idx<T>> {
            old_idx_to_id
                .get(&old_idx)
                .and_then(|id| merged_collection.get_idx(id))
        }
        fn idx_to_id<T: Id<T>>(collection: &CollectionWithId<T>) -> HashMap<Idx<T>, String> {
            collection
                .iter()
                .map(|(idx, obj)| (idx, obj.id().into()))
                .collect()
        }

        let sp_idx_to_id = idx_to_id(&stop_points);
        let vj_idx_to_id = idx_to_id(&vehicle_journeys);
        let c_idx_to_id = idx_to_id(&comments);

        self.stop_points.try_merge(stop_points)?;

        // Update stop point idx in new stop times
        let mut vjs = vehicle_journeys.take();
        for vj in &mut vjs {
            for st in &mut vj.stop_times.iter_mut() {
                if let Some(new_idx) =
                    get_new_idx(st.stop_point_idx, &sp_idx_to_id, &self.stop_points)
                {
                    st.stop_point_idx = new_idx;
                }
            }
        }
        vehicle_journeys = CollectionWithId::new(vjs)?;
        self.vehicle_journeys.try_merge(vehicle_journeys)?;

        fn update_vj_idx<'a, T: Clone>(
            map: &'a HashMap<(Idx<VehicleJourney>, u32), T>,
            vjs: &'a CollectionWithId<VehicleJourney>,
            vj_idx_to_id: &'a HashMap<Idx<VehicleJourney>, String>,
        ) -> impl Iterator<Item = ((Idx<VehicleJourney>, u32), T)> + 'a {
            map.iter()
                .filter_map(move |((old_vj_idx, sequence), value)| {
                    get_new_idx(*old_vj_idx, vj_idx_to_id, vjs)
                        .map(|new_vj_idx| ((new_vj_idx, *sequence), value.clone()))
                })
        }

        // Update vehicle journey idx
        self.stop_time_headsigns.extend(update_vj_idx(
            &stop_time_headsigns,
            &self.vehicle_journeys,
            &vj_idx_to_id,
        ));

        self.stop_time_ids.extend(update_vj_idx(
            &stop_time_ids,
            &self.vehicle_journeys,
            &vj_idx_to_id,
        ));

        self.comments.try_merge(comments)?;
        let mut new_stop_time_comments = HashMap::new();
        for ((old_vj_idx, sequence), value) in &stop_time_comments {
            let new_vj_idx =
                get_new_idx(*old_vj_idx, &vj_idx_to_id, &self.vehicle_journeys).unwrap();
            let new_c_idx = get_new_idx(*value, &c_idx_to_id, &self.comments).unwrap();
            new_stop_time_comments.insert((new_vj_idx, *sequence), new_c_idx);
        }
        self.stop_time_comments.extend(new_stop_time_comments);
        self.calendars.try_merge(calendars)?;
        self.companies.try_merge(companies)?;
        self.equipments.try_merge(equipments)?;
        self.transfers.merge(transfers);
        self.trip_properties.try_merge(trip_properties)?;
        self.geometries.try_merge(geometries)?;
        self.admin_stations.merge(admin_stations);
        Ok(())
    }

    /// Restrict the validity period of the current `Collections` with the start_date and end_date
    pub fn restrict_period(&mut self, start_date: &NaiveDate, end_date: &NaiveDate) -> Result<()> {
        let mut calendars = self.calendars.take();
        for calendar in calendars.iter_mut() {
            calendar.dates = calendar
                .dates
                .iter()
                .cloned()
                .filter(|date| date >= start_date && date <= end_date)
                .collect();
        }
        let mut data_sets = self.datasets.take();
        for data_set in data_sets.iter_mut() {
            data_set.start_date = cmp::max(*start_date, data_set.start_date);
            data_set.end_date = cmp::min(*end_date, data_set.end_date);
        }
        self.datasets = CollectionWithId::new(data_sets)?;
        self.calendars = CollectionWithId::new(calendars)?;
        Ok(())
    }

    /// Keep the collections consistent for the new model by purging unreferenced data by
    /// calendars
    pub fn sanitize(&mut self) -> Result<()> {
        fn update_comments_used(
            comments_used: &mut HashSet<String>,
            comment_links: &CommentLinksT,
            comments: &CollectionWithId<Comment>,
        ) {
            comments_used.extend(comment_links.iter().map(|cl| comments[*cl].id.clone()));
        }
        fn update_comments_idx<T>(
            container: &mut Vec<T>,
            comment_old_idx_to_new_idx: &HashMap<Idx<Comment>, Idx<Comment>>,
        ) where
            T: CommentLinks,
        {
            for elt in container.iter_mut() {
                let links = elt.comment_links_mut();
                *links = links
                    .iter()
                    .map(|l| comment_old_idx_to_new_idx[l])
                    .collect::<BTreeSet<_>>();
            }
        }

        self.calendars.retain(|cal| !cal.dates.is_empty());

        let mut geometries_used: HashSet<String> = HashSet::new();
        let mut companies_used: HashSet<String> = HashSet::new();
        let mut trip_properties_used: HashSet<String> = HashSet::new();
        let mut route_ids_used: HashSet<String> = HashSet::new();
        let mut stop_points_used: HashSet<String> = HashSet::new();
        let mut data_sets_used: HashSet<String> = HashSet::new();
        let mut physical_modes_used: HashSet<String> = HashSet::new();
        let mut comments_used: HashSet<String> = HashSet::new();

        let vj_id_to_old_idx = self.vehicle_journeys.get_id_to_idx().clone();
        let comment_id_to_old_idx = self.comments.get_id_to_idx().clone();
        let stop_point_id_to_old_idx = self.stop_points.get_id_to_idx().clone();

        let mut vjs: Vec<VehicleJourney> = self
            .vehicle_journeys
            .take()
            .into_iter()
            .filter(|vj| {
                if self.calendars.get(&vj.service_id).is_some() {
                    if let Some(geo_id) = &vj.geometry_id {
                        geometries_used.insert(geo_id.clone());
                    }
                    if let Some(prop_id) = &vj.trip_property_id {
                        trip_properties_used.insert(prop_id.clone());
                    }
                    companies_used.insert(vj.company_id.clone());
                    route_ids_used.insert(vj.route_id.clone());
                    for stop_time in &vj.stop_times {
                        stop_points_used
                            .insert(self.stop_points[stop_time.stop_point_idx].id.clone());
                    }
                    data_sets_used.insert(vj.dataset_id.clone());
                    physical_modes_used.insert(vj.physical_mode_id.clone());
                    update_comments_used(&mut comments_used, &vj.comment_links, &self.comments);
                    return true;
                }
                false
            })
            .collect();
        let mut line_ids_used: HashSet<String> = HashSet::new();
        let mut routes = self
            .routes
            .take()
            .into_iter()
            .filter(|r| {
                if route_ids_used.contains(&r.id) {
                    if let Some(geo_id) = &r.geometry_id {
                        geometries_used.insert(geo_id.clone());
                    }
                    line_ids_used.insert(r.line_id.clone());
                    update_comments_used(&mut comments_used, &r.comment_links, &self.comments);
                    return true;
                }
                false
            })
            .collect::<Vec<_>>();
        let mut stop_area_ids_used: HashSet<String> = HashSet::new();
        let mut equipments_used: HashSet<String> = HashSet::new();
        let mut stop_points = self
            .stop_points
            .take()
            .into_iter()
            .filter(|sp| {
                if stop_points_used.contains(&sp.id) {
                    stop_area_ids_used.insert(sp.stop_area_id.clone());
                    if let Some(equipment_id) = &sp.equipment_id {
                        equipments_used.insert(equipment_id.clone());
                    }
                    update_comments_used(&mut comments_used, &sp.comment_links, &self.comments);
                    return true;
                }
                false
            })
            .collect::<Vec<_>>();
        let mut networks_used: HashSet<String> = HashSet::new();
        let mut commercial_modes_used: HashSet<String> = HashSet::new();
        let mut lines = self
            .lines
            .take()
            .into_iter()
            .filter(|l| {
                if line_ids_used.contains(&l.id) {
                    if let Some(geo_id) = &l.geometry_id {
                        geometries_used.insert(geo_id.clone());
                    }
                    networks_used.insert(l.network_id.clone());
                    commercial_modes_used.insert(l.commercial_mode_id.clone());
                    update_comments_used(&mut comments_used, &l.comment_links, &self.comments);
                    return true;
                }
                false
            })
            .collect::<Vec<_>>();
        let mut contributors_used: HashSet<String> = HashSet::new();
        self.datasets = CollectionWithId::new(
            self.datasets
                .take()
                .into_iter()
                .filter(|d| {
                    if data_sets_used.contains(&d.id) {
                        contributors_used.insert(d.contributor_id.clone());
                        return true;
                    }
                    false
                })
                .collect(),
        )?;
        let mut stop_areas = self
            .stop_areas
            .take()
            .into_iter()
            .filter(|sp| {
                if stop_area_ids_used.contains(&sp.id) {
                    update_comments_used(&mut comments_used, &sp.comment_links, &self.comments);
                    return true;
                }
                false
            })
            .collect::<Vec<_>>();

        self.comments
            .retain(|comment| comments_used.contains(&comment.id));
        let comment_old_idx_to_new_idx: HashMap<Idx<Comment>, Idx<Comment>> = self
            .comments
            .iter()
            .map(|(new_idx, comment)| (comment_id_to_old_idx[&comment.id], new_idx))
            .collect();

        update_comments_idx(&mut lines, &comment_old_idx_to_new_idx);
        self.lines = CollectionWithId::new(lines)?;
        update_comments_idx(&mut stop_points, &comment_old_idx_to_new_idx);
        self.stop_points = CollectionWithId::new(stop_points)?;
        let stop_point_old_idx_to_new_idx: HashMap<Idx<StopPoint>, Idx<StopPoint>> = self
            .stop_points
            .iter()
            .map(|(new_idx, stop_point)| (stop_point_id_to_old_idx[&stop_point.id], new_idx))
            .collect();
        for vj in vjs.iter_mut() {
            for st in vj.stop_times.iter_mut() {
                st.stop_point_idx = stop_point_old_idx_to_new_idx[&st.stop_point_idx];
            }
        }
        update_comments_idx(&mut stop_areas, &comment_old_idx_to_new_idx);
        self.stop_areas = CollectionWithId::new(stop_areas)?;
        update_comments_idx(&mut routes, &comment_old_idx_to_new_idx);
        self.routes = CollectionWithId::new(routes)?;
        update_comments_idx(&mut vjs, &comment_old_idx_to_new_idx);
        self.vehicle_journeys = CollectionWithId::new(vjs)?;

        let vj_old_idx_to_new_idx: HashMap<Idx<VehicleJourney>, Idx<VehicleJourney>> = self
            .vehicle_journeys
            .iter()
            .map(|(new_idx, vj)| (vj_id_to_old_idx[&vj.id], new_idx))
            .collect();
        self.stop_time_comments = self
            .stop_time_comments
            .iter()
            .filter_map(|((old_vj_id, seq), comment_old_idx)| {
                match (
                    vj_old_idx_to_new_idx.get(&old_vj_id),
                    comment_old_idx_to_new_idx.get(&comment_old_idx),
                ) {
                    (Some(new_vj_idx), Some(new_comment_idx)) => {
                        Some(((*new_vj_idx, *seq), *new_comment_idx))
                    }
                    _ => None,
                }
            })
            .collect();
        self.stop_time_ids = self
            .stop_time_ids
            .iter()
            .filter_map(|((old_vj_id, seq), stop_time_id)| {
                vj_old_idx_to_new_idx
                    .get(&old_vj_id)
                    .map(|new_vj_id| ((*new_vj_id, *seq), stop_time_id.clone()))
            })
            .collect();
        self.stop_time_headsigns = self
            .stop_time_headsigns
            .iter()
            .filter_map(|((old_vj_id, seq), headsign)| {
                vj_old_idx_to_new_idx
                    .get(&old_vj_id)
                    .map(|new_vj_id| ((*new_vj_id, *seq), headsign.clone()))
            })
            .collect();

        self.networks
            .retain(|network| networks_used.contains(&network.id));
        self.trip_properties
            .retain(|trip_property| trip_properties_used.contains(&trip_property.id));
        self.geometries
            .retain(|geometry| geometries_used.contains(&geometry.id));
        self.companies
            .retain(|company| companies_used.contains(&company.id));
        self.equipments
            .retain(|equipment| equipments_used.contains(&equipment.id));
        self.contributors
            .retain(|contributor| contributors_used.contains(&contributor.id));
        self.commercial_modes
            .retain(|commercial_mode| commercial_modes_used.contains(&commercial_mode.id));
        self.physical_modes
            .retain(|physical_mode| physical_modes_used.contains(&physical_mode.id));
        self.transfers.retain(|t| {
            stop_points_used.contains(&t.from_stop_id) && stop_points_used.contains(&t.to_stop_id)
        });

        Ok(())
    }
}

/// The navitia transit model.
#[derive(GetCorresponding)]
pub struct Model {
    collections: Collections,

    // original relations
    networks_to_lines: OneToMany<Network, Line>,
    commercial_modes_to_lines: OneToMany<CommercialMode, Line>,
    lines_to_routes: OneToMany<Line, Route>,
    routes_to_vehicle_journeys: OneToMany<Route, VehicleJourney>,
    physical_modes_to_vehicle_journeys: OneToMany<PhysicalMode, VehicleJourney>,
    stop_areas_to_stop_points: OneToMany<StopArea, StopPoint>,
    contributors_to_datasets: OneToMany<Contributor, Dataset>,
    datasets_to_vehicle_journeys: OneToMany<Dataset, VehicleJourney>,
    companies_to_vehicle_journeys: OneToMany<Company, VehicleJourney>,
    vehicle_journeys_to_stop_points: ManyToMany<VehicleJourney, StopPoint>,
    transfers_to_stop_points: ManyToMany<Transfer, StopPoint>,
    calendars_to_vehicle_journeys: OneToMany<Calendar, VehicleJourney>,

    // shortcuts
    #[get_corresponding(weight = "1.9")]
    routes_to_stop_points: ManyToMany<Route, StopPoint>,
    #[get_corresponding(weight = "1.9")]
    physical_modes_to_stop_points: ManyToMany<PhysicalMode, StopPoint>,
    #[get_corresponding(weight = "1.9")]
    physical_modes_to_routes: ManyToMany<PhysicalMode, Route>,
    #[get_corresponding(weight = "1.9")]
    datasets_to_stop_points: ManyToMany<Dataset, StopPoint>,
    #[get_corresponding(weight = "1.9")]
    datasets_to_routes: ManyToMany<Dataset, Route>,
    #[get_corresponding(weight = "1.9")]
    datasets_to_physical_modes: ManyToMany<Dataset, PhysicalMode>,
}

impl Model {
    /// Constructs a model from the given `Collections`.  Fails in
    /// case of incoherence, as invalid external references.
    ///
    /// # Examples
    ///
    /// ```
    /// # use transit_model::model::*;
    /// # fn run() -> transit_model::Result<()> {
    /// let _: Model = Model::new(Collections::default())?;
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    ///
    /// ```
    /// # use transit_model::model::*;
    /// # use transit_model::collection::Collection;
    /// # use transit_model::objects::Transfer;
    /// let mut collections = Collections::default();
    /// // This transfer is invalid as there is no stop points in collections
    /// collections.transfers = Collection::from(Transfer {
    ///     from_stop_id: "invalid".into(),
    ///     to_stop_id: "also_invalid".into(),
    ///     min_transfer_time: None,
    ///     real_min_transfer_time: None,
    ///     equipment_id: None,
    /// });
    /// assert!(Model::new(collections).is_err());
    /// ```
    pub fn new(c: Collections) -> Result<Self> {
        let forward_vj_to_sp = c
            .vehicle_journeys
            .iter()
            .map(|(idx, vj)| {
                let sps = vj.stop_times.iter().map(|st| st.stop_point_idx).collect();
                (idx, sps)
            })
            .collect();

        let forward_tr_to_sp = c
            .transfers
            .iter()
            .map(|(idx, tr)| {
                let mut stop_points = IdxSet::default();
                stop_points.insert(c.stop_points.get_idx(&tr.from_stop_id).ok_or_else(|| {
                    format_err!("Invalid id: transfer.from_stop_id={:?}", tr.from_stop_id)
                })?);
                stop_points.insert(c.stop_points.get_idx(&tr.to_stop_id).ok_or_else(|| {
                    format_err!("Invalid id: transfer.to_stop_id={:?}", tr.to_stop_id)
                })?);
                Ok((idx, stop_points))
            })
            .collect::<StdResult<BTreeMap<_, _>, Error>>()?;
        let vehicle_journeys_to_stop_points = ManyToMany::from_forward(forward_vj_to_sp);
        let routes_to_vehicle_journeys =
            OneToMany::new(&c.routes, &c.vehicle_journeys, "routes_to_vehicle_journeys")?;
        let physical_modes_to_vehicle_journeys = OneToMany::new(
            &c.physical_modes,
            &c.vehicle_journeys,
            "physical_modes_to_vehicle_journeys",
        )?;
        let datasets_to_vehicle_journeys = OneToMany::new(
            &c.datasets,
            &c.vehicle_journeys,
            "datasets_to_vehicle_journeys",
        )?;
        Ok(Model {
            routes_to_stop_points: ManyToMany::from_relations_chain(
                &routes_to_vehicle_journeys,
                &vehicle_journeys_to_stop_points,
            ),
            physical_modes_to_stop_points: ManyToMany::from_relations_chain(
                &physical_modes_to_vehicle_journeys,
                &vehicle_journeys_to_stop_points,
            ),
            physical_modes_to_routes: ManyToMany::from_relations_sink(
                &physical_modes_to_vehicle_journeys,
                &routes_to_vehicle_journeys,
            ),
            datasets_to_stop_points: ManyToMany::from_relations_chain(
                &datasets_to_vehicle_journeys,
                &vehicle_journeys_to_stop_points,
            ),
            datasets_to_routes: ManyToMany::from_relations_sink(
                &datasets_to_vehicle_journeys,
                &routes_to_vehicle_journeys,
            ),
            datasets_to_physical_modes: ManyToMany::from_relations_sink(
                &datasets_to_vehicle_journeys,
                &physical_modes_to_vehicle_journeys,
            ),
            transfers_to_stop_points: ManyToMany::from_forward(forward_tr_to_sp),
            datasets_to_vehicle_journeys,
            routes_to_vehicle_journeys,
            vehicle_journeys_to_stop_points,
            physical_modes_to_vehicle_journeys,
            networks_to_lines: OneToMany::new(&c.networks, &c.lines, "networks_to_lines")?,
            commercial_modes_to_lines: OneToMany::new(
                &c.commercial_modes,
                &c.lines,
                "commercial_modes_to_lines",
            )?,
            lines_to_routes: OneToMany::new(&c.lines, &c.routes, "lines_to_routes")?,
            stop_areas_to_stop_points: OneToMany::new(
                &c.stop_areas,
                &c.stop_points,
                "stop_areas_to_stop_points",
            )?,
            contributors_to_datasets: OneToMany::new(
                &c.contributors,
                &c.datasets,
                "contributors_to_datasets",
            )?,
            companies_to_vehicle_journeys: OneToMany::new(
                &c.companies,
                &c.vehicle_journeys,
                "companies_to_vehicle_journeys",
            )?,
            calendars_to_vehicle_journeys: OneToMany::new(
                &c.calendars,
                &c.vehicle_journeys,
                "calendars_to_vehicle_journeys",
            )?,
            collections: c,
        })
    }

    /// Consumes collections,
    ///
    /// # Examples
    ///
    /// ```
    /// # use transit_model::model::*;
    /// # use std::collections::HashMap;
    /// # fn run() -> transit_model::Result<()> {
    /// let model: Model = Model::new(Collections::default())?;
    /// let mut collections = model.into_collections();
    ///  collections
    ///    .feed_infos
    ///    .insert("foo".to_string(), "bar".to_string());
    /// let feeds: Vec<(_, _)> = collections.feed_infos.into_iter().collect();
    /// assert_eq!(
    ///    feeds,
    ///    vec![("foo".to_string(), "bar".to_string())]
    /// );
    /// # Ok(())
    /// # }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn into_collections(self) -> Collections {
        self.collections
    }
}
impl ::serde::Serialize for Model {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        self.collections.serialize(serializer)
    }
}
impl<'de> ::serde::Deserialize<'de> for Model {
    fn deserialize<D>(deserializer: D) -> StdResult<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        use serde::de::Error;
        ::serde::Deserialize::deserialize(deserializer)
            .and_then(|o| Model::new(o).map_err(D::Error::custom))
    }
}
impl ops::Deref for Model {
    type Target = Collections;
    fn deref(&self) -> &Self::Target {
        &self.collections
    }
}
