use std::{
    fs::File,
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};

use crate::{
    config::{EncoderMode, PipelineConfig},
    error::{QuanergyError, Result},
    protocol::RawPacket,
};

const QRAW_MAGIC: &[u8; 8] = b"QRAWv1\0\0";
const QRAW_RECORD_HEADER_LEN: usize = 12;

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

pub struct QrawWriter<W: Write> {
    writer: W,
}

impl QrawWriter<BufWriter<File>> {
    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        Self::new(BufWriter::new(File::create(path)?))
    }
}

impl<W: Write> QrawWriter<W> {
    pub fn new(mut writer: W) -> Result<Self> {
        writer.write_all(QRAW_MAGIC)?;
        Ok(Self { writer })
    }

    pub fn write_packet(&mut self, delta_ns: u64, packet: &[u8]) -> Result<()> {
        let len = u32::try_from(packet.len())
            .map_err(|_| QuanergyError::ReplayFormat("packet too large for qraw v1".to_owned()))?;
        self.writer.write_u64::<LittleEndian>(delta_ns)?;
        self.writer.write_u32::<LittleEndian>(len)?;
        self.writer.write_all(packet)?;
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        self.writer.flush()?;
        Ok(())
    }
}

pub struct QrawReader<R: Read> {
    reader: R,
}

impl QrawReader<BufReader<File>> {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::new(BufReader::new(File::open(path)?))
    }
}

impl<R: Read> QrawReader<R> {
    pub fn new(mut reader: R) -> Result<Self> {
        let mut magic = [0u8; QRAW_MAGIC.len()];
        reader.read_exact(&mut magic)?;
        if &magic != QRAW_MAGIC {
            return Err(QuanergyError::ReplayFormat("invalid qraw magic".to_owned()));
        }
        Ok(Self { reader })
    }

    pub fn next_packet(&mut self) -> Result<Option<RawPacket>> {
        let mut header = [0u8; QRAW_RECORD_HEADER_LEN];
        match self.reader.read_exact(&mut header) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(error) => return Err(error.into()),
        }

        let mut cursor = &header[..];
        let delta_ns = cursor.read_u64::<LittleEndian>()?;
        let len = cursor.read_u32::<LittleEndian>()? as usize;
        let mut bytes = vec![0u8; len];
        self.reader.read_exact(&mut bytes)?;
        Ok(Some(RawPacket::parse(bytes, delta_ns)?))
    }
}

pub fn current_time_string() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

#[cfg(test)]
mod tests {
    use crate::protocol::{PacketHeader, HEADER_LEN, PACKET_TYPE_HVDIR_LIST, SIGNATURE};

    use super::*;

    #[test]
    fn qraw_round_trip_preserves_delta_and_packet() {
        let mut packet = Vec::new();
        PacketHeader {
            signature: SIGNATURE,
            size: HEADER_LEN as u32,
            seconds: 1,
            nanoseconds: 2,
            version_major: 0,
            version_minor: 1,
            version_patch: 0,
            packet_type: PACKET_TYPE_HVDIR_LIST,
        }
        .write_to(&mut packet)
        .unwrap();

        let mut buffer = Vec::new();
        {
            let mut writer = QrawWriter::new(&mut buffer).unwrap();
            writer.write_packet(123, &packet).unwrap();
        }

        let mut reader = QrawReader::new(&buffer[..]).unwrap();
        let raw = reader.next_packet().unwrap().unwrap();
        assert_eq!(raw.arrival_delta_ns, 123);
        assert_eq!(raw.bytes, packet);
        assert!(reader.next_packet().unwrap().is_none());
    }
}
