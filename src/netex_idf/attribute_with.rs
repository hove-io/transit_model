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

use crate::Result;
use failure::format_err;
use minidom::Element;
use std::{borrow::Cow, str::FromStr};

/// Try to get an attribute of a [Element](minidom::Element) and apply a
/// function on it before returning it.
pub trait AttributeWith {
    /// Try to get an attribute from its name and apply a function on it before
    /// returning it as a [Result](crate::Result)
    fn try_attribute_with<'a, C, F, S>(&'a self, attr_name: &str, f: F) -> Result<S>
    where
        C: Into<Cow<'a, str>>,
        F: Fn(&'a str) -> Result<C>,
        S: FromStr;

    /// Try to get an attribute from its name and apply a function on it before
    /// returning it as a [Result](crate::Result)
    fn attribute_with<'a, C, F, S>(&'a self, attr_name: &str, f: F) -> Option<S>
    where
        C: Into<Cow<'a, str>>,
        F: Fn(&'a str) -> Result<C>,
        S: FromStr,
    {
        self.try_attribute_with(attr_name, f).ok()
    }
}

impl AttributeWith for Element {
    fn try_attribute_with<'a, C, F, S>(&'a self, attr_name: &str, f: F) -> Result<S>
    where
        C: Into<Cow<'a, str>>,
        F: Fn(&'a str) -> Result<C>,
        S: FromStr,
    {
        let raw_id: &'a str = self.attr(attr_name).ok_or_else(|| {
            format_err!(
                "Failed to find attribute '{}' in element '{}'",
                attr_name,
                self.name()
            )
        })?;
        let id: Cow<'a, str> = f(raw_id)?.into();
        id.parse::<S>()
            .map_err(|_| format_err!("Failed to parse and convert '{}'", id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn one_attribute_borrowed() {
        let xml: &'static str = r#"<root id="FR:Network:1234:LOC" />"#;
        let root: Element = xml.parse().unwrap();
        let id: u64 = root
            .try_attribute_with("id", |id| {
                // Fn(&str) -> Result<&str>
                id.split(':').nth(2).ok_or_else(|| format_err!("Boom!"))
            })
            .unwrap();
        assert_eq!(1234, id);
    }

    #[test]
    fn one_attribute_owned() {
        let xml: &'static str = r#"<root id="FR:Network:1234:LOC" />"#;
        let root: Element = xml.parse().unwrap();
        let id: String = root
            .try_attribute_with("id", |id| {
                // Fn(&str) -> Result<String>
                id.split(':')
                    .nth(2)
                    .map(|n| n.to_string())
                    .ok_or_else(|| format_err!("Boom!"))
            })
            .unwrap();
        assert_eq!(String::from("1234"), id);
    }

    #[test]
    #[should_panic(expected = "Failed to find attribute \\'id\\' in element \\'root\\'")]
    fn no_attribute() {
        let xml: &'static str = r#"<root />"#;
        let root: Element = xml.parse().unwrap();
        root.try_attribute_with::<_, _, String>("id", |id| {
            id.split(':').nth(2).ok_or_else(|| format_err!("Boom!"))
        })
        .unwrap();
    }

    #[test]
    #[should_panic(expected = "Failed to parse and convert \\'root\\'")]
    fn no_unique_child() {
        let xml: &'static str = r#"<root id="root:1" />"#;
        let root: Element = xml.parse().unwrap();
        root.try_attribute_with::<_, _, f64>("id", |id| {
            id.split(':').nth(0).ok_or_else(|| format_err!("Boom!"))
        })
        .unwrap();
    }

    #[test]
    #[should_panic(expected = "Cannot extract part \\'2\\' from identifier \\'root:1\\'")]
    fn not_enough_parts() {
        let xml: &'static str = r#"<root id="root:1" />"#;
        let root: Element = xml.parse().unwrap();
        root.try_attribute_with::<_, _, f64>("id", |id| {
            id.split(':')
                .nth(2)
                .ok_or_else(|| format_err!("Cannot extract part '2' from identifier '{}'", id))
        })
        .unwrap();
    }
}
