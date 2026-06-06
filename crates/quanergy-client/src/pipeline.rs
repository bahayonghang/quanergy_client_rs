use std::io::{Cursor, Read};

use byteorder::{BigEndian, ReadBytesExt};
use tracing::warn;

use crate::{
    calibration::{apply_correction, AutoCalibrationConfig, AutoCalibrator, EncoderCorrection},
    cloud::{Frame, PointHvdir, PointXyzir},
    config::{EncoderMode, PipelineConfig},
    error::{QuanergyError, Result},
    filters::DistanceFilter,
    protocol::{
        horizontal_angle_lut, PacketHeader, RawPacket, ReturnSelection, HEADER_LEN,
        M_SERIES_FIRINGS_PER_PACKET, M_SERIES_NUM_LASERS, M_SERIES_NUM_RETURNS,
        PACKET_TYPE_HVDIR_LIST, PACKET_TYPE_M1, PACKET_TYPE_M_SERIES, PACKET_TYPE_M_SERIES_REDUCED,
    },
};

#[derive(Debug, Default, Clone)]
pub struct PipelineCounters {
    pub packets_seen: u64,
    pub frames_emitted: u64,
    pub bad_packets: u64,
    pub dropped_packets: u64,
}

pub struct SensorPipeline {
    config: PipelineConfig,
    parser: ParserDispatch,
    distance_filter: DistanceFilter,
    auto_calibrator: Option<AutoCalibrator>,
    manual_correction: Option<EncoderCorrection>,
    counters: PipelineCounters,
}

impl SensorPipeline {
    pub fn new(config: PipelineConfig) -> Result<Self> {
        let manual_correction = match config.encoder_mode {
            EncoderMode::Manual { amplitude, phase } => {
                Some(EncoderCorrection::new(amplitude, phase)?)
            }
            _ => None,
        };
        let auto_calibrator = match config.encoder_mode {
            EncoderMode::Automatic => {
                let auto_config = AutoCalibrationConfig {
                    frame_rate: config.frame_rate,
                    ..AutoCalibrationConfig::default()
                };
                Some(AutoCalibrator::new(auto_config))
            }
            _ => None,
        };

        Ok(Self {
            parser: ParserDispatch::new(&config)?,
            distance_filter: DistanceFilter {
                min: config.min_distance,
                max: config.max_distance,
            },
            config,
            auto_calibrator,
            manual_correction,
            counters: PipelineCounters::default(),
        })
    }

    pub fn reset_calibration(&mut self) {
        if let Some(calibrator) = &mut self.auto_calibrator {
            calibrator.reset();
        }
    }

    pub fn counters(&self) -> &PipelineCounters {
        &self.counters
    }

    pub fn process_raw(&mut self, packet: &RawPacket) -> Result<Vec<Frame<PointXyzir>>> {
        self.process_packet_bytes(&packet.bytes)
    }

    pub fn process_packet_bytes(&mut self, packet: &[u8]) -> Result<Vec<Frame<PointXyzir>>> {
        self.counters.packets_seen += 1;
        let frames = match self.parser.parse(packet) {
            Ok(frames) => frames,
            Err(error) if self.config.strict => return Err(error),
            Err(error) => {
                self.counters.bad_packets += 1;
                warn!(%error, "dropping bad packet");
                return Ok(Vec::new());
            }
        };

        let mut output = Vec::with_capacity(frames.len());
        for mut frame in frames {
            if let Some(calibrator) = &mut self.auto_calibrator {
                if let Some(correction) = calibrator.process_frame(&frame)? {
                    apply_correction(&mut frame, correction);
                } else {
                    continue;
                }
            } else if let Some(correction) = self.manual_correction {
                apply_correction(&mut frame, correction);
            }

            self.distance_filter.apply(&mut frame);
            self.config.ring_filter.apply(&mut frame);
            let xyz = frame.to_xyzir();
            self.counters.frames_emitted += 1;
            output.push(xyz);
        }

        Ok(output)
    }
}

struct ParserDispatch {
    m_series: MSeriesParser,
}

impl ParserDispatch {
    fn new(config: &PipelineConfig) -> Result<Self> {
        Ok(Self {
            m_series: MSeriesParser::new(config)?,
        })
    }

