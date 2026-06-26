//! Height estimation for tamping-hammer point clouds.
//!
//! The primary estimator is a robust top-percentile median:
//! 1. Filter to finite Z values within the ROI.
//! 2. Sort by Z descending.
//! 3. Take the top `top_ratio` proportion of points.
//! 4. Return the median Z of those points.
//!
//! This is resistant to outliers and does not require a flat
//! reference surface to be pre-defined.

use super::result::HammerMeasurement;

/// Configuration for the top-percentile median estimator.
#[derive(Debug, Clone)]
pub struct TopPercentileConfig {
    /// Proportion of points considered "top" (e.g. 0.1 = top 10%).
    pub top_ratio: f32,
    /// Minimum number of valid (finite Z) points required for a valid estimate.
    pub min_valid_points: usize,
}

impl Default for TopPercentileConfig {
    fn default() -> Self {
        Self {
            top_ratio: 0.1,
            min_valid_points: 10,
        }
    }
}

/// Estimate the top Z using a robust top-percentile median.
///
/// `z_values` should be the Z coordinates of points within the hammer ROI.
/// Finite values are filtered; NaNs are ignored.
///
/// Returns `None` if there are fewer than `min_valid_points` valid points.
pub fn estimate_top_z(z_values: &[f32], config: &TopPercentileConfig) -> Option<f32> {
    let mut finite: Vec<f32> = z_values.iter().copied().filter(|z| z.is_finite()).collect();
    if finite.len() < config.min_valid_points {
        return None;
    }

    // Sort descending
    finite.sort_unstable_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    let top_n = ((finite.len() as f32) * config.top_ratio).ceil() as usize;
    let top_n = top_n.max(1).min(finite.len());

    let median = if top_n % 2 == 0 {
        (finite[top_n / 2 - 1] + finite[top_n / 2]) / 2.0
    } else {
        finite[top_n / 2]
    };

    Some(median)
}

/// Compute Z spread (max - min) of finite values.
pub fn z_spread(z_values: &[f32]) -> Option<f32> {
    let finite: Vec<f32> = z_values.iter().copied().filter(|z| z.is_finite()).collect();
    if finite.len() < 2 {
        return None;
    }
    let min = finite.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = finite.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    if min.is_finite() && max.is_finite() {
        Some(max - min)
    } else {
        None
    }
}

/// Compute a quality score 0.0–1.0 based on point count vs expected minimum.
///
/// Points at or above `min_points` get quality 1.0.
/// Below that, quality scales linearly.
pub fn quality_score(valid_points: usize, min_points: usize) -> f32 {
    if valid_points >= min_points {
        1.0
    } else if min_points == 0 {
        0.0
    } else {
        (valid_points as f32) / (min_points as f32)
    }
}

/// Produce a full HammerMeasurement for a single frame and hammer.
pub fn measure_hammer(
    sequence: u64,
    hammer_id: &str,
    all_z: &[f32],
    config: &TopPercentileConfig,
    reference_z_m: Option<f32>,
) -> HammerMeasurement {
    let valid: Vec<f32> = all_z.iter().copied().filter(|z| z.is_finite()).collect();
    let roi_point_count = all_z.len();
    let valid_point_count = valid.len();

    let top_z_m = estimate_top_z(all_z, config);
    let z_spread_m = z_spread(all_z);
    let quality = quality_score(valid_point_count, config.min_valid_points);

    let status = if valid_point_count < config.min_valid_points {
        "insufficient_points"
    } else if top_z_m.is_some() {
        "ok"
    } else {
        "no_valid_z"
    };

    let height_m = match (top_z_m, reference_z_m) {
        (Some(tz), Some(ref_z)) => Some(tz - ref_z),
        _ => None,
    };

    HammerMeasurement {
        sequence,
        hammer_id: hammer_id.to_owned(),
        roi_point_count,
        valid_point_count,
        top_z_m,
        z_spread_m,
        quality,
        estimator: "top_pct_median".to_owned(),
        status: status.to_owned(),
        reference_z_m,
        height_m,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_z_returns_median_of_top_portion() {
        let zs: Vec<f32> = (0..100).map(|i| i as f32).collect();
        let config = TopPercentileConfig {
            top_ratio: 0.1,
            min_valid_points: 5,
        };
        // Top 10% of 0..100 = [90, 91, ..., 99] (10 values)
        // Median of those = 94.5
        let result = estimate_top_z(&zs, &config).unwrap();
        assert!((result - 94.5).abs() < 0.01);
    }

    #[test]
    fn insufficient_points_returns_none() {
        let zs = vec![1.0, 2.0];
        let config = TopPercentileConfig {
            top_ratio: 0.1,
            min_valid_points: 5,
        };
        assert!(estimate_top_z(&zs, &config).is_none());
    }

    #[test]
    fn nan_values_are_filtered() {
        let zs = vec![1.0, f32::NAN, 2.0, 3.0, 4.0, 5.0];
        let config = TopPercentileConfig {
            top_ratio: 0.5,
            min_valid_points: 3,
        };
        // 5 finite values, top 50% = ceil(2.5) = 3 values → [3,4,5], median = 4
        let result = estimate_top_z(&zs, &config).unwrap();
        assert!((result - 4.0).abs() < 0.01);
    }

    #[test]
    fn quality_scores_correctly() {
        assert!((quality_score(10, 10) - 1.0).abs() < 0.001);
        assert!((quality_score(5, 10) - 0.5).abs() < 0.001);
        assert!((quality_score(0, 10) - 0.0).abs() < 0.001);
    }
}
