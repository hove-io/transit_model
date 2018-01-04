use std::path;
use csv;
use collection::Collection;
use {Collections, PtObjects};
use objects::{self, CodesT};

fn default_agency_id() -> String {
    "1".to_string()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Agency {
    #[serde(rename = "agency_id", default = "default_agency_id")] id: String,
    #[serde(rename = "agency_name")] name: String,
    #[serde(rename = "agency_url")] url: String,
    #[serde(rename = "agency_timezone")] timezone: Option<String>,
    #[serde(rename = "agency_lang")] lang: Option<String>,
    #[serde(rename = "agency_phone")] phone: Option<String>,
    #[serde(rename = "agency_email")] email: Option<String>,
}
impl From<Agency> for objects::Network {
    fn from(agency: Agency) -> objects::Network {
        objects::Network {
            id: agency.id,
            name: agency.name,
            codes: CodesT::default(),
            timezone: agency.timezone,
            url: Some(agency.url),
            lang: agency.lang,
            phone: agency.phone,
            sort_order: None,
        }
    }
}

pub fn read<P: AsRef<path::Path>>(path: P) -> PtObjects {
    let path = path.as_ref();
    let mut collections = Collections::default();
    collections.networks = read_agency(path);
    PtObjects::new(collections)
}

pub fn read_agency<P: AsRef<path::Path>>(path: P) -> Collection<objects::Network> {
    let path = path.as_ref().join("agency.txt");
    let mut rdr = csv::Reader::from_path(path).unwrap();
    Collection::new(
        rdr.deserialize()
            .map(Result::unwrap)
            .map(|agency: Agency| objects::Network::from(agency))
            .collect(),
    )
}
