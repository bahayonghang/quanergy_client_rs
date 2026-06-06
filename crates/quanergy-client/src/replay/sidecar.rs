use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{
    config::{EncoderMode, PipelineConfig},
    Result,
};

use super::current_time_string;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarMetadata {
    pub format_version: u8,
    pub sensor_host: Option<String>,
    pub capture_started_at: String,
    pub client_version: String,
    pub model: Option<String>,
    pub vertical_angles: Option<Vec<f32>>,
    pub encoder_amplitude: Option<f32>,
    pub encoder_phase: Option<f32>,
    pub calibration_complete: bool,
    pub calibration_error: Option<String>,
}

impl SidecarMetadata {
    pub fn from_config(host: Option<String>, config: &PipelineConfig) -> Self {
        let (encoder_amplitude, encoder_phase) = match config.encoder_mode {
            EncoderMode::Manual { amplitude, phase } => (Some(amplitude), Some(phase)),
            EncoderMode::Disabled | EncoderMode::DeviceInfo | EncoderMode::Automatic => {
                (None, None)
            }
        };

        Self {
            format_version: 1,
            sensor_host: host,
            capture_started_at: current_time_string(),
            client_version: env!("CARGO_PKG_VERSION").to_owned(),
            model: config.model.as_ref().map(|model| format!("{model:?}")),
            vertical_angles: config.vertical_angles.clone(),
            encoder_amplitude,
            encoder_phase,
            calibration_complete: true,
            calibration_error: None,
        }
    }

    pub fn incomplete(host: Option<String>, error: impl Into<String>) -> Self {
        Self {
            format_version: 1,
            sensor_host: host,
            capture_started_at: current_time_string(),
            client_version: env!("CARGO_PKG_VERSION").to_owned(),
            model: None,
            vertical_angles: None,
            encoder_amplitude: None,
            encoder_phase: None,
            calibration_complete: false,
            calibration_error: Some(error.into()),
        }
    }

    pub fn sidecar_path(qraw_path: impl AsRef<Path>) -> PathBuf {
        let path = qraw_path.as_ref();
        let mut sidecar = path.as_os_str().to_owned();
        sidecar.push(".toml");
        PathBuf::from(sidecar)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&text)?)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        std::fs::write(path, toml::to_string_pretty(self)?)?;
        Ok(())
    }
}
