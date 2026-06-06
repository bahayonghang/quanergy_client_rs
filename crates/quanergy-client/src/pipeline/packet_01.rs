use std::io::Cursor;

use byteorder::{BigEndian, ReadBytesExt};

use crate::{
    cloud::{Frame, PointHvdir},
    error::{QuanergyError, Result},
    protocol::{PacketHeader, HEADER_LEN},
};

use super::helpers::ensure_packet_len;

pub(super) fn parse_01(
    packet: &[u8],
    header: PacketHeader,
    frame_id: &str,
) -> Result<Vec<Frame<PointHvdir>>> {
    header.require_version(0, 1, 0)?;
    if packet.len() < HEADER_LEN + 16 {
        return Err(QuanergyError::PacketTooShort {
            actual: packet.len(),
            minimum: HEADER_LEN + 16,
        });
    }

    let mut cursor = Cursor::new(&packet[HEADER_LEN..]);
    let sequence = cursor.read_u32::<BigEndian>()? as u64;
    let _status = cursor.read_u32::<BigEndian>()?;
    let point_count = cursor.read_u32::<BigEndian>()? as usize;
    let _reserved = cursor.read_u32::<BigEndian>()?;
    let expected = HEADER_LEN + 16 + point_count * 12;
    ensure_packet_len(packet, expected)?;

    let mut frame = Frame::new(frame_id.to_owned());
    frame.stamp_micros = header.timestamp_micros();
    frame.sequence = sequence;
    frame.points.reserve(point_count);

    let mut ring_angles: Vec<(f32, u16)> = Vec::new();
    for _ in 0..point_count {
        let horizontal_angle = cursor.read_i16::<BigEndian>()? as f32 * 1e-4;
        let vertical_angle = cursor.read_i16::<BigEndian>()? as f32 * 1e-4;
        let range = cursor.read_u32::<BigEndian>()? as f32 * 1e-6;
        let intensity = cursor.read_u16::<BigEndian>()? as f32;
        let _status = cursor.read_u8()?;
        let _reserved = cursor.read_u8()?;

        let cos_h = horizontal_angle.cos();
        let h = horizontal_angle.sin().atan2(cos_h * vertical_angle.cos());
        let v = (cos_h * vertical_angle.sin()).asin();
        let ring = ring_for_vertical_angle(vertical_angle, &mut ring_angles);
        frame.points.push(PointHvdir {
            h,
            v,
            d: range,
            intensity,
            ring,
        });
    }
    frame.refresh_unorganized_dims();
    Ok(vec![frame])
}

fn ring_for_vertical_angle(vertical_angle: f32, ring_angles: &mut Vec<(f32, u16)>) -> u16 {
    const RING_VERTICAL_ANGLE_RESOLUTION: f32 = 0.1 * std::f32::consts::PI / 180.0;
    if let Some((_, ring)) = ring_angles
        .iter()
        .find(|(angle, _)| (vertical_angle - *angle).abs() < RING_VERTICAL_ANGLE_RESOLUTION)
    {
        return *ring;
    }

    let ring = ring_angles.len() as u16;
    ring_angles.push((vertical_angle, ring));
    ring
}
