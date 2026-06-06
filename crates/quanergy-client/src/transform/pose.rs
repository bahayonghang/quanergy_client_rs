use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{CoordinateTransform, Matrix4Transform, TransformSnapshot};
use crate::cloud::PointXyzir;

pub const YAW_PITCH_ROLL_ALGORITHM: &str = "yaw_pitch_roll_pose";

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct YawPitchRollPose {
    pub x_m: f32,
    pub y_m: f32,
    pub z_m: f32,
    pub yaw_deg: f32,
    pub pitch_deg: f32,
    pub roll_deg: f32,
}

impl Default for YawPitchRollPose {
    fn default() -> Self {
        Self {
            x_m: 0.0,
            y_m: 0.0,
            z_m: 0.0,
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            roll_deg: 0.0,
        }
    }
}

impl YawPitchRollPose {
    pub fn to_transform(self) -> YawPitchRollTransform {
        YawPitchRollTransform::new(self)
    }

    pub fn matrix_4x4(self) -> [[f32; 4]; 4] {
        let yaw = self.yaw_deg.to_radians();
        let pitch = self.pitch_deg.to_radians();
        let roll = self.roll_deg.to_radians();

        let (sy, cy) = yaw.sin_cos();
        let (sp, cp) = pitch.sin_cos();
        let (sr, cr) = roll.sin_cos();

        [
            [
                cy * cp,
                cy * sp * sr - sy * cr,
                cy * sp * cr + sy * sr,
                self.x_m,
            ],
            [
                sy * cp,
                sy * sp * sr + cy * cr,
                sy * sp * cr - cy * sr,
                self.y_m,
            ],
            [-sp, cp * sr, cp * cr, self.z_m],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct YawPitchRollTransform {
    pose: YawPitchRollPose,
    matrix: Matrix4Transform,
}

impl YawPitchRollTransform {
    pub fn new(pose: YawPitchRollPose) -> Self {
        let config = json!({
            "x_m": pose.x_m,
            "y_m": pose.y_m,
            "z_m": pose.z_m,
            "yaw_deg": pose.yaw_deg,
            "pitch_deg": pose.pitch_deg,
            "roll_deg": pose.roll_deg,
        });
        Self {
            pose,
            matrix: Matrix4Transform::with_config(
                YAW_PITCH_ROLL_ALGORITHM,
                pose.matrix_4x4(),
                config,
            ),
        }
    }

    pub fn pose(&self) -> YawPitchRollPose {
        self.pose
    }
}

impl CoordinateTransform for YawPitchRollTransform {
    fn name(&self) -> &str {
        self.matrix.name()
    }

    fn transform_point(&self, point: PointXyzir) -> PointXyzir {
        self.matrix.transform_point(point)
    }

    fn matrix_4x4(&self) -> [[f32; 4]; 4] {
        self.matrix.matrix_4x4()
    }

    fn snapshot(&self) -> TransformSnapshot {
        self.matrix.snapshot()
    }
}
