use std::collections::HashMap;

use quick_xml::{events::Event, Reader, XmlVersion};

use crate::Result;

pub fn flatten_xml(xml: &str) -> Result<HashMap<String, String>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut path: Vec<String> = Vec::new();
    let mut values = HashMap::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let mut name = String::from_utf8_lossy(event.name().as_ref()).to_string();
                if name == "laser" {
                    for attr in event.attributes().flatten() {
                        if attr.key.as_ref() == b"id" {
                            let id = attr.decoded_and_normalized_value(
                                XmlVersion::Implicit1_0,
                                reader.decoder(),
                            )?;
                            name = format!("laser#{id}");
                        }
                    }
                }
                path.push(name);
            }
            Ok(Event::Text(text)) => {
                let value = text
                    .decode()
                    .map_err(quick_xml::Error::from)?
                    .trim()
                    .to_owned();
                if !value.is_empty() && !path.is_empty() {
                    values.insert(path.join("."), value);
                }
            }
            Ok(Event::End(_)) => {
                path.pop();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
    }

    Ok(values)
}

pub(super) fn parse_optional_f32(value: &str) -> Option<f32> {
    let trimmed = value.trim();
    (!trimmed.is_empty())
        .then(|| trimmed.parse().ok())
        .flatten()
}

pub(super) fn parse_optional_usize(value: &str) -> Option<usize> {
    let trimmed = value.trim();
    (!trimmed.is_empty())
        .then(|| trimmed.parse().ok())
        .flatten()
}

pub(super) fn parse_optional_u8(value: &str) -> Option<u8> {
    let trimmed = value.trim();
    (!trimmed.is_empty())
        .then(|| trimmed.parse().ok())
        .flatten()
}

pub(super) fn parse_optional_bool(value: &str) -> Option<bool> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        match trimmed.to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Some(true),
            "false" | "0" | "no" => Some(false),
            _ => None,
        }
    }
}
