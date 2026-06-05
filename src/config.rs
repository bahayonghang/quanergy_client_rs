use std::{collections::HashMap, path::Path};

use quick_xml::{events::Event, Reader, XmlVersion};

use crate::{
    error::{QuanergyError, Result},
    filters::RingIntensityFilter,
    protocol::{m8_vertical_angles, mq8_vertical_angles, ReturnSelection, M_SERIES_NUM_LASERS},
};

#[derive(Debug, Clone, PartialEq)]
pub enum SensorModel {
    M8,
    Mq8,
    M1,
    Other(String),
}

impl SensorModel {
    pub fn parse(value: &str) -> Self {
        let lower = value.to_ascii_lowercase();
        if lower.contains("mq") {
            Self::Mq8
        } else if lower.contains("m1") {
            Self::M1
        } else if lower.contains("m8") || lower.starts_with('m') {
            Self::M8
        } else {
            Self::Other(value.to_owned())
        }
    }

    pub fn default_vertical_angles(&self) -> Option<Vec<f32>> {
        match self {
            Self::M8 => Some(m8_vertical_angles().to_vec()),
            Self::Mq8 => Some(mq8_vertical_angles().to_vec()),
            Self::M1 | Self::Other(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub model: SensorModel,
    pub vertical_angles: Option<Vec<f32>>,
    pub encoder_amplitude: Option<f32>,
    pub encoder_phase: Option<f32>,
}

impl DeviceInfo {
    pub fn parse_xml(xml: &str) -> Result<Self> {
        let values = flatten_xml(xml)?;
        let model = values
            .get("DeviceInfo.model")
            .map(|value| SensorModel::parse(value))
            .ok_or_else(|| {
                QuanergyError::Config("deviceInfo missing DeviceInfo.model".to_owned())
            })?;

        let encoder_amplitude = values
            .get("DeviceInfo.calibration.encoder.amplitude")
            .and_then(|value| parse_optional_f32(value));
        let encoder_phase = values
            .get("DeviceInfo.calibration.encoder.phase")
            .and_then(|value| parse_optional_f32(value));

        let mut vertical_angles = Vec::new();
        for index in 0..M_SERIES_NUM_LASERS {
            let key = format!("DeviceInfo.calibration.lasers.laser#{index}.v");
            if let Some(value) = values.get(&key).and_then(|value| parse_optional_f32(value)) {
                if vertical_angles.len() <= index {
                    vertical_angles.resize(index + 1, 0.0);
                }
                vertical_angles[index] = value;
            }
        }

        Ok(Self {
            model,
            vertical_angles: (!vertical_angles.is_empty()).then_some(vertical_angles),
            encoder_amplitude,
            encoder_phase,
        })
    }
}

#[derive(Debug, Clone)]
pub enum EncoderMode {
    Disabled,
    Manual { amplitude: f32, phase: f32 },
    DeviceInfo,
    Automatic,
}

#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub host: String,
    pub frame_id: String,
    pub return_selection: ReturnSelection,
    pub return_selection_set: bool,
    pub encoder_mode: EncoderMode,
    pub frame_rate: f32,
    pub min_distance: f32,
    pub max_distance: f32,
    pub min_cloud_size: usize,
    pub max_cloud_size: usize,
    pub ring_filter: RingIntensityFilter,
    pub vertical_angles: Option<Vec<f32>>,
    pub model: Option<SensorModel>,
    pub strict: bool,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            frame_id: "quanergy".to_owned(),
            return_selection: ReturnSelection::default(),
            return_selection_set: false,
            encoder_mode: EncoderMode::DeviceInfo,
            frame_rate: 10.0,
            min_distance: 0.0,
            max_distance: 500.0,
            min_cloud_size: 1,
            max_cloud_size: 1_000_000,
            ring_filter: RingIntensityFilter::default(),
            vertical_angles: Some(m8_vertical_angles().to_vec()),
            model: Some(SensorModel::M8),
            strict: false,
        }
    }
}

