// Copyright 2017 Kisio Digital and/or its affiliates.
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

use crate::Result;
use failure::bail;
use minidom::Element;

/// Try to get the only child of an [Element](minidom::Element) and returns a
/// [Result](crate::Result) instead of an [Option](Option). Note also that
/// [get_child()](minidom::Element::get_child) will return the first child if
/// multiple childrens are found but TryOnlyChild will succeed only if one child
/// is present (if none or more than two childrens are found, TryOnlyChild will
/// fail)
pub trait TryOnlyChild {
    /// Try to get an unique child from its name and return a [Result](crate::Result)
    /// A filter can be apply on the kind of children you want to select
    fn try_only_child_with_filter<'a, P>(&'a self, child_name: &str, filter: P) -> Result<&'a Self>
    where
        P: Fn(&'a Self) -> bool;

    /// Try to get an unique child from its name and return a [Result](crate::Result)
    fn try_only_child<'a>(&'a self, child_name: &str) -> Result<&'a Self> {
        self.try_only_child_with_filter(child_name, |_| true)
    }
}

impl TryOnlyChild for Element {
    fn try_only_child_with_filter<'a, P>(&'a self, child_name: &str, filter: P) -> Result<&'a Self>
    where
        P: Fn(&'a Self) -> bool,
    {
        let mut child_iterator = self
            .children()
            .filter(|child| child.name() == child_name)
            .filter(|child| filter(*child));
        if let Some(child) = child_iterator.next() {
            if child_iterator.next().is_none() {
                Ok(child)
            } else {
                bail!(
                    "Failed to find a unique child '{}' in element '{}'",
                    child_name,
                    self.name()
                );
            }
        } else {
            bail!(
                "Failed to find a child '{}' in element '{}'",
                child_name,
                self.name()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn only_one_child() {
        let xml: &'static str = r#"<root>
                <child type="ugly" />
                <child />
            </root>"#;
        let root: Element = xml.parse().unwrap();
        let child = root
            .try_only_child_with_filter("child", |e| {
                e.attr("type").map(|id| id == "ugly").unwrap_or(false)
            })
            .unwrap();
        assert_eq!("child", child.name());
    }

    #[test]
    #[should_panic(expected = "Failed to find a child \\'child\\' in element \\'root\\'")]
    fn no_child() {
        let xml: &'static str = r#"<root />"#;
        let root: Element = xml.parse().unwrap();
        root.try_only_child_with_filter("child", |_| true).unwrap();
    }

    #[test]
    #[should_panic(expected = "Failed to find a unique child \\'child\\' in element \\'root\\'")]
    fn no_unique_child() {
        let xml: &'static str = r#"<root>
                <child type="nice"/>
                <child type="nice"/>
            </root>"#;
        let root: Element = xml.parse().unwrap();
        root.try_only_child_with_filter("child", |e| {
            e.attr("type").map(|id| id == "nice").unwrap_or(false)
        })
        .unwrap();
    }
}
