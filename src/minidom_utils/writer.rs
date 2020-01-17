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

//! Writer to serialize an `Element` (its name, attributes and children)

use crate::Result;
use minidom::{Element, Node};
use quick_xml::{
    events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event},
    Writer,
};
use std::io::Write;
const XML_VERSION: &str = "1.0";
const ENCODING: &str = "UTF-8";

pub struct ElementWriter {
    element: Element,
    indent: bool,
}

// Publicly exposed methods
impl ElementWriter {
    pub fn new(element: Element, indent: bool) -> Self {
        ElementWriter { element, indent }
    }

    pub fn write<W>(&self, write: &mut W) -> Result<()>
    where
        W: Write,
    {
        // 9 is ASCII code for Tabulation
        let mut writer = if self.indent {
            Writer::new_with_indent(write, 9, 1)
        } else {
            Writer::new(write)
        };
        let decl_bytes = BytesDecl::new(XML_VERSION.as_bytes(), Some(ENCODING.as_bytes()), None);
        writer.write_event(Event::Decl(decl_bytes))?;
        self.write_element(&mut writer, &self.element)
    }
}

// Internal methods
impl ElementWriter {
    fn write_element<W>(&self, writer: &mut Writer<W>, element: &Element) -> Result<()>
    where
        W: Write,
    {
        let name = if let Some(prefix) = element.prefix() {
            format!("{}:{}", prefix, element.name())
        } else {
            element.name().to_string()
        };
        let mut start_bytes = BytesStart::borrowed(name.as_bytes(), name.len());
        start_bytes.extend_attributes(element.attrs());
        writer.write_event(Event::Start(start_bytes))?;

        for node in element.nodes() {
            match node {
                Node::Element(e) => {
                    self.write_element(writer, e)?;
                }
                Node::Text(t) => {
                    let text_bytes = BytesText::from_plain_str(t.as_str());
                    writer.write_event(Event::Text(text_bytes))?;
                }
                Node::Comment(_) => (),
            }
        }

        let end_bytes = BytesEnd::borrowed(name.as_bytes());
        writer.write_event(Event::End(end_bytes))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::io::Cursor;

    fn tag() -> Element {
        let subtag = Element::builder("ns:subtag")
            .attr("id", "my_subtag")
            .append(Node::Text(String::from("Some text")))
            .build();
        Element::builder("tag")
            .attr("id", "my_tag")
            .append(subtag)
            .build()
    }

    #[test]
    fn write_xml() {
        let tag = tag();
        let element_writer = ElementWriter::new(tag, false);
        let mut write = Cursor::new(Vec::new());
        element_writer.write(&mut write).unwrap();
        let expected = r#"<?xml version="1.0" encoding="UTF-8"?><tag id="my_tag"><ns:subtag id="my_subtag">Some text</ns:subtag></tag>"#;
        assert_eq!(expected, String::from_utf8(write.into_inner()).unwrap());
    }

    #[test]
    fn write_xml_with_indent() {
        let tag = tag();
        let element_writer = ElementWriter::new(tag, true);
        let mut write = Cursor::new(Vec::new());
        element_writer.write(&mut write).unwrap();
        let expected = r#"<?xml version="1.0" encoding="UTF-8"?>
<tag id="my_tag">
	<ns:subtag id="my_subtag">Some text</ns:subtag>
</tag>"#;
        assert_eq!(expected, String::from_utf8(write.into_inner()).unwrap());
    }
}
