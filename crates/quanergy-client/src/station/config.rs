//! Station configuration: TOML model, parsing, validation, and hashing.
//!
//! The authoritative format is documented in
//! `ref/plans/quanergy_tamping_station_y_axis_modification_plan.md` §6.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::transform::{validate_rigid_matrix, RigidMatrixError, RigidTransformValidation};

use super::hammer::{
    AxisAlignedRoi, HammerAssignment, HammerGeometry, HammerLayout, SupportedHammerAxis,
};

// ---------------------------------------------------------------------------
// Raw TOML model — mirrors the on-disk format exactly
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StationConfigToml {
    pub schema_version: u32,
    pub station_id: String,
    #[serde(default)]
    pub frames: Option<FrameConfigToml>,
    #[serde(default)]
    pub scanner: Option<ScannerConfigToml>,
    #[serde(default)]
    pub hammers: Option<HammersConfigToml>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameConfigToml {
    pub source: String,
    pub target: String,
    #[serde(default = "default_length_unit")]
    pub length_unit: String,
}

fn default_length_unit() -> String {
    "m".to_owned()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScannerConfigToml {
    #[serde(default)]
    pub serial: String,
    #[serde(default)]
    pub mount_description: String,
    pub extrinsic_id: String,
    pub extrinsic: ExtrinsicConfigToml,
    #[serde(default)]
    pub calibration: Option<CalibrationMetaToml>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtrinsicConfigToml {
    pub kind: String,
    pub matrix_4x4: [[f64; 4]; 4],
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalibrationMetaToml {
    #[serde(default)]
    pub calibrated_at: String,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub rms_error_m: f64,
    #[serde(default)]
    pub max_error_m: f64,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HammersConfigToml {
    pub axis: String,
    #[serde(default)]
    pub axis_x_m: f64,
    pub vertical_axis: String,
    #[serde(default)]
    pub assignment: String,
    #[serde(default = "default_roi_half_x")]
    pub default_roi_half_x_m: f64,
    #[serde(default = "default_roi_half_y")]
    pub default_roi_half_y_m: f64,
    #[serde(default)]
    pub default_z_min_m: f64,
    #[serde(default = "default_z_max")]
    pub default_z_max_m: f64,
    #[serde(default = "default_min_points")]
    pub minimum_points: u64,
    pub items: Vec<HammerItemToml>,
}

fn default_roi_half_x() -> f64 {
    0.15
}
fn default_roi_half_y() -> f64 {
    0.10
}
fn default_z_max() -> f64 {
    5.0
}
fn default_min_points() -> u64 {
    100
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HammerItemToml {
    pub id: String,
    #[serde(default)]
    pub enabled: bool,
    pub y_m: f64,
    #[serde(default)]
    pub x_offset_m: f64,
    #[serde(default)]
    pub roi_half_x_m: Option<f64>,
    #[serde(default)]
    pub roi_half_y_m: Option<f64>,
    #[serde(default)]
    pub z_min_m: Option<f64>,
    #[serde(default)]
    pub z_max_m: Option<f64>,
}

// ---------------------------------------------------------------------------
// Validation errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum StationConfigError {
    UnsupportedSchema {
        found: u32,
        expected: u32,
    },
    EmptyField {
        field: String,
    },
    FramesIdentical {
        source: String,
        target: String,
    },
    UnsupportedUnit {
        unit: String,
    },
    UnsupportedExtrinsicKind {
        kind: String,
    },
    InvalidMatrix(RigidMatrixError),
    UnsupportedHammerAxis {
        axis: String,
    },
    InvalidVerticalAxis {
        axis: String,
    },
    DuplicateHammerId {
        id: String,
    },
    NonFiniteHammerY {
        id: String,
        y: f64,
    },
    NonFiniteHammerXOffset {
        id: String,
        offset: f64,
    },
    InvalidRoiHalfWidth {
        id: String,
        field: String,
        value: f64,
    },
    InvalidZRange {
        id: String,
        z_min: f64,
        z_max: f64,
    },
    MinimumPointsZero,
    EmptyHammers,
    Io {
        path: String,
        message: String,
    },
    TomlParse(toml::de::Error),
}

impl std::fmt::Display for StationConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedSchema { found, expected } => {
                write!(f, "unsupported schema_version {found}, expected {expected}")
            }
            Self::EmptyField { field } => write!(f, "{field} must not be empty"),
            Self::FramesIdentical { source, target } => {
                write!(
                    f,
                    "source frame '{source}' and target frame '{target}' must differ"
                )
            }
            Self::UnsupportedUnit { unit } => {
                write!(f, "unsupported length_unit '{unit}', expected 'm'")
            }
            Self::UnsupportedExtrinsicKind { kind } => {
                write!(f, "unsupported extrinsic kind '{kind}', expected 'matrix'")
            }
            Self::InvalidMatrix(e) => write!(f, "invalid extrinsic matrix: {e}"),
            Self::UnsupportedHammerAxis { axis } => {
                write!(f, "unsupported hammer axis '{axis}', expected 'y'")
            }
            Self::InvalidVerticalAxis { axis } => {
                write!(f, "unsupported vertical_axis '{axis}', expected 'z'")
            }
            Self::DuplicateHammerId { id } => write!(f, "duplicate hammer id: {id}"),
            Self::NonFiniteHammerY { id, y } => {
                write!(f, "hammer {id}: y_m={y} is non-finite")
            }
            Self::NonFiniteHammerXOffset { id, offset } => {
                write!(f, "hammer {id}: x_offset_m={offset} is non-finite")
            }
            Self::InvalidRoiHalfWidth { id, field, value } => {
                write!(f, "hammer {id}: {field}={value} must be > 0")
            }
            Self::InvalidZRange { id, z_min, z_max } => {
                write!(f, "hammer {id}: z_min={z_min} >= z_max={z_max}")
            }
            Self::MinimumPointsZero => {
                write!(f, "minimum_points must be > 0")
            }
            Self::EmptyHammers => write!(f, "hammers.items must not be empty"),
            Self::Io { path, message } => {
                write!(f, "failed to read '{path}': {message}")
            }
            Self::TomlParse(e) => write!(f, "TOML parse error: {e}"),
        }
    }
}

impl std::error::Error for StationConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidMatrix(e) => Some(e),
            Self::TomlParse(e) => Some(e),
            _ => None,
        }
    }
}

