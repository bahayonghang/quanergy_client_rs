//! Measurement results for tamping-hammer height estimation.
//!
//! These types are the output of ROI-based hammer analysis.
//! They are designed to be serializable for CSV export and
//! SQLite storage.

use serde::Serialize;

/// Result of analysing a single hammer ROI in one frame.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HammerMeasurement {
    /// Frame sequence number.
    pub sequence: u64,
    /// Hammer identifier (e.g. "H01").
    pub hammer_id: String,
    /// Number of points that fell inside the ROI.
    pub roi_point_count: usize,
    /// Number of points with finite (non-NaN) Z values.
    pub valid_point_count: usize,
    /// Estimated top Z in metres (top percentile median).
    pub top_z_m: Option<f32>,
    /// Z spread: max_z - min_z of valid points.
    pub z_spread_m: Option<f32>,
    /// Quality indicator: 1.0 = ideal, 0.0 = invalid.
    pub quality: f32,
    /// Estimator name (e.g. "top_pct_median").
    pub estimator: String,
    /// Status: "ok" | "insufficient_points" | "no_valid_z"
    pub status: String,
    /// Configured reference Z (if any), for height calculation.
    pub reference_z_m: Option<f32>,
    /// Height = top_z - reference_z (only when reference is configured).
    pub height_m: Option<f32>,
}

impl HammerMeasurement {
    /// Create an invalid measurement (insufficient points).
    pub fn invalid(sequence: u64, hammer_id: impl Into<String>, estimator: &str) -> Self {
        Self {
            sequence,
            hammer_id: hammer_id.into(),
            roi_point_count: 0,
            valid_point_count: 0,
            top_z_m: None,
            z_spread_m: None,
            quality: 0.0,
            estimator: estimator.to_owned(),
            status: "insufficient_points".to_owned(),
            reference_z_m: None,
            height_m: None,
        }
    }
}

/// Aggregated statistics for a single hammer across multiple frames.
#[derive(Debug, Clone, Serialize)]
pub struct HammerSessionStats {
    pub hammer_id: String,
    pub frame_count: usize,
    pub valid_frame_count: usize,
    pub mean_top_z_m: Option<f32>,
    pub std_top_z_m: Option<f32>,
    pub min_top_z_m: Option<f32>,
    pub max_top_z_m: Option<f32>,
    pub mean_point_count: f32,
}
