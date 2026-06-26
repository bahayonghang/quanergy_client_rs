//! Station coordinate transforms for fixed tamping-station installations.
//!
//! Station coordinates are meters from the field-chosen lower-left station
//! origin: X points right, Y points deeper/upward in the station drawing, and Z
//! points vertically upward.

mod matrix;
mod pose;
mod validation;

#[cfg(test)]
mod tests;

pub use matrix::Matrix4Transform;
pub use pose::{YawPitchRollPose, YawPitchRollTransform};
pub use validation::{validate_rigid_matrix, RigidMatrixError, RigidTransformValidation};

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

// ---------------------------------------------------------------------------
// StationTransform — fixed extrinsic with frame semantics
// ---------------------------------------------------------------------------

/// A fixed rigid-body transform from a source frame to a target frame.
///
/// Unlike the generic [`CoordinateTransform`] / [`apply_transform`] path
/// (which preserves the input `frame_id`), `StationTransform` explicitly
/// sets the output `frame_id` to the target frame.
///
/// This is the preferred path for station-frame point clouds.
#[derive(Debug, Clone)]
pub struct StationTransform {
    source_frame: String,
    target_frame: String,
    transform_id: String,
    matrix: Matrix4Transform,
}

impl StationTransform {
    /// Create a new station transform.
    ///
    /// The `matrix` must already have passed rigid-body validation
    /// (see [`validate_rigid_matrix`]).
    pub fn new(
        source_frame: impl Into<String>,
        target_frame: impl Into<String>,
        transform_id: impl Into<String>,
        matrix_4x4: [[f32; 4]; 4],
    ) -> Self {
        let source_frame = source_frame.into();
        let target_frame = target_frame.into();
        let transform_id = transform_id.into();
        let matrix = Matrix4Transform::new(
            format!("station_transform_{source_frame}_to_{target_frame}"),
            matrix_4x4,
        );

        Self {
            source_frame,
            target_frame,
            transform_id,
            matrix,
        }
    }

    /// Source coordinate frame name.
    pub fn source_frame(&self) -> &str {
        &self.source_frame
    }

    /// Target coordinate frame name.
    pub fn target_frame(&self) -> &str {
        &self.target_frame
    }

    /// External transform identifier (e.g. from station config).
    pub fn transform_id(&self) -> &str {
        &self.transform_id
    }

    /// The underlying 4×4 matrix.
    pub fn matrix_4x4(&self) -> [[f32; 4]; 4] {
        self.matrix.matrix_4x4()
    }

    /// Transform a single point.
    pub fn transform_point(&self, point: PointXyzir) -> PointXyzir {
        self.matrix.transform_point(point)
    }

    /// Transform a frame and set its `frame_id` to `target_frame`.
    ///
    /// All XYZ coordinates are transformed by the 4×4 matrix.
    /// Intensity and ring are preserved unchanged.
    /// Timestamp, sequence, width, height, and `is_dense` are preserved.
    /// The output `frame_id` is set to `self.target_frame`.
    pub fn transform_frame_to_target(&self, frame: &Frame<PointXyzir>) -> Frame<PointXyzir> {
        Frame {
            stamp_micros: frame.stamp_micros,
            sequence: frame.sequence,
            frame_id: self.target_frame.clone(),
            width: frame.width,
            height: frame.height,
            is_dense: frame.is_dense,
            points: frame
                .points
                .iter()
                .copied()
                .map(|p| self.transform_point(p))
                .collect(),
        }
    }

    /// Produce a [`TransformSnapshot`] with frame provenance in its config.
    pub fn snapshot(&self) -> TransformSnapshot {
        let mut snapshot = self.matrix.snapshot();
        snapshot.config["source_frame"] = serde_json::Value::String(self.source_frame.clone());
        snapshot.config["target_frame"] = serde_json::Value::String(self.target_frame.clone());
        snapshot.config["transform_id"] = serde_json::Value::String(self.transform_id.clone());
        snapshot
    }

    /// Consume self and return the inner [`Matrix4Transform`].
    pub fn into_inner(self) -> Matrix4Transform {
        self.matrix
    }
}
