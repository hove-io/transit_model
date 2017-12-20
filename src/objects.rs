use collection::Id;

#[derive(Serialize, Deserialize, Debug)]
pub struct CommercialMode {
    #[serde(rename = "commercial_mode_id")] pub id: String,
    #[serde(rename = "commercial_mode_name")] pub name: String,
}
impl Id<CommercialMode> for CommercialMode {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PhysicalMode {
    #[serde(rename = "physical_mode_id")] pub id: String,
    #[serde(rename = "physical_mode_name")] pub name: String,
}
impl Id<PhysicalMode> for PhysicalMode {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Network {
    #[serde(rename = "network_id")] pub id: String,
    #[serde(rename = "network_name")] pub name: String,
    #[serde(rename = "network_timezone")] pub timezone: String,
}
impl Id<Network> for Network {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Line {
    #[serde(rename = "line_id")] pub id: String,
    #[serde(rename = "line_name")] pub name: String,
    pub network_id: String,
    pub commercial_mode_id: String,
}
impl Id<Line> for Line {
    fn id(&self) -> &str {
        &self.id
    }
}
impl Id<Network> for Line {
    fn id(&self) -> &str {
        &self.network_id
    }
}
impl Id<CommercialMode> for Line {
    fn id(&self) -> &str {
        &self.commercial_mode_id
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Route {
    #[serde(rename = "route_id")] pub id: String,
    #[serde(rename = "route_name")] pub name: String,
    pub line_id: String,
}
impl Id<Route> for Route {
    fn id(&self) -> &str {
        &self.id
    }
}
impl Id<Line> for Route {
    fn id(&self) -> &str {
        &self.line_id
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VehicleJourney {
    #[serde(rename = "trip_id")] pub id: String,
    pub route_id: String,
    pub physical_mode_id: String,
}
impl Id<VehicleJourney> for VehicleJourney {
    fn id(&self) -> &str {
        &self.id
    }
}
impl Id<Route> for VehicleJourney {
    fn id(&self) -> &str {
        &self.route_id
    }
}
impl Id<PhysicalMode> for VehicleJourney {
    fn id(&self) -> &str {
        &self.physical_mode_id
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Coord {
    pub lon: f64,
    pub lat: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StopArea {
    pub id: String,
    pub name: String,
    pub visible: bool,
    pub coord: Coord,
}
impl Id<StopArea> for StopArea {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StopPoint {
    pub id: String,
    pub name: String,
    pub visible: bool,
    pub coord: Coord,
    pub stop_area_id: String,
}
impl Id<StopPoint> for StopPoint {
    fn id(&self) -> &str {
        &self.id
    }
}
impl Id<StopArea> for StopPoint {
    fn id(&self) -> &str {
        &self.stop_area_id
    }
}
