use std::io::{Cursor, Read};

use byteorder::{BigEndian, ReadBytesExt};
use tracing::warn;

use crate::{
    cloud::{Frame, PointHvdir},
    config::PipelineConfig,
    error::{QuanergyError, Result},
    protocol::{
        horizontal_angle_lut, PacketHeader, ReturnSelection, HEADER_LEN,
        M_SERIES_FIRINGS_PER_PACKET, M_SERIES_NUM_LASERS, M_SERIES_NUM_RETURNS,
    },
};

use super::helpers::{
    ensure_packet_len, organize_cloud, point_for_distance, push_all_returns, validate_status,
};

pub(super) struct MSeriesParser {
    pub(super) frame_id: String,
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
    pub(super) fn new(config: &PipelineConfig) -> Result<Self> {
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

    pub(super) fn parse_00(
        &mut self,
        packet: &[u8],
        header: PacketHeader,
    ) -> Result<Vec<Frame<PointHvdir>>> {
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

    pub(super) fn parse_04(
        &mut self,
        packet: &[u8],
        header: PacketHeader,
    ) -> Result<Vec<Frame<PointHvdir>>> {
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

    pub(super) fn parse_06(
        &mut self,
        packet: &[u8],
        header: PacketHeader,
    ) -> Result<Vec<Frame<PointHvdir>>> {
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
