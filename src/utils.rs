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

use crate::{
    objects::Date,
    read_utils::{read_objects, FileHandler},
};
use chrono::NaiveDate;
use failure::{format_err, ResultExt};
use log::{debug, error, info};
use rust_decimal::Decimal;
use std::fs;
use std::io::{Read, Write};
use std::path;
use typed_index_collection::{Collection, CollectionWithId, Id};
use walkdir::WalkDir;
use wkt::{self, conversion::try_into_geometry, ToWkt};

pub fn zip_to<P, R>(source_path: P, zip_file: R) -> crate::Result<()>
where
    P: AsRef<path::Path>,
    R: AsRef<path::Path>,
{
    let source_path = source_path.as_ref();
    let file = fs::File::create(zip_file.as_ref())?;
    let mut zip = zip::ZipWriter::new(file);
    let options =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut buffer = Vec::new();
    for entry in WalkDir::new(source_path) {
        let path = entry?.path().to_owned();
        if path.is_file() {
            let name = path.strip_prefix(path::Path::new(source_path))?.to_owned();
            if let Some(name) = name.to_str() {
                debug!("adding {:?} as {:?} ...", path, name);
                zip.start_file(name, options)?;
                let mut f = fs::File::open(path)?;

                f.read_to_end(&mut buffer)?;
                zip.write_all(&*buffer)?;
                buffer.clear();
            }
        }
    }
    zip.finish()?;
    Ok(())
}

pub fn de_from_u8<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::{
        de::{Error, Unexpected::Other},
        Deserialize,
    };
    let i = <u8 as Deserialize<'de>>::deserialize(deserializer)?;
    if i == 0 || i == 1 {
        Ok(i != 0)
    } else {
        Err(D::Error::invalid_value(
            Other(&format!("{} non boolean value", i)),
            &"boolean",
        ))
    }
}

pub fn de_from_u8_with_true_default<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    match u8::deserialize(deserializer) {
        Ok(val) => Ok(val != 0),
        Err(_) => Ok(true),
    }
}

// The signature of the function must pass by reference for 'serde' to be able to use the function
pub fn ser_from_bool<S>(v: &bool, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_u8(*v as u8)
}

pub fn de_from_date_string<'de, D>(deserializer: D) -> Result<Date, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let s = String::deserialize(deserializer)?;

    NaiveDate::parse_from_str(&s, "%Y%m%d").map_err(serde::de::Error::custom)
}

// The signature of the function must pass by reference for 'serde' to be able to use the function
pub fn ser_from_naive_date<S>(date: &Date, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let s = format!("{}", date.format("%Y%m%d"));
    serializer.serialize_str(&s)
}

pub fn de_with_empty_default<'de, T: Default, D>(de: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    use serde::Deserialize;
    Option::<T>::deserialize(de).map(|opt| opt.unwrap_or_else(Default::default))
}

pub fn ser_option_u32_with_default<S>(value: &Option<u32>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_u32(value.unwrap_or_default())
}

pub fn de_with_invalid_option<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    Option<T>: serde::Deserialize<'de>,
{
    use serde::Deserialize;
    Option::<T>::deserialize(de).or_else(|e| {
        error!("{}", e);
        Ok(None)
    })
}

pub fn de_without_slashes<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    de_option_without_slashes(deserializer).map(|opt| opt.unwrap_or_else(Default::default))
}

pub fn de_option_without_slashes<'de, D>(de: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let option = Option::<String>::deserialize(de)?;
    Ok(option.map(|s| s.replace("/", "")))
}

pub fn de_with_empty_or_invalid_default<'de, D, T>(de: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    Option<T>: serde::Deserialize<'de>,
    T: Default,
{
    de_with_invalid_option(de).map(|opt| opt.unwrap_or_else(Default::default))
}

pub fn de_wkt<'de, D>(deserializer: D) -> Result<geo::Geometry<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let s = String::deserialize(deserializer)?;
    let wkt = wkt::Wkt::from_str(&s).map_err(serde::de::Error::custom)?;
    try_into_geometry(&wkt.items[0]).map_err(serde::de::Error::custom)
}

pub fn de_positive_decimal<'de, D>(deserializer: D) -> Result<Decimal, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::{
        de::{Error, Unexpected::Other},
        Deserialize,
    };
    let number = <Decimal as Deserialize<'de>>::deserialize(deserializer)?;
    if number.is_sign_positive() {
        Ok(number)
    } else {
        Err(D::Error::invalid_value(
            Other("strictly negative float number"),
            &"positive float number",
        ))
    }
}

