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

//! Some utils to work with the NeTEx format, especially the frames.

use crate::Result;
use failure::{bail, format_err, Error};
use minidom::Element;
use minidom_ext::OnlyChildElementExt;
use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
    str::FromStr,
};

/// Type of NeTEx frame.
#[derive(Debug, Eq, Hash, PartialEq)]
pub enum FrameType {
    /// Type of a `<CompositeFrame>`
    Composite,
    /// Type of a `<FareFrame>`
    Fare,
    /// Type of a `<GeneralFrame>`
    General,
    /// Type of a `<ResourceFrame>`
    Resource,
    /// Type of a `<ServiceFrame>`
    Service,
}
/// Map of frames, categorized by `FrameType`. Multiple frames of the same type
/// can exist, they're stored in a `Vec`.
pub type Frames<'a> = HashMap<FrameType, Vec<&'a Element>>;

impl Display for FrameType {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        use FrameType::*;
        match self {
            Composite => write!(f, "CompositeFrame"),
            Fare => write!(f, "FareFrame"),
            General => write!(f, "GeneralFrame"),
            Resource => write!(f, "ResourceFrame"),
            Service => write!(f, "ServiceFrame"),
        }
    }
}

impl FromStr for FrameType {
    type Err = Error;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        use FrameType::*;
        match s {
            "CompositeFrame" => Ok(Composite),
            "FareFrame" => Ok(Fare),
            "GeneralFrame" => Ok(General),
            "ResourceFrame" => Ok(Resource),
            "ServiceFrame" => Ok(Service),
            _ => bail!("Failed to convert '{}' into a FrameType", s),
        }
    }
}

/// Returns a map of all frames (pointer to an `Element`) per `FrameType`.
/// The input parameter must be an `Element` (XML element) that contains frames.
/// Usually, it will be an element call `<frames>` in NeTEx standard.
pub fn parse_frames_by_type<'a>(frames: &'a Element) -> Result<Frames<'a>> {
    frames
        .children()
        .try_fold(HashMap::new(), |mut map, frame| {
            let frame_type: FrameType = frame.name().parse()?;
            map.entry(frame_type).or_insert_with(Vec::new).push(frame);
            Ok(map)
        })
}

/// Extract a frame of type `frame_type` from the map of `frames`.  This
/// function fails if none or more than one frame is found.
pub fn get_only_frame<'a>(frames: &'a Frames<'a>, frame_type: FrameType) -> Result<&'a Element> {
    let frame = frames
        .get(&frame_type)
        .ok_or_else(|| format_err!("Failed to find a '{}' frame in the Netex file", frame_type))?;
    if frame.len() == 1 {
        Ok(frame[0])
    } else {
        bail!(
            "Failed to find a unique '{}' frame in the Netex file",
            frame_type
        )
    }
}

/// Returns the value from its key in a `<KeyList>` XML element (standard
/// element of NeTEx format). You can convert the result into the type you want.
/// ```
/// # use minidom::Element;
/// # use transit_model::netex_utils::get_value_in_keylist;
/// let xml = r#"<root>
///         <KeyList>
///             <KeyValue>
///                 <Key>key</Key>
///                 <Value>42</Value>
///             </KeyValue>
///         </KeyList>
///     </root>"#;
/// let root: Element = xml.parse().unwrap();
/// let value: u32 = get_value_in_keylist(&root, "key").unwrap();
/// assert_eq!(42, value);
/// ```
pub fn get_value_in_keylist<F>(element: &Element, key: &str) -> Result<F>
where
    F: FromStr,
{
    let values = element
        .try_only_child("KeyList")
        .map_err(|e| format_err!("{}", e))?
        .children()
        .filter(|key_value| match key_value.try_only_child("Key") {
            Ok(k) => k.text() == key,
            _ => false,
        })
        .map(|key_value| {
            key_value
                .try_only_child("Value")
                .map_err(|e| format_err!("{}", e))
        })
        .collect::<Result<Vec<_>>>()?;
    if values.len() != 1 {
        bail!(
            "Failed to find a unique key '{}' in '{}'",
            key,
            element.name()
        )
    }
    values[0]
        .text()
        .parse()
        .map_err(|_| format_err!("Failed to get the value out of 'KeyList' for key '{}'", key))
}