    fn parse(&mut self, packet: &[u8]) -> Result<Vec<Frame<PointHvdir>>> {
        let header = PacketHeader::parse(packet)?;
        match header.packet_type {
            PACKET_TYPE_M_SERIES => self.m_series.parse_00(packet, header),
            PACKET_TYPE_HVDIR_LIST => parse_01(packet, header, &self.m_series.frame_id),
            PACKET_TYPE_M_SERIES_REDUCED => self.m_series.parse_04(packet, header),
            PACKET_TYPE_M1 => self.m_series.parse_06(packet, header),
            other => Err(QuanergyError::UnsupportedPacketType(other)),
        }
    }
}

struct MSeriesParser {
    frame_id: String,
    horizontal_lut: Vec<f32>,
    vertical_angles: Vec<f32>,
    return_selection: ReturnSelection,
    return_selection_set: bool,
    min_cloud_size: usize,
    max_cloud_size: usize,
    angle_per_cloud: f32,
    cloud_counter: u64,
    start_azimuth: Option<f32>,
    last_azimuth: f32,
    direction: i32,
    current_packet_stamp_micros: u64,
    previous_packet_stamp_micros: u64,
    firing_number: usize,
    current_cloud: Frame<PointHvdir>,
}

impl MSeriesParser {
    fn new(config: &PipelineConfig) -> Result<Self> {
        let vertical_angles = config.vertical_angles.clone().ok_or_else(|| {
            QuanergyError::InvalidVerticalAngles(
                "M-Series parser requires vertical angles from deviceInfo, sidecar, or defaults"
                    .to_owned(),
            )
        })?;
        if vertical_angles.len() != M_SERIES_NUM_LASERS {
            return Err(QuanergyError::InvalidVerticalAngles(format!(
                "expected {M_SERIES_NUM_LASERS} vertical angles, got {}",
                vertical_angles.len()
            )));
        }

        Ok(Self {
            frame_id: config.frame_id.clone(),
            horizontal_lut: horizontal_angle_lut(),
            vertical_angles,
            return_selection: config.return_selection,
            return_selection_set: config.return_selection_set,
            min_cloud_size: config.min_cloud_size.max(1),
            max_cloud_size: config.max_cloud_size.max(config.min_cloud_size.max(1)),
            angle_per_cloud: std::f32::consts::TAU,
            cloud_counter: 0,
            start_azimuth: None,
            last_azimuth: 65_000.0,
            direction: 1,
            current_packet_stamp_micros: 0,
            previous_packet_stamp_micros: 0,
            firing_number: 0,
            current_cloud: Frame::new(config.frame_id.clone()),
        })
    }

