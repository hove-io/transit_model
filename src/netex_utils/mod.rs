use crate::{minidom_utils::TryOnlyChild, Result};
use failure::{bail, format_err, Error};
use minidom::Element;
use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
    str::FromStr,
};

#[derive(Debug, Eq, Hash, PartialEq)]
pub enum FrameType {
    General,
    Resource,
    Service,
    Fare,
}

impl Display for FrameType {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            FrameType::Fare => write!(f, "Fare"),
            FrameType::General => write!(f, "General"),
            FrameType::Resource => write!(f, "Resource"),
            FrameType::Service => write!(f, "Service"),
        }
    }
}

impl FromStr for FrameType {
    type Err = Error;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "FareFrame" => Ok(FrameType::Fare),
            "GeneralFrame" => Ok(FrameType::General),
            "ResourceFrame" => Ok(FrameType::Resource),
            "ServiceFrame" => Ok(FrameType::Service),
            _ => bail!("Failed to convert '{}' into a FrameType", s),
        }
    }
}

pub fn parse_frames_by_type<'a>(
    frames: &'a Element,
) -> Result<HashMap<FrameType, Vec<&'a Element>>> {
    frames
        .children()
        .try_fold(HashMap::new(), |mut map, frame| {
            let frame_type: FrameType = frame.name().parse()?;
            map.entry(frame_type).or_insert_with(Vec::new).push(frame);
            Ok(map)
        })
}

pub fn get_only_frame<'a>(
    frames: &'a HashMap<FrameType, Vec<&'a Element>>,
    frame_type: FrameType,
) -> Result<&'a Element> {
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

pub fn get_value_in_keylist<F>(element: &Element, key: &str) -> Result<F>
where
    F: FromStr,
{
    let values = element
        .try_only_child("KeyList")?
        .children()
        .filter(|key_value| match key_value.try_only_child("Key") {
            Ok(k) => k.text() == key,
            _ => false,
        })
        .map(|key_value| key_value.try_only_child("Value"))
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
            assert_eq!(frame_type, FrameType::Service);
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
            assert_eq!(frames.keys().count(), 2);
            assert_eq!(frames.get(&FrameType::Service).unwrap().len(), 1);
            assert_eq!(frames.get(&FrameType::Fare).unwrap().len(), 2);
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
            assert_eq!(resource_frame.name(), "frame");
        }

        #[test]
        #[should_panic(expected = "Failed to find a \\'Service\\' frame in the Netex file")]
        fn no_frame() {
            let frames = HashMap::new();
            get_only_frame(&frames, FrameType::Service).unwrap();
        }

        #[test]
        #[should_panic(expected = "Failed to find a unique \\'Resource\\' frame in the Netex file")]
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
            assert_eq!(value, 42);
        }

        #[test]
        #[should_panic(expected = "Failed to find a child \\'KeyList\\' in element \\'root\\'")]
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
        #[should_panic(expected = "Failed to find a child \\'Value\\' in element \\'KeyValue\\'")]
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
