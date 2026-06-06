use std::{collections::HashMap, path::Path};

use crate::{
    filters::RingIntensityFilter,
    protocol::{m8_vertical_angles, ReturnSelection, M_SERIES_NUM_LASERS},
    Result,
};

use super::{
    device_info::{DeviceInfo, SensorModel},
    xml::{
        flatten_xml, parse_optional_bool, parse_optional_f32, parse_optional_u8,
        parse_optional_usize,
    },
};

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
