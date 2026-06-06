//! Station coordinate transforms for fixed tamping-station installations.
//!
//! Station coordinates are meters from the field-chosen lower-left station
//! origin: X points right, Y points deeper/upward in the station drawing, and Z
//! points vertically upward.

mod matrix;
mod pose;

#[cfg(test)]
mod tests;

pub use matrix::Matrix4Transform;
pub use pose::{YawPitchRollPose, YawPitchRollTransform};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cloud::{Frame, PointXyzir};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransformSnapshot {
    pub algorithm: String,
    pub matrix_4x4: [[f32; 4]; 4],
    #[serde(default)]
    pub config: Value,
}

pub trait CoordinateTransform {
    fn name(&self) -> &str;

    fn transform_point(&self, point: PointXyzir) -> PointXyzir;

    fn matrix_4x4(&self) -> [[f32; 4]; 4];

    fn snapshot(&self) -> TransformSnapshot;

    fn transform_frame(&self, frame: &Frame<PointXyzir>) -> Frame<PointXyzir> {
        apply_transform(frame, self)
    }
}

pub fn apply_transform<T>(frame: &Frame<PointXyzir>, transform: &T) -> Frame<PointXyzir>
where
    T: CoordinateTransform + ?Sized,
{
    Frame {
        stamp_micros: frame.stamp_micros,
        sequence: frame.sequence,
        frame_id: frame.frame_id.clone(),
        width: frame.width,
        height: frame.height,
        is_dense: frame.is_dense,
        points: frame
            .points
            .iter()
            .copied()
            .map(|point| transform.transform_point(point))
            .collect(),
    }
}