pub fn de_option_positive_decimal<'de, D>(deserializer: D) -> Result<Option<Decimal>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::{
        de::{Error, Unexpected::Other},
        Deserialize,
    };
    let option = <Option<Decimal> as Deserialize<'de>>::deserialize(deserializer)?;
    match option {
        Some(number) if number.is_sign_positive() => Ok(option),
        None => Ok(None),
        _ => Err(D::Error::invalid_value(
            Other("strictly negative float number"),
            &"positive float number",
        )),
    }
}

pub fn de_option_positive_float<'de, D>(deserializer: D) -> Result<Option<f32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::{
        de::{Error, Unexpected::Other},
        Deserialize,
    };
    let option = <Option<f32> as Deserialize<'de>>::deserialize(deserializer)?;
    match option {
        Some(number) if number.is_sign_positive() => Ok(option),
        None => Ok(None),
        _ => Err(D::Error::invalid_value(
            Other("strictly negative float number"),
            &"positive float number",
        )),
    }
}

pub fn de_option_non_null_integer<'de, D>(deserializer: D) -> Result<Option<i16>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::{
        de::{Error, Unexpected::Other},
        Deserialize,
    };
    let option = <Option<i16> as Deserialize<'de>>::deserialize(deserializer)?;
    match option {
        Some(number) if number != 0 => Ok(option),
        None => Ok(None),
        _ => Err(D::Error::invalid_value(Other("0"), &"non null number")),
    }
}

pub fn de_currency_code<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::{
        de::{Error, Unexpected::Other},
        Deserialize,
    };
    let string = String::deserialize(deserializer)?;
    let currency_code = iso4217::alpha3(&string).ok_or_else(|| {
        D::Error::invalid_value(
            Other("unrecognized currency code (ISO-4217)"),
            &"3-letters currency code (ISO-4217)",
        )
    })?;
    Ok(String::from(currency_code.alpha3))
}

pub fn ser_currency_code<S>(currency_code: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::Error;
    let currency_code = iso4217::alpha3(currency_code)
        .ok_or_else(|| S::Error::custom("The String is not a valid currency code (ISO-4217)"))?;
    serializer.serialize_str(&currency_code.alpha3.to_string())
}

pub fn ser_geometry<S>(geometry: &geo::Geometry<f64>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let wkt = geometry.to_wkt();
    serializer.serialize_str(&format!("{}", wkt.items[0]))
}

pub fn de_option_empty_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Deserialize;
    <Option<String> as Deserialize<'de>>::deserialize(deserializer)
        .map(|option| option.filter(|s| !s.trim().is_empty()))
}

pub(crate) fn make_opt_collection_with_id<T, H>(
    file_handler: &mut H,
    file: &str,
) -> crate::Result<CollectionWithId<T>>
where
    for<'de> T: Id<T> + serde::Deserialize<'de>,
    for<'a> &'a mut H: FileHandler,
{
    let vec = read_objects::<_, T>(file_handler, file, false)?;
    CollectionWithId::new(vec).map_err(|e| format_err!("{}", e))
}

pub(crate) fn make_collection_with_id<T, H>(
    file_handler: &mut H,
    file: &str,
) -> crate::Result<CollectionWithId<T>>
where
    for<'de> T: Id<T> + serde::Deserialize<'de>,
    for<'a> &'a mut H: FileHandler,
{
    let vec = read_objects::<_, T>(file_handler, file, true)?;
    CollectionWithId::new(vec).map_err(|e| format_err!("{}", e))
}

pub(crate) fn make_opt_collection<T, H>(
    file_handler: &mut H,
    file: &str,
) -> crate::Result<Collection<T>>
where
    for<'de> T: serde::Deserialize<'de>,
    for<'a> &'a mut H: FileHandler,
{
    let vec = read_objects::<_, T>(file_handler, file, false)?;
    Ok(Collection::new(vec))
}

pub(crate) fn make_collection<T, H>(
    file_handler: &mut H,
    file: &str,
) -> crate::Result<Collection<T>>
where
    for<'de> T: serde::Deserialize<'de>,
    for<'a> &'a mut H: FileHandler,
{
    let vec = read_objects::<_, T>(file_handler, file, true)?;
    Ok(Collection::new(vec))
}

pub fn write_collection_with_id<T>(
    path: &path::Path,
    file: &str,
    collection: &CollectionWithId<T>,
) -> crate::Result<()>
where
    T: Id<T> + serde::Serialize,
{
    if collection.is_empty() {
        return Ok(());
    }
    info!("Writing {}", file);
    let path = path.join(file);
    let mut wtr =
        csv::Writer::from_path(&path).with_context(|_| format!("Error reading {:?}", path))?;
    for obj in collection.values() {
        wtr.serialize(obj)
            .with_context(|_| format!("Error reading {:?}", path))?;
    }
    wtr.flush()
        .with_context(|_| format!("Error reading {:?}", path))?;

    Ok(())
}

