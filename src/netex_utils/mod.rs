use crate::{minidom_utils::TryOnlyChild, Result};
use failure::{bail, format_err};
use minidom::Element;
use std::str::FromStr;

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