impl From<toml::de::Error> for StationConfigError {
    fn from(e: toml::de::Error) -> Self {
        Self::TomlParse(e)
    }
}

// ---------------------------------------------------------------------------
// Validated (canonical) types
// ---------------------------------------------------------------------------

/// Fully validated station configuration, ready for use at runtime.
#[derive(Debug, Clone)]
pub struct ValidatedStationConfig {
    pub station_id: String,
    pub source_frame: String,
    pub target_frame: String,
    pub extrinsic_id: String,
    /// 4×4 rigid transform matrix (f32 for point cloud path).
    pub sensor_to_station: [[f32; 4]; 4],
    pub calibration: Option<CalibrationMeta>,
    pub hammer_layout: HammerLayout,
    /// SHA-256 hex digest of the canonical TOML used to produce this config.
    config_hash: String,
    /// Raw canonical TOML text used for the hash.
    canonical_toml: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationMeta {
    pub calibrated_at: String,
    pub method: String,
    pub rms_error_m: f64,
    pub max_error_m: f64,
    pub notes: String,
}

impl ValidatedStationConfig {
    /// SHA-256 hex digest of the canonical TOML that produced this config.
    pub fn config_hash(&self) -> &str {
        &self.config_hash
    }

    /// The raw canonical TOML that was hashed.
    pub fn canonical_toml(&self) -> &str {
        &self.canonical_toml
    }

    /// The 4×4 sensor-to-station transform matrix (f32).
    pub fn transform(&self) -> [[f32; 4]; 4] {
        self.sensor_to_station
    }