pub fn write_collection<T>(
    path: &path::Path,
    file: &str,
    collection: &Collection<T>,
) -> crate::Result<()>
where
    T: serde::Serialize,
{
    if collection.is_empty() {
        return Ok(());
    }
    info!("Writing {}", file);
    let path = path.join(file);
    let mut wtr =
        csv::Writer::from_path(&path).with_context(|_| format!("Error reading {:?}", path))?;
    for obj in collection.values() {
        wtr.serialize(obj)
            .with_context(|_| format!("Error reading {:?}", path))?;
    }
    wtr.flush()
        .with_context(|_| format!("Error reading {:?}", path))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    mod serde_option_string {
        use super::*;
        use pretty_assertions::assert_eq;
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize, Deserialize)]
        struct WithOption {
            #[serde(default, deserialize_with = "de_option_empty_string")]
            name: Option<String>,
        }

        #[test]
        fn with_string() {
            let json = r#"{"name": "baz"}"#;
            let object: WithOption = serde_json::from_str(&json).unwrap();
            assert_eq!(object.name.unwrap(), "baz");
        }

        #[test]
        fn with_empty_string() {
            let json = r#"{"name": ""}"#;
            let object: WithOption = serde_json::from_str(&json).unwrap();
            assert_eq!(object.name, None);
        }

        #[test]
        fn without_field() {
            let json = r#"{}"#;
            let object: WithOption = serde_json::from_str(&json).unwrap();
            assert_eq!(object.name, None);
        }
    }

    mod serde_currency {
        use super::*;
        use pretty_assertions::assert_eq;
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize, Deserialize)]
        struct CurrencyCodeWrapper {
            #[serde(
                serialize_with = "ser_currency_code",
                deserialize_with = "de_currency_code"
            )]
            pub currency_code: String,
        }

        #[test]
        fn test_serde_valid_currency_code() {
            let wrapper = CurrencyCodeWrapper {
                currency_code: "EUR".to_string(),
            };
            let json = serde_json::to_string(&wrapper).unwrap();
            let wrapper: CurrencyCodeWrapper = serde_json::from_str(&json).unwrap();

            assert_eq!("EUR", wrapper.currency_code);
        }

        #[test]
        fn test_de_invalid_currency_code() {
            let result: Result<CurrencyCodeWrapper, _> =
                serde_json::from_str("{\"currency_code\":\"XXX\"}");
            let err_msg = result.unwrap_err().to_string();
            assert_eq!( "invalid value: unrecognized currency code (ISO-4217), expected 3-letters currency code (ISO-4217) at line 1 column 23",err_msg);
        }

        #[test]
        fn test_ser_invalid_currency_code() {
            let wrapper = CurrencyCodeWrapper {
                currency_code: "XXX".to_string(),
            };
            let result = serde_json::to_string(&wrapper);
            let err_msg = result.unwrap_err().to_string();
            assert_eq!(
                "The String is not a valid currency code (ISO-4217)",
                err_msg
            );
        }
    }

    mod deserialize_decimal {
        use super::*;
        use pretty_assertions::assert_eq;
        use rust_decimal_macros::dec;
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize, Deserialize)]
        struct DecimalWrapper {
            #[serde(deserialize_with = "de_positive_decimal")]
            pub value: Decimal,
        }

        #[test]
        fn positive_decimal() {
            let result: Result<DecimalWrapper, _> = serde_json::from_str("{\"value\":\"4.2\"}");
            let wrapper = result.unwrap();
            assert_eq!(dec!(4.2), wrapper.value);
        }

        #[test]
        fn negative_decimal() {
            let result: Result<DecimalWrapper, _> = serde_json::from_str("{\"value\":\"-4.2\"}");
            let err_msg = result.unwrap_err().to_string();
            assert_eq!( "invalid value: strictly negative float number, expected positive float number at line 1 column 16",err_msg);
        }

        #[test]
        fn not_a_number() {
            let result: Result<DecimalWrapper, _> =
                serde_json::from_str("{\"value\":\"NotANumber\"}");
            let err_msg = result.unwrap_err().to_string();
            assert_eq!(
                "invalid value: string \"NotANumber\", expected a Decimal type representing a fixed-point number at line 1 column 21",
                err_msg
            );
        }
    }
}
