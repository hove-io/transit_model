// Copyright (C) 2017 Hove and/or its affiliates.
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

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use std::io::{self, Write};

/// Represents an XML node (either an Element or Text)
#[derive(Debug, Clone)]
pub enum Node {
    Element(Element),
    Text(String),
}

impl From<Element> for Node {
    fn from(element: Element) -> Self {
        Node::Element(element)
    }
}

impl From<String> for Node {
    fn from(text: String) -> Self {
        Node::Text(text)
    }
}

impl From<&str> for Node {
    fn from(text: &str) -> Self {
        Node::Text(text.to_string())
    }
}

impl Node {
    /// Get the text content if this is a Text node
    #[cfg(test)]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Node::Text(text) => Some(text.as_str()),
            _ => None,
        }
    }
}

/// Represents an XML element with name, attributes, and children
#[derive(Debug, Clone)]
pub struct Element {
    name: String,
    namespace: Option<String>,
    attributes: Vec<(String, String)>,
    children: Vec<Node>,
}

impl Element {
    /// Create a new builder for an element
    pub fn builder<S: Into<String>>(name: S) -> ElementBuilder {
        ElementBuilder {
            name: name.into(),
            namespace: None,
            attributes: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Get the name of this element
    #[allow(dead_code)]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get an attribute value by name
    #[allow(dead_code)]
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// Get all children
    pub fn children(&self) -> &[Node] {
        &self.children
    }

    /// Get an iterator over all children nodes
    #[cfg(test)]
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.children.iter()
    }

    /// Write this element to a writer with pretty formatting
    pub fn write_to<W: Write>(&self, writer: &mut Writer<W>) -> io::Result<()> {
        self.write_to_impl(writer)
    }

    fn write_to_impl<W: Write>(&self, writer: &mut Writer<W>) -> io::Result<()> {
        let mut start = BytesStart::new(&self.name);

        // Add namespace if present
        if let Some(ref ns) = self.namespace {
            start.push_attribute(("xmlns", ns.as_str()));
        }

        // Add all attributes
        for (key, value) in &self.attributes {
            start.push_attribute((key.as_str(), value.as_str()));
        }

        writer.write_event(Event::Start(start))?;

        // Write children
        for child in &self.children {
            match child {
                Node::Element(elem) => elem.write_to_impl(writer)?,
                Node::Text(text) => {
                    writer.write_event(Event::Text(BytesText::new(text)))?;
                }
            }
        }

        writer.write_event(Event::End(BytesEnd::new(&self.name)))?;
        Ok(())
    }
}

/// Builder for creating Elements
pub struct ElementBuilder {
    name: String,
    namespace: Option<String>,
    attributes: Vec<(String, String)>,
    children: Vec<Node>,
}

impl ElementBuilder {
    /// Set the namespace for this element
    #[allow(dead_code)]
    pub fn ns<S: Into<String>>(mut self, namespace: S) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    /// Add an attribute to this element
    pub fn attr<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.attributes.push((key.into(), value.into()));
        self
    }

    /// Append a child node
    pub fn append<N: Into<Node>>(mut self, child: N) -> Self {
        self.children.push(child.into());
        self
    }

    /// Append multiple children
    pub fn append_all<I, N>(mut self, children: I) -> Self
    where
        I: IntoIterator<Item = N>,
        N: Into<Node>,
    {
        self.children.extend(children.into_iter().map(|n| n.into()));
        self
    }

    /// Build the final Element
    pub fn build(self) -> Element {
        Element {
            name: self.name,
            namespace: self.namespace,
            attributes: self.attributes,
            children: self.children,
        }
    }
}

/// Writer for Elements with pretty printing
pub struct ElementWriter<W: Write> {
    writer: Writer<W>,
}

impl<W: Write> ElementWriter<W> {
    /// Create a new writer with pretty printing (using tabs for indentation)
    pub fn pretty(inner: W) -> Self {
        let writer = Writer::new_with_indent(inner, b'\t', 1);
        ElementWriter { writer }
    }

    /// Write an element with XML declaration
    pub fn write(&mut self, element: &Element) -> io::Result<()> {
        // Write XML declaration
        self.writer
            .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

        // Write the element
        element.write_to(&mut self.writer)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_element() {
        let elem = Element::builder("test")
            .attr("id", "123")
            .append("Hello")
            .build();

        assert_eq!(elem.name(), "test");
        assert_eq!(elem.attr("id"), Some("123"));
    }

    #[test]
    fn test_nested_elements() {
        let child = Element::builder("child").append("text").build();
        let parent = Element::builder("parent").append(child).build();

        assert_eq!(parent.name(), "parent");
        assert_eq!(parent.children().len(), 1);
    }

    #[test]
    fn test_xml_output() {
        let elem = Element::builder("root")
            .attr("version", "1.0")
            .append(Element::builder("child").append("Hello World").build())
            .build();

        let mut buffer = Vec::new();
        let mut writer = ElementWriter::pretty(&mut buffer);
        writer.write(&elem).unwrap();

        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains("<?xml"));
        assert!(output.contains("<root"));
        assert!(output.contains("version=\"1.0\""));
        assert!(output.contains("<child>"));
        assert!(output.contains("Hello World"));
    }
}
