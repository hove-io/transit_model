// Copyright (C) 2017 Kisio Digital and/or its affiliates.
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by the
// Free Software Foundation, version 3.

// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.

// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>

//! Module to handle Netex France profile

mod calendars;
use calendars::CalendarExporter;
mod companies;
use companies::CompanyExporter;
mod exporter;
pub use exporter::Exporter;
mod lines;
use lines::LineExporter;
use lines::LineModes;
mod modes;
use modes::NetexMode;
mod networks;
use networks::NetworkExporter;
mod offer;
use offer::OfferExporter;
mod route_points;
use route_points::build_route_points;
mod stops;
use stops::StopExporter;
mod transfers;
use transfers::TransferExporter;

const CORE_NS: &str = "http://www.govtalk.gov.uk/core";
const GML_NS: &str = "http://www.opengis.net/gml/3.2";
const IFOPT_NS: &str = "http://www.ifopt.org.uk/ifopt";
const NETEX_NS: &str = "http://www.netex.org.uk/netex";
const SIRI_NS: &str = "http://www.siri.org.uk/siri";
const XLINK_NS: &str = "http://www.w3.org/1999/xlink";
const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";
