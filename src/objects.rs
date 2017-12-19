use collection::Id;

#[derive(Derivative, Serialize, Deserialize, Debug)]
pub struct CommercialMode {
    #[serde(rename = "commercial_mode_id")] pub id: String,
    #[serde(rename = "commercial_mode_name")] pub name: String,
}
impl Id<CommercialMode> for CommercialMode {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Derivative, Serialize, Deserialize, Debug)]
pub struct Line {
    #[serde(rename = "line_id")] pub id: String,
    #[serde(rename = "line_name")] pub name: String,
    pub commercial_mode_id: String,
}
impl Id<Line> for Line {
    fn id(&self) -> &str {
        &self.id
    }
}
impl Id<CommercialMode> for Line {
    fn id(&self) -> &str {
        &self.commercial_mode_id
    }
}

#[derive(Derivative, Serialize, Deserialize, Debug)]
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