    fn parse_00(&mut self, packet: &[u8], header: PacketHeader) -> Result<Vec<Frame<PointHvdir>>> {
        header.require_version(0, 1, 0)?;
        const FIRING_LEN: usize = 132;
        let expected = HEADER_LEN + M_SERIES_FIRINGS_PER_PACKET * FIRING_LEN + 12;
        ensure_packet_len(packet, expected)?;

        let mut cursor = Cursor::new(&packet[HEADER_LEN..]);
        let mut firings = Vec::with_capacity(M_SERIES_FIRINGS_PER_PACKET);
        for _ in 0..M_SERIES_FIRINGS_PER_PACKET {
            let position = cursor.read_u16::<BigEndian>()?;
            let _padding = cursor.read_u16::<BigEndian>()?;
            let mut distances = [[0u32; M_SERIES_NUM_LASERS]; M_SERIES_NUM_RETURNS];
            for return_index in distances.iter_mut() {
                for distance in return_index.iter_mut() {
                    *distance = cursor.read_u32::<BigEndian>()?;
                }
            }
            let mut intensities = [[0u8; M_SERIES_NUM_LASERS]; M_SERIES_NUM_RETURNS];
            for return_index in intensities.iter_mut() {
                for intensity in return_index.iter_mut() {
                    *intensity = cursor.read_u8()?;
                }
            }
            let mut status = [0u8; M_SERIES_NUM_LASERS];
            cursor.read_exact(&mut status)?;
            firings.push(MSeriesFiring {
                position,
                distances,
                intensities,
            });
        }

        let seconds = cursor.read_u32::<BigEndian>()?;
        let nanoseconds = cursor.read_u32::<BigEndian>()?;
        let api_version = cursor.read_u16::<BigEndian>()?;
        let status = cursor.read_u16::<BigEndian>()?;
        validate_status(status)?;

        let stamp = if api_version <= 3 && api_version != 0 {
            seconds as u64 * 1_000_000 + nanoseconds as u64 / 100
        } else {
            seconds as u64 * 1_000_000 + nanoseconds as u64 / 1_000
        };
        self.register_packet(stamp, &firings);
        let scale = if api_version >= 5 { 0.000_01 } else { 0.01 };

        let mut output = Vec::new();
        for firing in &firings {
            let h = self.h_angle(firing.position)?;
            let mut firing_points = Vec::new();
            for laser in 0..M_SERIES_NUM_LASERS {
                let base = PointHvdir {
                    h,
                    v: self.vertical_angles[laser],
                    d: f32::NAN,
                    intensity: 0.0,
                    ring: laser as u16,
                };
                match self.return_selection {
                    ReturnSelection::All => push_all_returns(
                        &mut firing_points,
                        base,
                        &firing.distances,
                        &firing.intensities,
                        laser,
                        scale,
                    ),
                    ReturnSelection::Single(return_index) => {
                        let distance = firing.distances[return_index as usize][laser];
                        firing_points.push(point_for_distance(
                            base,
                            distance,
                            firing.intensities[return_index as usize][laser],
                            scale,
                        ));
                    }
                }
            }
            if let Some(frame) = self.check_complete(h) {
                if self.return_selection == ReturnSelection::All {
                    output.push(frame);
                } else {
                    output.push(organize_cloud(frame, M_SERIES_NUM_LASERS)?);
                }
            }
            self.add_firing(firing_points);
        }
        Ok(output)
    }

    fn parse_04(&mut self, packet: &[u8], header: PacketHeader) -> Result<Vec<Frame<PointHvdir>>> {
        header.require_version(0, 1, 0)?;
        const FIRING_LEN: usize = 44;
        let expected = HEADER_LEN + 4 + M_SERIES_FIRINGS_PER_PACKET * FIRING_LEN;
        ensure_packet_len(packet, expected)?;

        let mut cursor = Cursor::new(&packet[HEADER_LEN..]);
        let status = cursor.read_u16::<BigEndian>()?;
        validate_status(status)?;
        let return_id = cursor.read_u8()?;
        let _reserved = cursor.read_u8()?;
        if self.return_selection_set {
            if let ReturnSelection::Single(requested) = self.return_selection {
                if requested != return_id {
                    return Err(QuanergyError::ReturnIdMismatch {
                        requested,
                        actual: return_id,
                    });
                }
            }
        }

        let mut firings = Vec::with_capacity(M_SERIES_FIRINGS_PER_PACKET);
        for _ in 0..M_SERIES_FIRINGS_PER_PACKET {
            let position = cursor.read_u16::<BigEndian>()?;
            let _reserved = cursor.read_u16::<BigEndian>()?;
            let mut distances = [[0u32; M_SERIES_NUM_LASERS]; M_SERIES_NUM_RETURNS];
            for distance in distances[0].iter_mut().take(M_SERIES_NUM_LASERS) {
                *distance = cursor.read_u32::<BigEndian>()?;
            }
            let mut intensities = [[0u8; M_SERIES_NUM_LASERS]; M_SERIES_NUM_RETURNS];
            for intensity in intensities[0].iter_mut().take(M_SERIES_NUM_LASERS) {
                *intensity = cursor.read_u8()?;
            }
            firings.push(MSeriesFiring {
                position,
                distances,
                intensities,
            });
        }

        self.register_packet(header.timestamp_micros(), &firings);
        let mut output = Vec::new();
        for firing in &firings {
            let h = self.h_angle(firing.position)?;
            let mut firing_points = Vec::with_capacity(M_SERIES_NUM_LASERS);
            for laser in 0..M_SERIES_NUM_LASERS {
                let base = PointHvdir {
                    h,
                    v: self.vertical_angles[laser],
                    d: f32::NAN,
                    intensity: 0.0,
                    ring: laser as u16,
                };
                firing_points.push(point_for_distance(
                    base,
                    firing.distances[0][laser],
                    firing.intensities[0][laser],
                    0.000_01,
                ));
            }
            if let Some(frame) = self.check_complete(h) {
                output.push(organize_cloud(frame, M_SERIES_NUM_LASERS)?);
            }
            self.add_firing(firing_points);
        }
        Ok(output)
    }

