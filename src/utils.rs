use chrono::NaiveDate;
use objects::Date;

pub fn de_from_u8<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: ::serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let i = u8::deserialize(deserializer)?;
    Ok(i == 0)
}

pub fn ser_from_bool<S>(v: &bool, serializer: S) -> Result<S::Ok, S::Error>
where
    S: ::serde::Serializer,
{
    serializer.serialize_u8(*v as u8)
}

pub fn de_from_date_string<'de, D>(deserializer: D) -> Result<Date, D::Error>
where
    D: ::serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let s = String::deserialize(deserializer)?;

    NaiveDate::parse_from_str(&s, "%Y%m%d").map_err(::serde::de::Error::custom)
}

pub fn ser_from_naive_date<S>(date: &Date, serializer: S) -> Result<S::Ok, S::Error>
where
    S: ::serde::Serializer,
{
    let s = format!("{}", date.format("%Y%m%d"));
    serializer.serialize_str(&s)
}

pub fn de_with_empty_default<'de, T: Default, D>(deserializer: D) -> Result<T, D::Error>
where
    D: ::serde::Deserializer<'de>,
    for<'d> T: ::serde::Deserialize<'d>,
{
    use serde::Deserialize;
    let s = String::deserialize(deserializer)?;
    if s.is_empty() {
        return Ok(Default::default());
    }

    ::serde_json::from_value(::serde_json::value::Value::String(s))
        .map_err(::serde::de::Error::custom)
}
