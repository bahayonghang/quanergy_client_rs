use std::io;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, QuanergyError>;

#[derive(Debug, Error)]
pub enum QuanergyError {
    #[error("invalid packet signature 0x{0:08x}")]
    InvalidSignature(u32),

    #[error("packet is too short: got {actual} bytes, need at least {minimum}")]
    PacketTooShort { actual: usize, minimum: usize },

    #[error("packet size mismatch: header says {expected} bytes, buffer has {actual} bytes")]
    PacketSizeMismatch { expected: usize, actual: usize },

    #[error("unsupported packet type 0x{0:02x}")]
    UnsupportedPacketType(u8),

    #[error(
        "unsupported packet version {major}.{minor}.{patch} for packet type 0x{packet_type:02x}"
    )]
    UnsupportedPacketVersion {
        packet_type: u8,
        major: u8,
        minor: u8,
        patch: u8,
    },

    #[error("invalid return selection: {0}")]
    InvalidReturnSelection(String),

    #[error("return id mismatch: requested {requested}, packet contains {actual}")]
    ReturnIdMismatch { requested: u8, actual: u8 },

    #[error("invalid vertical angles: {0}")]
    InvalidVerticalAngles(String),

    #[error("invalid sensor status 0x{0:04x}")]
    InvalidSensorStatus(u16),

    #[error("calibration failed: {0}")]
    Calibration(String),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("replay format error: {0}")]
    ReplayFormat(String),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("XML error: {0}")]
    Xml(#[from] quick_xml::Error),

    #[error("TOML deserialize error: {0}")]
    TomlDe(#[from] toml::de::Error),

    #[error("TOML serialize error: {0}")]
    TomlSer(#[from] toml::ser::Error),

    #[error("HTTP error: {0}")]
    Http(String),
}