    fn parse_06(&mut self, packet: &[u8], header: PacketHeader) -> Result<Vec<Frame<PointHvdir>>> {
        header.require_version(0, 1, 0)?;
        if packet.len() < HEADER_LEN + 4 {
            return Err(QuanergyError::PacketTooShort {
                actual: packet.len(),
                minimum: HEADER_LEN + 4,
            });
        }

        let mut cursor = Cursor::new(&packet[HEADER_LEN..]);
        let status = cursor.read_u16::<BigEndian>()?;
        validate_status(status)?;
        let return_id = cursor.read_u8()?;
        let _reserved = cursor.read_u8()?;
        let returns_per_firing = if return_id == 3 { 3 } else { 1 };
        let firing_len = if returns_per_firing == 3 { 20 } else { 12 };
        let expected = HEADER_LEN + 4 + M_SERIES_FIRINGS_PER_PACKET * firing_len;
        ensure_packet_len(packet, expected)?;
        if returns_per_firing == 1 && self.return_selection_set {
            if let ReturnSelection::Single(requested) = self.return_selection {
                if requested != return_id {
                    return Err(QuanergyError::ReturnIdMismatch {
                        requested,
                        actual: return_id,
                    });
                }
            }
        }

        let mut firings = Vec::with_capacity(M_SERIES_FIRINGS_PER_PACKET);
        for _ in 0..M_SERIES_FIRINGS_PER_PACKET {
            let position = cursor.read_u16::<BigEndian>()?;
            let _reserved = cursor.read_u16::<BigEndian>()?;
            let mut distances = [[0u32; M_SERIES_NUM_LASERS]; M_SERIES_NUM_RETURNS];
            let mut intensities = [[0u8; M_SERIES_NUM_LASERS]; M_SERIES_NUM_RETURNS];
            for return_distances in distances.iter_mut().take(returns_per_firing) {
                return_distances[0] = cursor.read_u32::<BigEndian>()?;
            }
            for return_intensities in intensities.iter_mut().take(returns_per_firing) {
                return_intensities[0] = cursor.read_u8()?;
            }
            let padding = 4 - returns_per_firing;
            let mut discard = vec![0; padding];
            cursor.read_exact(&mut discard)?;
            firings.push(MSeriesFiring {
                position,
                distances,
                intensities,
            });
        }

        self.register_packet(header.timestamp_micros(), &firings);
        let mut output = Vec::new();
        for firing in &firings {
            let h = self.h_angle(firing.position)?;
            let base = PointHvdir {
                h,
                v: 0.0,
                d: f32::NAN,
                intensity: 0.0,
                ring: 0,
            };
            let mut firing_points = Vec::new();
            if returns_per_firing == 3 && self.return_selection == ReturnSelection::All {
                push_all_returns(
                    &mut firing_points,
                    base,
                    &firing.distances,
                    &firing.intensities,
                    0,
                    0.000_01,
                );
            } else if returns_per_firing == 3 {
                let return_index = match self.return_selection {
                    ReturnSelection::Single(value) => value as usize,
                    ReturnSelection::All => 0,
                };
                firing_points.push(point_for_distance(
                    base,
                    firing.distances[return_index][0],
                    firing.intensities[return_index][0],
                    0.000_01,
                ));
            } else {
                firing_points.push(point_for_distance(
                    base,
                    firing.distances[0][0],
                    firing.intensities[0][0],
                    0.000_01,
                ));
            }

            if let Some(frame) = self.check_complete(h) {
                output.push(frame);
            }
            self.add_firing(firing_points);
        }
        Ok(output)
    }

