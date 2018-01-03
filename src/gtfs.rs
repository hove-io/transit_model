use std::path;
use csv;
use collection::Collection;
use {Collections, PtObjects};
use objects::{self, CodesT};

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Agency {
    #[serde(rename = "agency_id")]
    id: String,
    #[serde(rename = "agency_name")]
    name: String,
    #[serde(rename = "agency_url")]
    url: String,
    #[serde(rename = "agency_timezone")]
    timezone: String,
    #[serde(rename = "agency_lang", default)]
    lang: String,
    #[serde(rename = "agency_phone", default)]
    phone: String,
    #[serde(rename = "agency_fare_url", default)]
    fare_url: String,
    #[serde(rename = "agency_email", default)]
    email: String,
}
impl From<Agency> for objects::Network {
    fn from(agency: Agency) -> objects::Network {
        objects::Network {
            id: agency.id,
            name: agency.name,
            codes: CodesT::default(),
            timezone: agency.timezone,
        }
    }
}

pub fn read<P: AsRef<path::Path>>(path: P) -> PtObjects {
    let path = path.as_ref();
    let mut collections = Collections::default();
    collections.networks = read_agency(path);
    PtObjects::new(collections)
}

fn read_agency(path: &path::Path) -> Collection<objects::Network> {
    let path = path.join("agency.txt");
    let mut rdr = csv::Reader::from_path(path).unwrap();
    Collection::new(rdr.deserialize()
        .map(Result::unwrap)
        .map(|agency: Agency| objects::Network::from(agency))
        .collect())
}
