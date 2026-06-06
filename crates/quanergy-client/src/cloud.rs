use std::f32::consts::PI;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame<T> {
    pub stamp_micros: u64,
    pub sequence: u64,
    pub frame_id: String,
    pub width: usize,
    pub height: usize,
    pub is_dense: bool,
    pub points: Vec<T>,
}

impl<T> Frame<T> {
    pub fn new(frame_id: impl Into<String>) -> Self {
        Self {
            stamp_micros: 0,
            sequence: 0,
            frame_id: frame_id.into(),
            width: 0,
            height: 1,
            is_dense: true,
            points: Vec::new(),
        }
    }

    pub fn with_points(frame_id: impl Into<String>, points: Vec<T>) -> Self {
        let width = points.len();
        Self {
            stamp_micros: 0,
            sequence: 0,
            frame_id: frame_id.into(),
            width,
            height: 1,
            is_dense: true,
            points,
        }
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    pub fn refresh_unorganized_dims(&mut self) {
        self.height = 1;
        self.width = self.points.len();
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointHvdir {
    pub h: f32,
    pub v: f32,
    pub d: f32,
    pub intensity: f32,
    pub ring: u16,
}

impl PointHvdir {
    pub fn corrected(self, correction: crate::calibration::EncoderCorrection) -> Self {
        let mut h = self.h + correction.zero_offset() - correction.offset(self.h);
        if h < -PI {
            h += 2.0 * PI;
        } else if h > PI {
            h -= 2.0 * PI;
        }

        Self { h, ..self }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointXyzir {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub intensity: f32,
    pub ring: u16,
}

impl From<PointHvdir> for PointXyzir {
    fn from(point: PointHvdir) -> Self {
        if point.d.is_nan() {
            return Self {
                x: f32::NAN,
                y: f32::NAN,
                z: f32::NAN,
                intensity: point.intensity,
                ring: point.ring,
            };
        }

        let xy = point.d * point.v.cos();
        Self {
            x: xy * point.h.cos(),
            y: xy * point.h.sin(),
            z: point.d * point.v.sin(),
            intensity: point.intensity,
            ring: point.ring,
        }
    }
}

impl Frame<PointHvdir> {
    pub fn to_xyzir(&self) -> Frame<PointXyzir> {
        Frame {
            stamp_micros: self.stamp_micros,
            sequence: self.sequence,
            frame_id: self.frame_id.clone(),
            width: self.width,
            height: self.height,
            is_dense: self.is_dense,
            points: self.points.iter().copied().map(PointXyzir::from).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hvdir_to_xyzir_preserves_nan_distance() {
        let xyz = PointXyzir::from(PointHvdir {
            h: 0.0,
            v: 0.0,
            d: f32::NAN,
            intensity: 42.0,
            ring: 3,
        });

        assert!(xyz.x.is_nan());
        assert!(xyz.y.is_nan());
        assert!(xyz.z.is_nan());
        assert_eq!(xyz.intensity, 42.0);
        assert_eq!(xyz.ring, 3);
    }

    #[test]
    fn hvdir_to_xyzir_converts_known_axes() {
        let xyz = PointXyzir::from(PointHvdir {
            h: 0.0,
            v: 0.0,
            d: 2.0,
            intensity: 1.0,
            ring: 0,
        });

        assert!((xyz.x - 2.0).abs() < 1e-6);
        assert!(xyz.y.abs() < 1e-6);
        assert!(xyz.z.abs() < 1e-6);
    }
}
