use std::io::{Read, Write};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

use crate::error::{QuanergyError, Result};

pub const SIGNATURE: u32 = 0x75bd_7e97;
pub const HEADER_LEN: usize = 20;
pub const DEFAULT_TCP_PORT: u16 = 4141;
pub const DEVICE_INFO_PORT: u16 = 7780;
pub const DEVICE_INFO_PATH: &str = "/PSIA/System/deviceInfo";

pub const M_SERIES_FIRINGS_PER_PACKET: usize = 50;
pub const M_SERIES_NUM_RETURNS: usize = 3;
pub const M_SERIES_NUM_LASERS: usize = 8;
pub const M_SERIES_NUM_ROT_ANGLES: usize = 10_400;
pub const ALL_RETURNS: i8 = -1;

pub const PACKET_TYPE_M_SERIES: u8 = 0x00;
pub const PACKET_TYPE_HVDIR_LIST: u8 = 0x01;
pub const PACKET_TYPE_M_SERIES_REDUCED: u8 = 0x04;
pub const PACKET_TYPE_M1: u8 = 0x06;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct PacketHeader {
    pub signature: u32,
    pub size: u32,
    pub seconds: u32,
    pub nanoseconds: u32,
    pub version_major: u8,
    pub version_minor: u8,
    pub version_patch: u8,
    pub packet_type: u8,
}

impl PacketHeader {
    pub fn read_from(mut reader: impl Read) -> Result<Self> {
        let header = Self {
            signature: reader.read_u32::<BigEndian>()?,
            size: reader.read_u32::<BigEndian>()?,
            seconds: reader.read_u32::<BigEndian>()?,
            nanoseconds: reader.read_u32::<BigEndian>()?,
            version_major: reader.read_u8()?,
            version_minor: reader.read_u8()?,
            version_patch: reader.read_u8()?,
            packet_type: reader.read_u8()?,
        };
        header.validate()?;
        Ok(header)
    }

    pub fn parse(packet: &[u8]) -> Result<Self> {
        if packet.len() < HEADER_LEN {
            return Err(QuanergyError::PacketTooShort {
                actual: packet.len(),
                minimum: HEADER_LEN,
            });
        }

        let header = Self::read_from(&packet[..HEADER_LEN])?;
        let expected = header.size as usize;
        if expected != packet.len() {
            return Err(QuanergyError::PacketSizeMismatch {
                expected,
                actual: packet.len(),
            });
        }
        Ok(header)
    }

    pub fn write_to(self, mut writer: impl Write) -> Result<()> {
        writer.write_u32::<BigEndian>(self.signature)?;
        writer.write_u32::<BigEndian>(self.size)?;
        writer.write_u32::<BigEndian>(self.seconds)?;
        writer.write_u32::<BigEndian>(self.nanoseconds)?;
        writer.write_u8(self.version_major)?;
        writer.write_u8(self.version_minor)?;
        writer.write_u8(self.version_patch)?;
        writer.write_u8(self.packet_type)?;
        Ok(())
    }

    pub fn validate(self) -> Result<()> {
        if self.signature != SIGNATURE {
            return Err(QuanergyError::InvalidSignature(self.signature));
        }
        if self.size < HEADER_LEN as u32 {
            return Err(QuanergyError::PacketSizeMismatch {
                expected: HEADER_LEN,
                actual: self.size as usize,
            });
        }
        Ok(())
    }

    pub fn timestamp_micros(self) -> u64 {
        self.seconds as u64 * 1_000_000 + self.nanoseconds as u64 / 1_000
    }

    pub fn require_version(self, major: u8, minor: u8, patch: u8) -> Result<()> {
        if self.version_major == major && self.version_minor == minor && self.version_patch == patch
        {
            Ok(())
        } else {
            Err(QuanergyError::UnsupportedPacketVersion {
                packet_type: self.packet_type,
                major: self.version_major,
                minor: self.version_minor,
                patch: self.version_patch,
            })
        }
    }
}

#[derive(Debug, Clone)]
pub struct RawPacket {
    pub header: PacketHeader,
    pub bytes: Vec<u8>,
    pub arrival_delta_ns: u64,
}

impl RawPacket {
    pub fn parse(bytes: Vec<u8>, arrival_delta_ns: u64) -> Result<Self> {
        let header = PacketHeader::parse(&bytes)?;
        Ok(Self {
            header,
            bytes,
            arrival_delta_ns,
        })
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ReturnSelection {
    Single(u8),
    All,
}

impl Default for ReturnSelection {
    fn default() -> Self {
        Self::Single(0)
    }
}

impl ReturnSelection {
    pub fn parse(value: &str) -> Result<Self> {
        if value.eq_ignore_ascii_case("all") {
            return Ok(Self::All);
        }
        let id: u8 = value
            .parse()
            .map_err(|_| QuanergyError::InvalidReturnSelection(value.to_owned()))?;
        if (id as usize) < M_SERIES_NUM_RETURNS {
            Ok(Self::Single(id))
        } else {
            Err(QuanergyError::InvalidReturnSelection(value.to_owned()))
        }
    }

    pub fn as_cpp_value(self) -> i8 {
        match self {
            Self::Single(id) => id as i8,
            Self::All => ALL_RETURNS,
        }
    }
}

pub fn horizontal_angle_lut() -> Vec<f32> {
    let mut lut = Vec::with_capacity(M_SERIES_NUM_ROT_ANGLES + 1);
    for i in 0..=M_SERIES_NUM_ROT_ANGLES {
        let j = (i + M_SERIES_NUM_ROT_ANGLES / 2) % M_SERIES_NUM_ROT_ANGLES;
        let n = j as f32 / M_SERIES_NUM_ROT_ANGLES as f32;
        lut.push(n * std::f32::consts::TAU - std::f32::consts::PI);
    }
    lut
}

pub fn m8_vertical_angles() -> [f32; M_SERIES_NUM_LASERS] {
    [
        -0.318_505,
        -0.269_2,
        -0.218_009,
        -0.165_195,
        -0.111_003,
        -0.055_798_2,
        0.0,
        0.055_798_2,
    ]
}

pub fn mq8_vertical_angles() -> [f32; M_SERIES_NUM_LASERS] {
    [
        -0.244_35, -0.183_26, -0.141_37, -0.102_97, -0.078_54, -0.055_327, -0.041_364, -0.027_402,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_big_endian_header() {
        let header = PacketHeader {
            signature: SIGNATURE,
            size: HEADER_LEN as u32,
            seconds: 1,
            nanoseconds: 2_000,
            version_major: 0,
            version_minor: 1,
            version_patch: 0,
            packet_type: PACKET_TYPE_HVDIR_LIST,
        };
        let mut bytes = Vec::new();
        header.write_to(&mut bytes).unwrap();

        assert_eq!(PacketHeader::parse(&bytes).unwrap(), header);
    }

    #[test]
    fn rejects_invalid_signature() {
        let mut bytes = vec![0; HEADER_LEN];
        bytes[3] = 1;
        bytes[7] = HEADER_LEN as u8;

        assert!(matches!(
            PacketHeader::parse(&bytes),
            Err(QuanergyError::InvalidSignature(_))
        ));
    }
}
