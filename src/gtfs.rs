use std::path;
use csv;
use collection::CollectionWithId;
use {Collections, PtObjects};
use objects::{self, CodesT};

fn default_agency_id() -> String {
    "default_agency_id".to_string()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Agency {
    #[serde(rename = "agency_id")]
    id: Option<String>,
    #[serde(rename = "agency_name")]
    name: String,
    #[serde(rename = "agency_url")]
    url: String,
    #[serde(rename = "agency_timezone")]
    timezone: Option<String>,
    #[serde(rename = "agency_lang")]
    lang: Option<String>,
    #[serde(rename = "agency_phone")]
    phone: Option<String>,
    #[serde(rename = "agency_email")]
    email: Option<String>,
}
impl From<Agency> for objects::Network {
    fn from(agency: Agency) -> objects::Network {
        objects::Network {
            id: agency.id.unwrap_or_else(default_agency_id),
            name: agency.name,
            codes: CodesT::default(),
            timezone: agency.timezone,
            url: Some(agency.url),
            lang: agency.lang,
            phone: agency.phone,
            address: None,
            sort_order: None,
        }
    }
}
impl From<Agency> for objects::Company {
    fn from(agency: Agency) -> objects::Company {
        objects::Company {
            id: agency.id.unwrap_or_else(default_agency_id),
            name: agency.name,
            address: None,
            url: Some(agency.url),
            mail: agency.email,
            phone: agency.phone,
        }
    }
}

pub fn read<P: AsRef<path::Path>>(path: P) -> PtObjects {
    let path = path.as_ref();
    let mut collections = Collections::default();
    let (networks, companies) = read_agency(path);
    collections.networks = networks;
    collections.companies = companies;
    PtObjects::new(collections)
}

pub fn read_agency<P: AsRef<path::Path>>(path: P)
        -> (CollectionWithId<objects::Network>, CollectionWithId<objects::Company>) {
    let path = path.as_ref().join("agency.txt");
    let mut rdr = csv::Reader::from_path(path).unwrap();
    let gtfs_agencies : Vec<Agency> = rdr.deserialize().map(Result::unwrap).collect();
    let networks = gtfs_agencies.iter().cloned()
                    .map(|agency| objects::Network::from(agency))
                    .collect();
    let networks = CollectionWithId::new(networks);
    let companies = gtfs_agencies.into_iter()
                    .map(|agency| objects::Company::from(agency))
                    .collect();
    let companies = CollectionWithId::new(companies);
    (networks, companies)
}
