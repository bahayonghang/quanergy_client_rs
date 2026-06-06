use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::cloud::PointXyzir;

use super::{CoordinateTransform, TransformSnapshot};

pub const IDENTITY_4X4: [[f32; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Matrix4Transform {
    name: String,
    matrix: [[f32; 4]; 4],
    config: serde_json::Value,
}

impl Matrix4Transform {
    pub fn new(name: impl Into<String>, matrix: [[f32; 4]; 4]) -> Self {
        Self {
            name: name.into(),
            matrix,
            config: json!({}),
        }
    }

    pub fn with_config(
        name: impl Into<String>,
        matrix: [[f32; 4]; 4],
        config: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            matrix,
            config,
        }
    }

    pub fn identity() -> Self {
        Self::new("identity_matrix", IDENTITY_4X4)
    }

    pub fn matrix(&self) -> [[f32; 4]; 4] {
        self.matrix
    }
}

impl CoordinateTransform for Matrix4Transform {
    fn name(&self) -> &str {
        &self.name
    }

    fn transform_point(&self, point: PointXyzir) -> PointXyzir {
        let x = self.matrix[0][0] * point.x
            + self.matrix[0][1] * point.y
            + self.matrix[0][2] * point.z
            + self.matrix[0][3];
        let y = self.matrix[1][0] * point.x
            + self.matrix[1][1] * point.y
            + self.matrix[1][2] * point.z
            + self.matrix[1][3];
        let z = self.matrix[2][0] * point.x
            + self.matrix[2][1] * point.y
            + self.matrix[2][2] * point.z
            + self.matrix[2][3];

        PointXyzir {
            x,
            y,
            z,
            intensity: point.intensity,
            ring: point.ring,
        }
    }

    fn matrix_4x4(&self) -> [[f32; 4]; 4] {
        self.matrix
    }

    fn snapshot(&self) -> TransformSnapshot {
        TransformSnapshot {
            algorithm: self.name.clone(),
            matrix_4x4: self.matrix,
            config: self.config.clone(),
        }
    }
}
