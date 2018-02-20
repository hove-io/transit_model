use Result;
use failure::ResultExt;
use csv;
use std::path;
use collection::*;
use objects::{Calendar, Date, ExceptionType};
use utils::{de_from_date_string, ser_from_naive_date};
use Collections;

#[derive(Serialize, Deserialize, Debug)]
pub struct CalendarDate {
    pub service_id: String,
    #[serde(deserialize_with = "de_from_date_string", serialize_with = "ser_from_naive_date")]
    pub date: Date,
    pub exception_type: ExceptionType,
}

fn insert_calendar_date(collection: &mut CollectionWithId<Calendar>, calendar_date: CalendarDate) {
    let idx = match collection.get_idx(&calendar_date.service_id) {
        Some(idx) => idx,
        None => {
            error!(
                "calendar_dates.txt: service_id={} not found",
                calendar_date.service_id
            );
            return;
        }
    };
    collection
        .index_mut(idx)
        .calendar_dates
        .push((calendar_date.date, calendar_date.exception_type))
}

pub fn manage_calendars(collections: &mut Collections, path: &path::Path) -> Result<()> {
    collections.calendars = make_collection_with_id(path, "calendar.txt")?;

    info!("Reading calendar_dates.txt");
    let path = path.join("calendar_dates.txt");
    if let Ok(mut rdr) = csv::Reader::from_path(&path) {
        for calendar_date in rdr.deserialize() {
            let calendar_date = calendar_date.with_context(ctx_from_path!(path))?;
            let calendar_date: CalendarDate = calendar_date;
            insert_calendar_date(&mut collections.calendars, calendar_date);
        }
    }
    Ok(())
}
