use crate::{
    error::{QuanergyError, Result},
    protocol::{m8_vertical_angles, mq8_vertical_angles, M_SERIES_NUM_LASERS},
};

use super::xml::{flatten_xml, parse_optional_f32};

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