    fn register_packet(&mut self, current_packet_stamp_micros: u64, firings: &[MSeriesFiring]) {
        self.previous_packet_stamp_micros = if self.current_packet_stamp_micros == 0 {
            current_packet_stamp_micros
        } else {
            self.current_packet_stamp_micros
        };
        self.current_packet_stamp_micros = current_packet_stamp_micros;

        let start = firings.first().map(|firing| firing.position).unwrap_or(0) as i32;
        let mid = firings
            .get(M_SERIES_FIRINGS_PER_PACKET / 2)
            .map(|firing| firing.position)
            .unwrap_or(0) as i32;
        let end = firings.last().map(|firing| firing.position).unwrap_or(0) as i32;

        if start - mid < 0 && mid - end < 0 {
            self.direction = 1;
        } else if start - mid > 0 && mid - end > 0 {
            self.direction = -1;
        }
        self.firing_number = 0;
    }

    fn h_angle(&self, position: u16) -> Result<f32> {
        self.horizontal_lut
            .get(position as usize)
            .copied()
            .ok_or_else(|| QuanergyError::Config(format!("invalid firing position {position}")))
    }

    fn check_complete(&mut self, azimuth_angle: f32) -> Option<Frame<PointHvdir>> {
        let cloud_full = self.current_cloud.points.len() >= self.max_cloud_size;
        let start = match self.start_azimuth {
            Some(start) => start,
            None => {
                self.start_azimuth = Some(azimuth_angle);
                self.last_azimuth = azimuth_angle;
                return None;
            }
        };

        let mut delta_angle = self.direction as f32 * (azimuth_angle - start);
        while delta_angle < 0.0 {
            delta_angle += std::f32::consts::TAU;
        }
        let wrapped = (self.angle_per_cloud - std::f32::consts::TAU).abs() < f32::EPSILON
            && self.direction as f32 * azimuth_angle < self.direction as f32 * self.last_azimuth;

        if delta_angle >= self.angle_per_cloud || wrapped {
            self.start_azimuth = Some(azimuth_angle);
            let result = if self.current_cloud.points.len() > self.min_cloud_size {
                if cloud_full {
                    warn!(limit = self.max_cloud_size, "maximum cloud size exceeded");
                }
                let mut complete =
                    std::mem::replace(&mut self.current_cloud, Frame::new(self.frame_id.clone()));
                let time_since_previous_packet = (self.current_packet_stamp_micros
                    - self.previous_packet_stamp_micros)
                    * self.firing_number as u64
                    / M_SERIES_FIRINGS_PER_PACKET as u64;
                complete.stamp_micros =
                    self.previous_packet_stamp_micros + time_since_previous_packet;
                complete.sequence = self.cloud_counter;
                complete.frame_id = self.frame_id.clone();
                complete.refresh_unorganized_dims();
                self.cloud_counter += 1;
                Some(complete)
            } else {
                if !self.current_cloud.points.is_empty() {
                    warn!(
                        min = self.min_cloud_size,
                        actual = self.current_cloud.points.len(),
                        "minimum cloud size not reached"
                    );
                }
                self.current_cloud = Frame::new(self.frame_id.clone());
                None
            };
            self.last_azimuth = azimuth_angle;
            result
        } else {
            self.last_azimuth = azimuth_angle;
            None
        }
    }

    fn add_firing(&mut self, firing_points: Vec<PointHvdir>) {
        if firing_points.is_empty() || self.current_cloud.points.len() >= self.max_cloud_size {
            return;
        }
        self.firing_number += 1;
        if firing_points.iter().any(|point| point.d.is_nan()) {
            self.current_cloud.is_dense = false;
        }
        self.current_cloud.points.extend(firing_points);
    }
}

#[derive(Debug, Clone)]
struct MSeriesFiring {
    position: u16,
    distances: [[u32; M_SERIES_NUM_LASERS]; M_SERIES_NUM_RETURNS],
    intensities: [[u8; M_SERIES_NUM_LASERS]; M_SERIES_NUM_RETURNS],
}

