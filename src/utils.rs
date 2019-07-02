// Copyright 2017-2019 Kisio Digital and/or its affiliates.
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

use crate::collection::{Collection, CollectionWithId, Id};
use crate::objects::{AddPrefix, Date};
use chrono::NaiveDate;
use csv;
use failure::ResultExt;
use geo_types;
use log::{debug, error, info};
use rust_decimal::Decimal;
use serde_derive::Serialize;
use std::fs;
use std::io::{Read, Write};
use std::path;
use walkdir::WalkDir;
use wkt::{self, conversion::try_into_geometry, ToWkt};
use zip;

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
    use serde::Deserialize;
    let i = u8::deserialize(deserializer)?;
    Ok(i != 0)
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

pub fn de_location_trim_with_default<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let s = String::deserialize(deserializer)?;
    Ok(s.parse::<f64>().unwrap_or_else(|e| {
        error!("{}", e);
        0.00
    }))
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

pub fn de_wkt<'de, D>(deserializer: D) -> Result<geo_types::Geometry<f64>, D::Error>
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

pub fn ser_geometry<S>(
    geometry: &geo_types::Geometry<f64>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let wkt = geometry.to_wkt();
    serializer.serialize_str(&format!("{}", wkt.items[0]))
}

macro_rules! ctx_from_path {
    ($path:expr) => {
        |_| format!("Error reading {:?}", $path)
    };
}

pub fn make_opt_collection_with_id<T>(
    path: &path::Path,
    file: &str,
) -> crate::Result<CollectionWithId<T>>
where
    T: Id<T>,
    for<'de> T: serde::Deserialize<'de>,
{
    if !path.join(file).exists() {
        info!("Skipping {}", file);
        Ok(CollectionWithId::default())
    } else {
        make_collection_with_id(path, file)
    }
}

pub fn make_collection_with_id<T>(
    path: &path::Path,
    file: &str,
) -> crate::Result<CollectionWithId<T>>
where
    T: Id<T>,
    for<'de> T: serde::Deserialize<'de>,
{
    info!("Reading {}", file);
    let path = path.join(file);
    let mut rdr = csv::Reader::from_path(&path).with_context(ctx_from_path!(path))?;
    let vec = rdr
        .deserialize()
        .collect::<Result<_, _>>()
        .with_context(ctx_from_path!(path))?;
    CollectionWithId::new(vec)
}

pub fn make_opt_collection<T>(path: &path::Path, file: &str) -> crate::Result<Collection<T>>
where
    for<'de> T: serde::Deserialize<'de>,
{
    if !path.join(file).exists() {
        info!("Skipping {}", file);
        Ok(Collection::default())
    } else {
        make_collection(path, file)
    }
}

pub fn make_collection<T>(path: &path::Path, file: &str) -> crate::Result<Collection<T>>
where
    for<'de> T: serde::Deserialize<'de>,
{
    info!("Reading {}", file);
    let path = path.join(file);
    let mut rdr = csv::Reader::from_path(&path).with_context(ctx_from_path!(path))?;
    let vec = rdr
        .deserialize()
        .collect::<Result<_, _>>()
        .with_context(ctx_from_path!(path))?;
    Ok(Collection::new(vec))
}

pub fn add_prefix_to_collection_with_id<T>(
    collection: &mut CollectionWithId<T>,
    prefix: &str,
) -> crate::Result<()>
where
    T: AddPrefix + Id<T>,
{
    let mut objects = collection.take();
    for obj in &mut objects {
        obj.add_prefix(prefix);
    }

    *collection = CollectionWithId::new(objects)?;

    Ok(())
}

pub fn add_prefix_to_collection<T>(collection: &mut Collection<T>, prefix: &str)
where
    T: AddPrefix,
{
    for obj in &mut collection.values_mut() {
        obj.add_prefix(prefix);
    }
}

