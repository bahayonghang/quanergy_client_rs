//! Rigid-body matrix validation for station extrinsics.
//!
//! Every 4×4 extrinsic matrix loaded from a station config must pass
//! these checks before it is used to transform point clouds.

/// Tolerance configuration for rigid-transform validation.
#[derive(Debug, Clone)]
pub struct RigidTransformValidation {
    /// Maximum allowed deviation from I for RᵀR (Frobenius-like check per element).
    pub orthonormal_tolerance: f32,
    /// Maximum allowed deviation of det(R) from +1.
    pub determinant_tolerance: f32,
    /// Maximum allowed deviation of the last row from [0, 0, 0, 1].
    pub last_row_tolerance: f32,
}

impl Default for RigidTransformValidation {
    fn default() -> Self {
        Self {
            orthonormal_tolerance: 1e-4,
            determinant_tolerance: 1e-4,
            last_row_tolerance: 1e-6,
        }
    }
}

/// Errors returned by [`validate_rigid_matrix`].
#[derive(Debug, Clone, PartialEq)]
pub enum RigidMatrixError {
    NonFiniteElement {
        row: usize,
        col: usize,
    },
    NotOrthonormal {
        row: usize,
        col: usize,
        value: f32,
        expected: f32,
    },
    DeterminantNotOne {
        det: f32,
    },
    InvalidLastRow {
        row: usize,
        col: usize,
        value: f32,
    },
}

impl std::fmt::Display for RigidMatrixError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonFiniteElement { row, col } => {
                write!(f, "matrix element [{row}][{col}] is non-finite")
            }
            Self::NotOrthonormal {
                row,
                col,
                value,
                expected,
            } => {
                write!(
                    f,
                    "RᵀR[{row}][{col}] = {value}, expected {expected} (not orthonormal)"
                )
            }
            Self::DeterminantNotOne { det } => {
                write!(f, "det(R) = {det}, expected +1 (mirror or scale detected)")
            }
            Self::InvalidLastRow { row, col, value } => {
                write!(
                    f,
                    "last row [{row}][{col}] = {value}, expected {} (should be [0,0,0,1])",
                    if *row == 3 && *col == 3 { 1.0 } else { 0.0 }
                )
            }
        }
    }
}

impl std::error::Error for RigidMatrixError {}

