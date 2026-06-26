//! Tamping-hammer measurement module.
//!
//! Provides ROI-based point cloud segmentation, robust top-Z height
//! estimation, and measurement result types.

pub mod hammer_roi;
pub mod height;
pub mod result;

pub use hammer_roi::segment_frame;
pub use height::{estimate_top_z, measure_hammer, quality_score, z_spread, TopPercentileConfig};
pub use result::{HammerMeasurement, HammerSessionStats};