    /// Convenience: all enabled hammer ROIs in order.
    pub fn hammer_rois(&self) -> impl Iterator<Item = &HammerGeometry> {
        self.hammer_layout.hammers.iter().filter(|h| h.enabled)
    }
}

// ---------------------------------------------------------------------------
// Parsing + validation
// ---------------------------------------------------------------------------

/// Load a station config from a TOML file path.
pub fn load_station_config(
    path: &std::path::Path,
) -> Result<ValidatedStationConfig, StationConfigError> {
    let text = std::fs::read_to_string(path).map_err(|e| StationConfigError::Io {
        path: path.display().to_string(),
        message: e.to_string(),
    })?;
    parse_station_config(&text)
}

/// Parse and validate a station config from TOML text.
pub fn parse_station_config(text: &str) -> Result<ValidatedStationConfig, StationConfigError> {
    let raw: StationConfigToml = toml::from_str(text)?;
    validate(raw, text)
}

fn validate(
    raw: StationConfigToml,
    canonical_text: &str,
) -> Result<ValidatedStationConfig, StationConfigError> {
    // --- schema_version ---
    if raw.schema_version != 1 {
        return Err(StationConfigError::UnsupportedSchema {
            found: raw.schema_version,
            expected: 1,
        });
    }

    // --- station_id ---
    if raw.station_id.is_empty() {
        return Err(StationConfigError::EmptyField {
            field: "station_id".to_owned(),
        });
    }

    // --- frames ---
    let frames = raw.frames.unwrap_or_else(|| FrameConfigToml {
        source: "quanergy_sensor".to_owned(),
        target: "station".to_owned(),
        length_unit: "m".to_owned(),
    });

    if frames.source.is_empty() {
        return Err(StationConfigError::EmptyField {
            field: "frames.source".to_owned(),
        });
    }
    if frames.target.is_empty() {
        return Err(StationConfigError::EmptyField {
            field: "frames.target".to_owned(),
        });
    }
    if frames.source == frames.target {
        return Err(StationConfigError::FramesIdentical {
            source: frames.source,
            target: frames.target,
        });
    }
    if frames.length_unit != "m" {
        return Err(StationConfigError::UnsupportedUnit {
            unit: frames.length_unit,
        });
    }

    // --- scanner extrinsic ---
    let scanner = raw.scanner.ok_or_else(|| StationConfigError::EmptyField {
        field: "scanner".to_owned(),
    })?;

    if scanner.extrinsic_id.is_empty() {
        return Err(StationConfigError::EmptyField {
            field: "scanner.extrinsic_id".to_owned(),
        });
    }

    if scanner.extrinsic.kind != "matrix" {
        return Err(StationConfigError::UnsupportedExtrinsicKind {
            kind: scanner.extrinsic.kind.clone(),
        });
    }

    // Convert f64 matrix to f32 and validate
    let mut matrix_f32 = [[0.0f32; 4]; 4];
    for (i, row) in scanner.extrinsic.matrix_4x4.iter().enumerate() {
        for (j, &val) in row.iter().enumerate() {
            matrix_f32[i][j] = val as f32;
        }
    }

    let validation_opts = RigidTransformValidation::default();
    validate_rigid_matrix(matrix_f32, &validation_opts)
        .map_err(StationConfigError::InvalidMatrix)?;

    // --- calibration ---
    let calibration = scanner.calibration.map(|c| CalibrationMeta {
        calibrated_at: c.calibrated_at,
        method: c.method,
        rms_error_m: c.rms_error_m,
        max_error_m: c.max_error_m,
        notes: c.notes,
    });

    // --- hammers ---
    let hammers_cfg = raw.hammers.ok_or(StationConfigError::EmptyHammers)?;

    if hammers_cfg.axis != "y" {
        return Err(StationConfigError::UnsupportedHammerAxis {
            axis: hammers_cfg.axis,
        });
    }
    if hammers_cfg.vertical_axis != "z" {
        return Err(StationConfigError::InvalidVerticalAxis {
            axis: hammers_cfg.vertical_axis,
        });
    }
    if hammers_cfg.minimum_points == 0 {
        return Err(StationConfigError::MinimumPointsZero);
    }
    if hammers_cfg.items.is_empty() {
        return Err(StationConfigError::EmptyHammers);
    }

    let assignment = match hammers_cfg.assignment.as_str() {
        "" | "nearest_y_center" => HammerAssignment::NearestYCenter,
        other => {
            // Unknown assignment strategy — should we error?
            // Plan says only nearest_y_center for now, so let's be strict.
            return Err(StationConfigError::UnsupportedUnit {
                unit: format!("assignment '{other}' — expected 'nearest_y_center'"),
            });
        }
    };

    let axis_x_m = hammers_cfg.axis_x_m as f32;

    let mut hammers = Vec::with_capacity(hammers_cfg.items.len());
    let mut seen_ids = std::collections::HashSet::new();

    for item in &hammers_cfg.items {
        if item.id.is_empty() {
            return Err(StationConfigError::EmptyField {
                field: "hammer id".to_owned(),
            });
        }
        if !seen_ids.insert(&item.id) {
            return Err(StationConfigError::DuplicateHammerId {
                id: item.id.clone(),
            });
        }

        if !item.y_m.is_finite() {
            return Err(StationConfigError::NonFiniteHammerY {
                id: item.id.clone(),
                y: item.y_m,
            });
        }
        if !item.x_offset_m.is_finite() {
            return Err(StationConfigError::NonFiniteHammerXOffset {
                id: item.id.clone(),
                offset: item.x_offset_m,
            });
        }

        let half_x = item
            .roi_half_x_m
            .unwrap_or(hammers_cfg.default_roi_half_x_m);
        let half_y = item
            .roi_half_y_m
            .unwrap_or(hammers_cfg.default_roi_half_y_m);
        let z_min = item.z_min_m.unwrap_or(hammers_cfg.default_z_min_m);
        let z_max = item.z_max_m.unwrap_or(hammers_cfg.default_z_max_m);

        if half_x <= 0.0 {
            return Err(StationConfigError::InvalidRoiHalfWidth {
                id: item.id.clone(),
                field: "roi_half_x_m".to_owned(),
                value: half_x,
            });
        }
        if half_y <= 0.0 {
            return Err(StationConfigError::InvalidRoiHalfWidth {
                id: item.id.clone(),
                field: "roi_half_y_m".to_owned(),
                value: half_y,
            });
        }
        if z_min >= z_max {
            return Err(StationConfigError::InvalidZRange {
                id: item.id.clone(),
                z_min,
                z_max,
            });
        }

        let center_x = axis_x_m + item.x_offset_m as f32;
        let center_y = item.y_m as f32;

        hammers.push(HammerGeometry {
            id: item.id.clone(),
            center_x_m: center_x,
            center_y_m: center_y,
            roi: AxisAlignedRoi::from_center_xy(
                center_x,
                center_y,
                half_x as f32,
                half_y as f32,
                z_min as f32,
                z_max as f32,
            ),
            enabled: item.enabled,
        });
    }

    let hammer_layout = HammerLayout {
        axis: SupportedHammerAxis::Y,
        axis_x_m,
        vertical_axis: hammers_cfg.vertical_axis,
        assignment,
        default_roi_half_x_m: hammers_cfg.default_roi_half_x_m as f32,
        default_roi_half_y_m: hammers_cfg.default_roi_half_y_m as f32,
        default_z_min_m: hammers_cfg.default_z_min_m as f32,
        default_z_max_m: hammers_cfg.default_z_max_m as f32,
        minimum_points: hammers_cfg.minimum_points as usize,
        hammers,
    };

    // --- config hash ---
    let config_hash = {
        let mut hasher = Sha256::new();
        hasher.update(canonical_text.as_bytes());
        format!("{:x}", hasher.finalize())
    };

    Ok(ValidatedStationConfig {
        station_id: raw.station_id,
        source_frame: frames.source,
        target_frame: frames.target,
        extrinsic_id: scanner.extrinsic_id,
        sensor_to_station: matrix_f32,
        calibration,
        hammer_layout,
        config_hash,
        canonical_toml: canonical_text.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example_toml() -> &'static str {
        include_str!("../../../../config/station.example.toml")
    }

    #[test]
    fn example_toml_parses() {
        let cfg = parse_station_config(example_toml()).expect("example config should parse");
        assert_eq!(cfg.station_id, "tamping-station-01");
        assert_eq!(cfg.source_frame, "quanergy_sensor");
        assert_eq!(cfg.target_frame, "station");
        assert_eq!(cfg.hammer_layout.hammers.len(), 3);
        assert!(!cfg.config_hash().is_empty());
    }

    #[test]
    fn hash_stable_for_same_input() {
        let a = parse_station_config(example_toml()).unwrap();
        let b = parse_station_config(example_toml()).unwrap();
        assert_eq!(a.config_hash(), b.config_hash());
    }

    #[test]
    fn hash_differs_for_different_input() {
        let a = parse_station_config(example_toml()).unwrap();
        let modified = example_toml().replace("tamping-station-01", "tamping-station-02");
        let b = parse_station_config(&modified).unwrap();
        assert_ne!(a.config_hash(), b.config_hash());
    }

    #[test]
    fn unknown_field_rejected() {
        let toml = example_toml().to_owned() + "\nfoo = 1\n";
        assert!(parse_station_config(&toml).is_err());
    }

    #[test]
    fn invalid_schema_rejected() {
        let toml = example_toml().replace("schema_version = 1", "schema_version = 99");
        assert!(matches!(
            parse_station_config(&toml),
            Err(StationConfigError::UnsupportedSchema { .. })
        ));
    }

    #[test]
    fn mirror_matrix_rejected() {
        // diag(1, 1, -1) → det = -1
        let bad = r#"
schema_version = 1
station_id = "test"

[frames]
source = "quanergy_sensor"
target = "station"

[scanner]
extrinsic_id = "test"
[scanner.extrinsic]
kind = "matrix"
matrix_4x4 = [
  [1.0, 0.0, 0.0, 0.0],
  [0.0, 1.0, 0.0, 0.0],
  [0.0, 0.0, -1.0, 0.0],
  [0.0, 0.0, 0.0, 1.0],
]

[hammers]
axis = "y"
vertical_axis = "z"
[[hammers.items]]
id = "H01"
y_m = 0.0
"#;
        assert!(matches!(
            parse_station_config(bad),
            Err(StationConfigError::InvalidMatrix(
                RigidMatrixError::DeterminantNotOne { .. }
            ))
        ));
    }

    #[test]
    fn empty_hammer_list_rejected() {
        let toml = r#"
schema_version = 1
station_id = "test"

[frames]
source = "quanergy_sensor"
target = "station"

[scanner]
extrinsic_id = "test"
[scanner.extrinsic]
kind = "matrix"
matrix_4x4 = [
  [1.0, 0.0, 0.0, 0.0],
  [0.0, 1.0, 0.0, 0.0],
  [0.0, 0.0, 1.0, 0.0],
  [0.0, 0.0, 0.0, 1.0],
]

[hammers]
axis = "y"
vertical_axis = "z"
items = []
"#;
        assert!(matches!(
            parse_station_config(toml),
            Err(StationConfigError::EmptyHammers)
        ));
    }

    #[test]
    fn duplicate_hammer_id_rejected() {
        let toml = r#"
schema_version = 1
station_id = "test"

[frames]
source = "quanergy_sensor"
target = "station"

[scanner]
extrinsic_id = "test"
[scanner.extrinsic]
kind = "matrix"
matrix_4x4 = [
  [1.0, 0.0, 0.0, 0.0],
  [0.0, 1.0, 0.0, 0.0],
  [0.0, 0.0, 1.0, 0.0],
  [0.0, 0.0, 0.0, 1.0],
]

[hammers]
axis = "y"
vertical_axis = "z"
[[hammers.items]]
id = "H01"
y_m = 0.0
[[hammers.items]]
id = "H01"
y_m = 1.0
"#;
        assert!(matches!(
            parse_station_config(toml),
            Err(StationConfigError::DuplicateHammerId { .. })
        ));
    }
}