/// Validate that a 4×4 matrix is a proper rigid-body transformation.
///
/// Checks:
/// 1. All elements are finite.
/// 2. The 3×3 rotation block is orthonormal (RᵀR ≈ I).
/// 3. det(R) ≈ +1 (no mirror / reflection).
/// 4. Last row is [0, 0, 0, 1].
///
/// Returns `Ok(())` on success, or the first encountered error.
pub fn validate_rigid_matrix(
    matrix: [[f32; 4]; 4],
    options: &RigidTransformValidation,
) -> Result<(), RigidMatrixError> {
    // 1. All elements finite
    for (i, row) in matrix.iter().enumerate() {
        for (j, &val) in row.iter().enumerate() {
            if !val.is_finite() {
                return Err(RigidMatrixError::NonFiniteElement { row: i, col: j });
            }
        }
    }

    // 2. Last row must be [0, 0, 0, 1]
    for (j, &val) in matrix[3].iter().enumerate() {
        let expected = if j == 3 { 1.0 } else { 0.0 };
        if (val - expected).abs() > options.last_row_tolerance {
            return Err(RigidMatrixError::InvalidLastRow {
                row: 3,
                col: j,
                value: val,
            });
        }
    }

    // 3. Orthonormality: compute RᵀR and compare to I₃
    for i in 0..3 {
        for j in 0..3 {
            let mut dot = 0.0f32;
            for row in matrix.iter().take(3) {
                dot += row[i] * row[j];
            }
            let expected = if i == j { 1.0 } else { 0.0 };
            if (dot - expected).abs() > options.orthonormal_tolerance {
                return Err(RigidMatrixError::NotOrthonormal {
                    row: i,
                    col: j,
                    value: dot,
                    expected,
                });
            }
        }
    }

    // 4. det(R) ≈ +1
    let det = matrix[0][0] * (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1])
        - matrix[0][1] * (matrix[1][0] * matrix[2][2] - matrix[1][2] * matrix[2][0])
        + matrix[0][2] * (matrix[1][0] * matrix[2][1] - matrix[1][1] * matrix[2][0]);

    if (det - 1.0).abs() > options.determinant_tolerance {
        return Err(RigidMatrixError::DeterminantNotOne { det });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_opts() -> RigidTransformValidation {
        RigidTransformValidation::default()
    }

    #[test]
    fn identity_passes() {
        let m = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        assert!(validate_rigid_matrix(m, &default_opts()).is_ok());
    }

    #[test]
    fn pure_translation_passes() {
        let m = [
            [1.0, 0.0, 0.0, 0.20],
            [0.0, 1.0, 0.0, 0.68],
            [0.0, 0.0, 1.0, 7.85],
            [0.0, 0.0, 0.0, 1.00],
        ];
        assert!(validate_rigid_matrix(m, &default_opts()).is_ok());
    }

    #[test]
    fn candidate_downward_matrix_passes() {
        // From plan §4.4: roll=180°, yaw=0°, pitch=0°
        let m = [
            [1.0, 0.0, 0.0, 0.20],
            [0.0, -1.0, 0.0, 0.68],
            [0.0, 0.0, -1.0, 7.85],
            [0.0, 0.0, 0.0, 1.00],
        ];
        assert!(validate_rigid_matrix(m, &default_opts()).is_ok());
    }

    #[test]
    fn nan_element_rejected() {
        let m = [
            [f32::NAN, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        assert!(matches!(
            validate_rigid_matrix(m, &default_opts()),
            Err(RigidMatrixError::NonFiniteElement { .. })
        ));
    }

    #[test]
    fn inf_element_rejected() {
        let m = [
            [1.0, 0.0, 0.0, f32::INFINITY],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        assert!(matches!(
            validate_rigid_matrix(m, &default_opts()),
            Err(RigidMatrixError::NonFiniteElement { .. })
        ));
    }

    #[test]
    fn mirror_det_negative_rejected() {
        // diag(1, 1, -1) — this is a reflection, det = -1
        let m = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, -1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        assert!(matches!(
            validate_rigid_matrix(m, &default_opts()),
            Err(RigidMatrixError::DeterminantNotOne { .. })
        ));
    }

    #[test]
    fn non_orthogonal_rejected() {
        // scale X by 2
        let m = [
            [2.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        assert!(matches!(
            validate_rigid_matrix(m, &default_opts()),
            Err(RigidMatrixError::NotOrthonormal { .. })
        ));
    }

    #[test]
    fn wrong_last_row_rejected() {
        let m = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0, 1.0], // [0,0,1,1] instead of [0,0,0,1]
        ];
        assert!(matches!(
            validate_rigid_matrix(m, &default_opts()),
            Err(RigidMatrixError::InvalidLastRow { .. })
        ));
    }

    #[test]
    fn candidate_downward_matrix_satisfies_axis_directions() {
        // §4.4: sensor +X → station +X, sensor +Y → station -Y, sensor +Z → station -Z
        let m = [
            [1.0, 0.0, 0.0, 0.20],
            [0.0, -1.0, 0.0, 0.68],
            [0.0, 0.0, -1.0, 7.85],
            [0.0, 0.0, 0.0, 1.00],
        ];

        // sensor origin → station position
        let origin = transform_point(&m, 0.0, 0.0, 0.0);
        assert!((origin.0 - 0.20).abs() < 1e-5);
        assert!((origin.1 - 0.68).abs() < 1e-5);
        assert!((origin.2 - 7.85).abs() < 1e-5);

        // sensor +X → station +X
        let px = transform_point(&m, 1.0, 0.0, 0.0);
        assert!((px.0 - 1.20).abs() < 1e-5);
        assert!((px.1 - 0.68).abs() < 1e-5);
        assert!((px.2 - 7.85).abs() < 1e-5);

        // sensor +Y → station -Y
        let py = transform_point(&m, 0.0, 1.0, 0.0);
        assert!((py.0 - 0.20).abs() < 1e-5);
        assert!((py.1 - (-0.32)).abs() < 1e-5);
        assert!((py.2 - 7.85).abs() < 1e-5);

        // sensor +Z → station -Z
        let pz = transform_point(&m, 0.0, 0.0, 1.0);
        assert!((pz.0 - 0.20).abs() < 1e-5);
        assert!((pz.1 - 0.68).abs() < 1e-5);
        assert!((pz.2 - 6.85).abs() < 1e-5);
    }

    fn transform_point(m: &[[f32; 4]; 4], x: f32, y: f32, z: f32) -> (f32, f32, f32) {
        let tx = m[0][0] * x + m[0][1] * y + m[0][2] * z + m[0][3];
        let ty = m[1][0] * x + m[1][1] * y + m[1][2] * z + m[1][3];
        let tz = m[2][0] * x + m[2][1] * y + m[2][2] * z + m[2][3];
        (tx, ty, tz)
    }
}