#[cfg(test)]
mod tests {
    use super::*;

    mod frame_type {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn parse_frame_type() {
            let frame_type: FrameType = "ServiceFrame".parse().unwrap();
            assert_eq!(FrameType::Service, frame_type);
        }

        #[test]
        #[should_panic(expected = "Failed to convert \\'NotAFrameType\\' into a FrameType")]
        fn parse_invalid_frame_type() {
            "NotAFrameType".parse::<FrameType>().unwrap();
        }
    }

    mod parse_frames_by_type {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn some_frame() {
            let xml = r#"<root>
                    <FareFrame />
                    <ServiceFrame />
                    <FareFrame />
                </root>"#;
            let root: Element = xml.parse().unwrap();
            let frames = parse_frames_by_type(&root).unwrap();
            assert_eq!(2, frames.keys().count());
            assert_eq!(1, frames.get(&FrameType::Service).unwrap().len());
            assert_eq!(2, frames.get(&FrameType::Fare).unwrap().len());
        }

        #[test]
        #[should_panic(expected = "Failed to convert \\'UnknownFrame\\' into a FrameType")]
        fn unknown_frame() {
            let xml = r#"<root>
                    <UnknownFrame />
                </root>"#;
            let root: Element = xml.parse().unwrap();
            parse_frames_by_type(&root).unwrap();
        }
    }

    mod get_only_frame {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn one_frame() {
            let mut frames = HashMap::new();
            let frame: Element = r#"<frame xmlns="test" />"#.parse().unwrap();
            frames.insert(FrameType::Resource, vec![&frame]);
            let resource_frame = get_only_frame(&frames, FrameType::Resource).unwrap();
            assert_eq!("frame", resource_frame.name());
        }

        #[test]
        #[should_panic(expected = "Failed to find a \\'ServiceFrame\\' frame in the Netex file")]
        fn no_frame() {
            let frames = HashMap::new();
            get_only_frame(&frames, FrameType::Service).unwrap();
        }

        #[test]
        #[should_panic(
            expected = "Failed to find a unique \\'ResourceFrame\\' frame in the Netex file"
        )]
        fn multiple_frames() {
            let mut frames = HashMap::new();
            let frame: Element = r#"<frame xmlns="test" />"#.parse().unwrap();
            frames.insert(FrameType::Resource, vec![&frame, &frame]);
            get_only_frame(&frames, FrameType::Resource).unwrap();
        }
    }

    mod value_in_keylist {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn has_value() {
            let xml = r#"<root>
                    <KeyList>
                        <KeyValue>
                            <Key>key</Key>
                            <Value>42</Value>
                        </KeyValue>
                    </KeyList>
                </root>"#;
            let root: Element = xml.parse().unwrap();
            let value: u32 = get_value_in_keylist(&root, "key").unwrap();
            assert_eq!(42, value);
        }

        #[test]
        #[should_panic(expected = "No children with name \\'KeyList\\' in Element \\'root\\'")]
        fn no_keylist_found() {
            let xml = r#"<root />"#;
            let root: Element = xml.parse().unwrap();
            get_value_in_keylist::<u32>(&root, "key").unwrap();
        }

        #[test]
        #[should_panic(expected = "Failed to find a unique key \\'key\\' in \\'root\\'")]
        fn no_key_found() {
            let xml = r#"<root>
                    <KeyList />
                </root>"#;
            let root: Element = xml.parse().unwrap();
            get_value_in_keylist::<u32>(&root, "key").unwrap();
        }

        #[test]
        #[should_panic(expected = "No children with name \\'Value\\' in Element \\'KeyValue\\'")]
        fn no_value_found() {
            let xml = r#"<root>
                    <KeyList>
                        <KeyValue>
                            <Key>key</Key>
                        </KeyValue>
                    </KeyList>
                </root>"#;
            let root: Element = xml.parse().unwrap();
            get_value_in_keylist::<u32>(&root, "key").unwrap();
        }
    }
}
