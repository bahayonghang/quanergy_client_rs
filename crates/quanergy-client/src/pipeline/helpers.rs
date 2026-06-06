use crate::{
    cloud::{Frame, PointHvdir},
    error::{QuanergyError, Result},
    protocol::{M_SERIES_NUM_LASERS, M_SERIES_NUM_RETURNS},
};

pub(super) fn point_for_distance(
    base: PointHvdir,
    distance: u32,
    intensity: u8,
    scale: f32,
) -> PointHvdir {
    PointHvdir {
        d: if distance == 0 {
            f32::NAN
        } else {
            distance as f32 * scale
        },
        intensity: intensity as f32,
        ..base
    }
}

pub(super) fn push_all_returns(
    output: &mut Vec<PointHvdir>,
    base: PointHvdir,
    distances: &[[u32; M_SERIES_NUM_LASERS]; M_SERIES_NUM_RETURNS],
    intensities: &[[u8; M_SERIES_NUM_LASERS]; M_SERIES_NUM_RETURNS],
    laser: usize,
    scale: f32,
) {
    let dist2 = distances[2][laser];
    let dist0 = distances[0][laser];
    if dist0 != 0 && dist0 != dist2 {
        output.push(point_for_distance(
            base,
            dist0,
            intensities[0][laser],
            scale,
        ));
    }
    let dist1 = distances[1][laser];
    if dist1 != 0 && dist1 != dist2 {
        output.push(point_for_distance(
            base,
            dist1,
            intensities[1][laser],
            scale,
        ));
    }
    if dist2 != 0 {
        output.push(point_for_distance(
            base,
            dist2,
            intensities[2][laser],
            scale,
        ));
    }
}

pub(super) fn organize_cloud(
    mut frame: Frame<PointHvdir>,
    height: usize,
) -> Result<Frame<PointHvdir>> {
    if height == 0 || frame.points.len() % height != 0 {
        return Err(QuanergyError::Config(
            "cannot organize cloud when size is not divisible by height".to_owned(),
        ));
    }
    let width = frame.points.len() / height;
    if height != 1 {
        let mut points = Vec::with_capacity(frame.points.len());
        for ring in (0..height).rev() {
            for column in 0..width {
                points.push(frame.points[column * height + ring]);
            }
        }
        frame.points = points;
    }
    frame.height = height;
    frame.width = width;
    Ok(frame)
}

pub(super) fn validate_status(status: u16) -> Result<()> {
    if status & 0b11 != 0 {
        Err(QuanergyError::InvalidSensorStatus(status))
    } else {
        Ok(())
    }
}

pub(super) fn ensure_packet_len(packet: &[u8], expected: usize) -> Result<()> {
    if packet.len() == expected {
        Ok(())
    } else {
        Err(QuanergyError::PacketSizeMismatch {
            expected,
            actual: packet.len(),
        })
    }
}