impl PipelineConfig {
    pub fn load_settings_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let text = std::fs::read_to_string(path)?;
        self.apply_settings_xml(&text)
    }

    pub fn apply_settings_xml(&mut self, xml: &str) -> Result<()> {
        let values = flatten_xml(xml)?;
        if let Some(value) = values
            .get("Settings.host")
            .filter(|value| !value.is_empty())
        {
            self.host = value.clone();
        }
        if let Some(value) = values
            .get("Settings.frame")
            .filter(|value| !value.is_empty())
        {
            self.frame_id = value.clone();
        }
        if let Some(value) = values
            .get("Settings.return")
            .filter(|value| !value.is_empty())
        {
            self.return_selection = ReturnSelection::parse(value)?;
            self.return_selection_set = true;
        }
        if let Some(value) = values
            .get("Settings.DistanceFilter.min")
            .and_then(|value| parse_optional_f32(value))
        {
            self.min_distance = value;
        }
        if let Some(value) = values
            .get("Settings.DistanceFilter.max")
            .and_then(|value| parse_optional_f32(value))
        {
            self.max_distance = value;
        }

        let calibrate = values
            .get("Settings.EncoderCorrection.calibrate")
            .and_then(|value| parse_optional_bool(value))
            .unwrap_or(false);
        let override_encoder_params = values
            .get("Settings.EncoderCorrection.override")
            .and_then(|value| parse_optional_bool(value))
            .unwrap_or(false);
        if let Some(value) = values
            .get("Settings.EncoderCorrection.frameRate")
            .and_then(|value| parse_optional_f32(value))
        {
            self.frame_rate = value;
        }
        let amplitude = values
            .get("Settings.EncoderCorrection.amplitude")
            .and_then(|value| parse_optional_f32(value))
            .unwrap_or(0.0);
        let phase = values
            .get("Settings.EncoderCorrection.phase")
            .and_then(|value| parse_optional_f32(value))
            .unwrap_or(0.0);
        if calibrate {
            self.encoder_mode = EncoderMode::Automatic;
        } else if override_encoder_params {
            self.encoder_mode = EncoderMode::Manual { amplitude, phase };
        }

        if let Some(value) = values
            .get("Settings.minCloudSize")
            .and_then(|value| parse_optional_usize(value))
        {
            self.min_cloud_size = value;
        }
        if let Some(value) = values
            .get("Settings.maxCloudSize")
            .and_then(|value| parse_optional_usize(value))
        {
            self.max_cloud_size = value;
        }

        for index in 0..M_SERIES_NUM_LASERS {
            if let Some(value) = get_case_fallback(
                &values,
                &format!("Settings.RingFilter.range{index}"),
                &format!("Settings.RingFilter.Range{index}"),
            )
            .and_then(parse_optional_f32)
            {
                self.ring_filter.min_range[index] = value;
            }
            if let Some(value) = get_case_fallback(
                &values,
                &format!("Settings.RingFilter.intensity{index}"),
                &format!("Settings.RingFilter.Intensity{index}"),
            )
            .and_then(parse_optional_u8)
            {
                self.ring_filter.min_intensity[index] = value;
            }
        }

        Ok(())
    }

    pub fn apply_device_info(&mut self, info: &DeviceInfo) {
        self.model = Some(info.model.clone());
        if let Some(vertical_angles) = &info.vertical_angles {
            self.vertical_angles = Some(vertical_angles.clone());
        } else if let Some(vertical_angles) = info.model.default_vertical_angles() {
            self.vertical_angles = Some(vertical_angles);
        }
        if matches!(self.encoder_mode, EncoderMode::DeviceInfo) {
            if let (Some(amplitude), Some(phase)) = (info.encoder_amplitude, info.encoder_phase) {
                self.encoder_mode = EncoderMode::Manual { amplitude, phase };
            }
        }
    }
}

fn get_case_fallback<'a>(
    values: &'a HashMap<String, String>,
    lower_key: &str,
    sample_key: &str,
) -> Option<&'a str> {
    values
        .get(lower_key)
        .or_else(|| values.get(sample_key))
        .map(String::as_str)
}

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

fn parse_optional_f32(value: &str) -> Option<f32> {
    let trimmed = value.trim();
    (!trimmed.is_empty())
        .then(|| trimmed.parse().ok())
        .flatten()
}

fn parse_optional_usize(value: &str) -> Option<usize> {
    let trimmed = value.trim();
    (!trimmed.is_empty())
        .then(|| trimmed.parse().ok())
        .flatten()
}

fn parse_optional_u8(value: &str) -> Option<u8> {
    let trimmed = value.trim();
    (!trimmed.is_empty())
        .then(|| trimmed.parse().ok())
        .flatten()
}

fn parse_optional_bool(value: &str) -> Option<bool> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_accepts_sample_uppercase_ring_filter_keys() {
        let xml = r#"
        <Settings>
          <RingFilter><Range0>2.5</Range0><Intensity0>7</Intensity0></RingFilter>
        </Settings>
        "#;
        let mut config = PipelineConfig::default();
        config.apply_settings_xml(xml).unwrap();
        assert_eq!(config.ring_filter.min_range[0], 2.5);
        assert_eq!(config.ring_filter.min_intensity[0], 7);
    }

    #[test]
    fn settings_accepts_cpp_lowercase_ring_filter_keys() {
        let xml = r#"
        <Settings>
          <RingFilter><range1>3.5</range1><intensity1>9</intensity1></RingFilter>
        </Settings>
        "#;
        let mut config = PipelineConfig::default();
        config.apply_settings_xml(xml).unwrap();
        assert_eq!(config.ring_filter.min_range[1], 3.5);
        assert_eq!(config.ring_filter.min_intensity[1], 9);
    }
}
