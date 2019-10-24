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
use std::str::FromStr;

/// Try to get an attribute of a [Element](minidom::Element) and returns a
/// [Result](crate::Result) instead of an [Option](Option)
pub trait TryAttribute {
    /// Try to get an attribute from its name and return a [Result](crate::Result)
    fn try_attribute<F>(&self, attr_name: &str) -> Result<F>
    where
        F: FromStr;

    /// Get an attribute from its name if present and return a [Option](std::option::Option)
    fn attribute<F>(&self, attr_name: &str) -> Option<F>
    where
        F: FromStr,
    {
        self.try_attribute(attr_name).ok()
    }
}

impl TryAttribute for Element {
    fn try_attribute<F>(&self, attr_name: &str) -> Result<F>
    where
        F: FromStr,
    {
        let value = self.attr(attr_name).ok_or_else(|| {
            format_err!("Failed to find attribute 'id' in element '{}'", self.name())
        })?;
        value
            .parse()
            .map_err(|_| format_err!("Failed to parse and convert '{}'", value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn one_attribute() {
        let xml: &'static str = r#"<root id="42" />"#;
        let root: Element = xml.parse().unwrap();
        let id: u64 = root.try_attribute("id").unwrap();
        assert_eq!(42, id);
    }

    #[test]
    #[should_panic(expected = "Failed to find attribute \\'id\\' in element \\'root\\'")]
    fn no_attribute() {
        let xml: &'static str = r#"<root />"#;
        let root: Element = xml.parse().unwrap();
        let _id: String = root.try_attribute("id").unwrap();
    }

    #[test]
    #[should_panic(expected = "Failed to parse and convert \\'root:1\\'")]
    fn no_unique_child() {
        let xml: &'static str = r#"<root id="root:1" />"#;
        let root: Element = xml.parse().unwrap();
        let _id: f64 = root.try_attribute("id").unwrap();
    }
}