macro_rules! skip_fail {
    ($res:expr) => {{
        use log::warn;
        match $res {
            Ok(val) => val,
            Err(e) => {
                warn!("{}", e);
                continue;
            }
        }
    }};
}

#[derive(Debug, Serialize)]
pub enum ReportType {
    // merge stop areas types
    OnlyOneStopArea,
    AmbiguousPriorities,
    NothingToMerge,
    MissingToMerge,
    NoMasterPossible,
    MasterReplaced,
    // transfers types
    TransferIntraIgnored,
    TransferInterIgnored,
    TransferOnNonExistentStop,
    TransferOnUnreferencedStop,
    TransferAlreadyDeclared,
    // apply-rules types
    ObjectNotFound,
    InvalidFile,
    UnknownPropertyName,
    MultipleValue,
    OldPropertyValueDoesNotMatch,
    GeometryNotValid,
    NonConvertibleString,
}

#[derive(Debug, Serialize)]
struct ReportRow {
    category: ReportType,
    message: String,
}

#[derive(Debug, Default, Serialize)]
pub struct Report {
    errors: Vec<ReportRow>,
    warnings: Vec<ReportRow>,
}

impl Report {
    pub fn add_warning(&mut self, warning: String, warning_type: ReportType) {
        self.warnings.push(ReportRow {
            category: warning_type,
            message: warning,
        });
    }
    pub fn add_error(&mut self, error: String, error_type: ReportType) {
        self.errors.push(ReportRow {
            category: error_type,
            message: error,
        });
    }
}

#[cfg(test)]
mod tests {
    mod serde_currency {
        use super::super::*;
        use pretty_assertions::assert_eq;
        use serde_derive::{Deserialize, Serialize};

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

            assert_eq!(wrapper.currency_code, "EUR");
        }

        #[test]
        fn test_de_invalid_currency_code() {
            let result: Result<CurrencyCodeWrapper, _> =
                serde_json::from_str("{\"currency_code\":\"XXX\"}");
            let err_msg = result.unwrap_err().to_string();
            assert_eq!(err_msg, "invalid value: unrecognized currency code (ISO-4217), expected 3-letters currency code (ISO-4217) at line 1 column 23");
        }

        #[test]
        fn test_ser_invalid_currency_code() {
            let wrapper = CurrencyCodeWrapper {
                currency_code: "XXX".to_string(),
            };
            let result = serde_json::to_string(&wrapper);
            let err_msg = result.unwrap_err().to_string();
            assert_eq!(
                err_msg,
                "The String is not a valid currency code (ISO-4217)"
            );
        }
    }

    mod deserialize_decimal {
        use super::super::*;
        use pretty_assertions::assert_eq;
        use rust_decimal_macros::dec;
        use serde_derive::{Deserialize, Serialize};

        #[derive(Debug, Serialize, Deserialize)]
        struct DecimalWrapper {
            #[serde(deserialize_with = "de_positive_decimal")]
            pub value: Decimal,
        }

        #[test]
        fn positive_decimal() {
            let result: Result<DecimalWrapper, _> = serde_json::from_str("{\"value\":\"4.2\"}");
            let wrapper = result.unwrap();
            assert_eq!(wrapper.value, dec!(4.2));
        }

        #[test]
        fn negative_decimal() {
            let result: Result<DecimalWrapper, _> = serde_json::from_str("{\"value\":\"-4.2\"}");
            let err_msg = result.unwrap_err().to_string();
            assert_eq!(err_msg, "invalid value: strictly negative float number, expected positive float number at line 1 column 16");
        }

        #[test]
        fn not_a_number() {
            let result: Result<DecimalWrapper, _> =
                serde_json::from_str("{\"value\":\"NotANumber\"}");
            let err_msg = result.unwrap_err().to_string();
            assert_eq!(
                err_msg,
                "invalid value: string \"NotANumber\", expected a Decimal type representing a fixed-point number at line 1 column 21"
            );
        }
    }
}