fn parse_01(packet: &[u8], header: PacketHeader, frame_id: &str) -> Result<Vec<Frame<PointHvdir>>> {
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

fn point_for_distance(base: PointHvdir, distance: u32, intensity: u8, scale: f32) -> PointHvdir {
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

fn push_all_returns(
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

fn organize_cloud(mut frame: Frame<PointHvdir>, height: usize) -> Result<Frame<PointHvdir>> {
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

fn validate_status(status: u16) -> Result<()> {
    if status & 0b11 != 0 {
        Err(QuanergyError::InvalidSensorStatus(status))
    } else {
        Ok(())
    }
}

fn ensure_packet_len(packet: &[u8], expected: usize) -> Result<()> {
    if packet.len() == expected {
        Ok(())
    } else {
        Err(QuanergyError::PacketSizeMismatch {
            expected,
            actual: packet.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use byteorder::{BigEndian, WriteBytesExt};

    use super::*;
    use crate::protocol::{PacketHeader, SIGNATURE};

    #[test]
    fn packet_01_parses_hvdir_points() {
        let mut body = Vec::new();
        body.write_u32::<BigEndian>(7).unwrap();
        body.write_u32::<BigEndian>(0).unwrap();
        body.write_u32::<BigEndian>(1).unwrap();
        body.write_u32::<BigEndian>(0).unwrap();
        body.write_i16::<BigEndian>(0).unwrap();
        body.write_i16::<BigEndian>(0).unwrap();
        body.write_u32::<BigEndian>(2_000_000).unwrap();
        body.write_u16::<BigEndian>(42).unwrap();
        body.write_u8(0).unwrap();
        body.write_u8(0).unwrap();

        let mut packet = Vec::new();
        PacketHeader {
            signature: SIGNATURE,
            size: (HEADER_LEN + body.len()) as u32,
            seconds: 10,
            nanoseconds: 20_000,
            version_major: 0,
            version_minor: 1,
            version_patch: 0,
            packet_type: PACKET_TYPE_HVDIR_LIST,
        }
        .write_to(&mut packet)
        .unwrap();
        packet.extend(body);

        let mut pipeline = SensorPipeline::new(PipelineConfig::default()).unwrap();
        let frames = pipeline.process_packet_bytes(&packet).unwrap();
        assert_eq!(frames.len(), 1);
        assert!((frames[0].points[0].x - 2.0).abs() < 1e-6);
        assert_eq!(frames[0].points[0].intensity, 42.0);
    }

    #[test]
    fn packet_00_parses_m_series_frame_on_wrap() {
        let mut pipeline = SensorPipeline::new(PipelineConfig::default()).unwrap();
        assert!(pipeline
            .process_packet_bytes(&packet_00_with_positions(5150))
            .unwrap()
            .is_empty());

        let frames = pipeline
            .process_packet_bytes(&packet_00_with_positions(5200))
            .unwrap();

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].height, M_SERIES_NUM_LASERS);
        assert_eq!(frames[0].width, M_SERIES_FIRINGS_PER_PACKET);
        assert_eq!(
            frames[0].points.len(),
            M_SERIES_FIRINGS_PER_PACKET * M_SERIES_NUM_LASERS
        );
        assert!(frames[0].points.iter().any(|point| point.intensity > 0.0));
    }

    #[test]
    fn packet_04_parses_reduced_m_series_frame_on_wrap() {
        let mut pipeline = SensorPipeline::new(PipelineConfig::default()).unwrap();
        assert!(pipeline
            .process_packet_bytes(&packet_04_with_positions(5150))
            .unwrap()
            .is_empty());

        let frames = pipeline
            .process_packet_bytes(&packet_04_with_positions(5200))
            .unwrap();

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].height, M_SERIES_NUM_LASERS);
        assert_eq!(frames[0].width, M_SERIES_FIRINGS_PER_PACKET);
        assert_eq!(
            frames[0].points.len(),
            M_SERIES_FIRINGS_PER_PACKET * M_SERIES_NUM_LASERS
        );
    }

    #[test]
    fn packet_06_parses_m1_frame_on_wrap() {
        let mut pipeline = SensorPipeline::new(PipelineConfig::default()).unwrap();
        assert!(pipeline
            .process_packet_bytes(&packet_06_with_positions(5150))
            .unwrap()
            .is_empty());

        let frames = pipeline
            .process_packet_bytes(&packet_06_with_positions(5200))
            .unwrap();

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].height, 1);
        assert_eq!(frames[0].width, M_SERIES_FIRINGS_PER_PACKET);
        assert_eq!(frames[0].points.len(), M_SERIES_FIRINGS_PER_PACKET);
    }

    #[test]
    fn invalid_packet_is_dropped_when_lenient() {
        let mut pipeline = SensorPipeline::new(PipelineConfig::default()).unwrap();
        let frames = pipeline.process_packet_bytes(&[0; HEADER_LEN]).unwrap();

        assert!(frames.is_empty());
        assert_eq!(pipeline.counters().bad_packets, 1);
    }

    #[test]
    fn invalid_packet_fails_when_strict() {
        let mut pipeline = SensorPipeline::new(PipelineConfig {
            strict: true,
            ..PipelineConfig::default()
        })
        .unwrap();

        assert!(pipeline.process_packet_bytes(&[0; HEADER_LEN]).is_err());
    }

    fn packet_00_with_positions(start_position: u16) -> Vec<u8> {
        let mut body = Vec::new();
        for firing in 0..M_SERIES_FIRINGS_PER_PACKET {
            body.write_u16::<BigEndian>(start_position + firing as u16)
                .unwrap();
            body.write_u16::<BigEndian>(0).unwrap();
            for return_index in 0..M_SERIES_NUM_RETURNS {
                for laser in 0..M_SERIES_NUM_LASERS {
                    let distance = if return_index == 0 {
                        200_000 + firing as u32 * 100 + laser as u32
                    } else {
                        0
                    };
                    body.write_u32::<BigEndian>(distance).unwrap();
                }
            }
            for return_index in 0..M_SERIES_NUM_RETURNS {
                for laser in 0..M_SERIES_NUM_LASERS {
                    let intensity = if return_index == 0 {
                        10 + laser as u8
                    } else {
                        0
                    };
                    body.write_u8(intensity).unwrap();
                }
            }
            body.extend([0; M_SERIES_NUM_LASERS]);
        }
        body.write_u32::<BigEndian>(1).unwrap();
        body.write_u32::<BigEndian>(2_000).unwrap();
        body.write_u16::<BigEndian>(5).unwrap();
        body.write_u16::<BigEndian>(0).unwrap();
        packet_with_header(PACKET_TYPE_M_SERIES, body)
    }

    fn packet_04_with_positions(start_position: u16) -> Vec<u8> {
        let mut body = Vec::new();
        body.write_u16::<BigEndian>(0).unwrap();
        body.write_u8(0).unwrap();
        body.write_u8(0).unwrap();
        for firing in 0..M_SERIES_FIRINGS_PER_PACKET {
            body.write_u16::<BigEndian>(start_position + firing as u16)
                .unwrap();
            body.write_u16::<BigEndian>(0).unwrap();
            for laser in 0..M_SERIES_NUM_LASERS {
                body.write_u32::<BigEndian>(200_000 + firing as u32 * 100 + laser as u32)
                    .unwrap();
            }
            for laser in 0..M_SERIES_NUM_LASERS {
                body.write_u8(20 + laser as u8).unwrap();
            }
        }
        packet_with_header(PACKET_TYPE_M_SERIES_REDUCED, body)
    }

    fn packet_06_with_positions(start_position: u16) -> Vec<u8> {
        let mut body = Vec::new();
        body.write_u16::<BigEndian>(0).unwrap();
        body.write_u8(0).unwrap();
        body.write_u8(0).unwrap();
        for firing in 0..M_SERIES_FIRINGS_PER_PACKET {
            body.write_u16::<BigEndian>(start_position + firing as u16)
                .unwrap();
            body.write_u16::<BigEndian>(0).unwrap();
            body.write_u32::<BigEndian>(200_000 + firing as u32 * 100)
                .unwrap();
            body.write_u8(30).unwrap();
            body.extend([0; 3]);
        }
        packet_with_header(PACKET_TYPE_M1, body)
    }

    fn packet_with_header(packet_type: u8, body: Vec<u8>) -> Vec<u8> {
        let mut packet = Vec::new();
        PacketHeader {
            signature: SIGNATURE,
            size: (HEADER_LEN + body.len()) as u32,
            seconds: 10,
            nanoseconds: 20_000,
            version_major: 0,
            version_minor: 1,
            version_patch: 0,
            packet_type,
        }
        .write_to(&mut packet)
        .unwrap();
        packet.extend(body);
        packet
    }
}
