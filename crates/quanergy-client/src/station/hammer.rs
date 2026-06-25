//! Static hammer geometry for tamping-station Y-axis layouts.
//!
//! Hammers are mounted at fixed positions along the station Y axis.
//! Their XY positions do not change during operation; only Z varies.

/// Axis-aligned 3D region of interest.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisAlignedRoi {
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
    pub min_z: f32,
    pub max_z: f32,
}

impl AxisAlignedRoi {
    /// Create a new ROI with explicit bounds.
    pub fn new(min_x: f32, max_x: f32, min_y: f32, max_y: f32, min_z: f32, max_z: f32) -> Self {
        Self {
            min_x,
            max_x,
            min_y,
            max_y,
            min_z,
            max_z,
        }
    }

    /// Create an XY ROI centered at (cx, cy) with given half-extents, plus Z range.
    pub fn from_center_xy(
        cx: f32,
        cy: f32,
        half_x: f32,
        half_y: f32,
        min_z: f32,
        max_z: f32,
    ) -> Self {
        Self {
            min_x: cx - half_x,
            max_x: cx + half_x,
            min_y: cy - half_y,
            max_y: cy + half_y,
            min_z,
            max_z,
        }
    }

    /// Test whether a 3D point (x, y, z) is inside this ROI.
    pub fn contains(&self, x: f32, y: f32, z: f32) -> bool {
        x >= self.min_x
            && x <= self.max_x
            && y >= self.min_y
            && y <= self.max_y
            && z >= self.min_z
            && z <= self.max_z
    }
}

/// Supported hammer layout axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportedHammerAxis {
    /// Hammers are distributed along the station Y axis.
    Y,
}

/// Geometry of a single tamping hammer.
#[derive(Debug, Clone, PartialEq)]
pub struct HammerGeometry {
    /// Unique hammer identifier (e.g. "H01").
    pub id: String,
    /// Centre X coordinate in station frame (may be offset from the axis).
    pub center_x_m: f32,
    /// Centre Y coordinate in station frame.
    pub center_y_m: f32,
    /// 3D region of interest for this hammer.
    pub roi: AxisAlignedRoi,
    /// Whether this hammer participates in measurement.
    pub enabled: bool,
}

impl HammerGeometry {
    /// Check whether a point falls inside this hammer's ROI.
    pub fn contains(&self, x: f32, y: f32, z: f32) -> bool {
        self.roi.contains(x, y, z)
    }
}

/// Full hammer layout: axis, global defaults, and individual hammer definitions.
#[derive(Debug, Clone, PartialEq)]
pub struct HammerLayout {
    /// The axis along which hammers are distributed.
    pub axis: SupportedHammerAxis,
    /// Nominal X coordinate of the hammer axis (usually 0.0).
    pub axis_x_m: f32,
    /// Vertical axis for height measurement.
    pub vertical_axis: String,
    /// Assignment strategy for overlapping ROIs.
    pub assignment: HammerAssignment,
    /// Default ROI half-extents and Z range.
    pub default_roi_half_x_m: f32,
    pub default_roi_half_y_m: f32,
    pub default_z_min_m: f32,
    pub default_z_max_m: f32,
    /// Minimum points required for a valid measurement.
    pub minimum_points: usize,
    /// Individual hammer definitions.
    pub hammers: Vec<HammerGeometry>,
}

/// How to assign a point that falls into multiple hammer ROIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HammerAssignment {
    /// Assign to the hammer whose centre Y is closest to the point's Y.
    NearestYCenter,
}

impl HammerLayout {
    /// Validate the layout is non-empty and has unique hammer IDs.
    pub fn validate_non_empty(&self) -> Result<(), String> {
        if self.hammers.is_empty() {
            return Err("hammer layout must contain at least one hammer".to_owned());
        }

        let mut seen = std::collections::HashSet::new();
        for h in &self.hammers {
            if !seen.insert(&h.id) {
                return Err(format!("duplicate hammer id: {}", h.id));
            }
        }

        Ok(())
    }

    /// Assign a point (x, y, z) to the nearest hammer by Y distance.
    ///
    /// Returns the index of the assigned hammer, or `None` if the point
    /// does not fall inside any enabled hammer's ROI.
    pub fn assign_nearest_y(&self, x: f32, y: f32, z: f32) -> Option<usize> {
        let mut best_idx: Option<usize> = None;
        let mut best_dist = f32::INFINITY;

        for (i, hammer) in self.hammers.iter().enumerate() {
            if !hammer.enabled {
                continue;
            }
            if !hammer.contains(x, y, z) {
                continue;
            }
            let dist = (y - hammer.center_y_m).abs();
            if dist < best_dist {
                best_dist = dist;
                best_idx = Some(i);
            }
        }

        best_idx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_hammer(id: &str, y: f32) -> HammerGeometry {
        HammerGeometry {
            id: id.to_owned(),
            center_x_m: 0.0,
            center_y_m: y,
            roi: AxisAlignedRoi::from_center_xy(0.0, y, 0.15, 0.10, 0.0, 5.0),
            enabled: true,
        }
    }

    #[test]
    fn roi_contains_point() {
        let roi = AxisAlignedRoi::from_center_xy(0.0, 1.0, 0.5, 0.3, 0.0, 4.0);
        assert!(roi.contains(0.0, 1.0, 2.0));
        assert!(!roi.contains(0.6, 1.0, 2.0)); // x out
        assert!(!roi.contains(0.0, 1.4, 2.0)); // y out
        assert!(!roi.contains(0.0, 1.0, -0.1)); // z below
        assert!(!roi.contains(0.0, 1.0, 4.1)); // z above
    }

    #[test]
    fn roi_boundary_inclusive() {
        let roi = AxisAlignedRoi::from_center_xy(0.0, 0.0, 1.0, 1.0, 0.0, 1.0);
        assert!(roi.contains(1.0, 1.0, 1.0));
        assert!(roi.contains(-1.0, -1.0, 0.0));
    }

    #[test]
    fn assign_nearest_y_center() {
        let layout = HammerLayout {
            axis: SupportedHammerAxis::Y,
            axis_x_m: 0.0,
            vertical_axis: "z".to_owned(),
            assignment: HammerAssignment::NearestYCenter,
            default_roi_half_x_m: 0.15,
            default_roi_half_y_m: 0.10,
            default_z_min_m: 0.0,
            default_z_max_m: 5.0,
            minimum_points: 100,
            hammers: vec![
                make_hammer("H01", 0.0),
                make_hammer("H02", 1.0),
                make_hammer("H03", 2.0),
            ],
        };

        // point at y=0.95 should go to H02 (dist 0.05 vs dist 0.95 from H01)
        assert_eq!(layout.assign_nearest_y(0.0, 0.95, 2.0), Some(1));

        // point at y=0.04 should go to H01
        assert_eq!(layout.assign_nearest_y(0.0, 0.04, 2.0), Some(0));

        // point outside all ROIs
        assert_eq!(layout.assign_nearest_y(10.0, 0.5, 2.0), None);
    }

    #[test]
    fn disabled_hammer_not_assigned() {
        // Two hammers with overlapping ROIs (half_y=1.0 → overlap).
        let make_h = |id: &str, y: f32| HammerGeometry {
            id: id.to_owned(),
            center_x_m: 0.0,
            center_y_m: y,
            roi: AxisAlignedRoi::from_center_xy(0.0, y, 1.0, 1.0, 0.0, 5.0),
            enabled: true,
        };

        let layout = HammerLayout {
            axis: SupportedHammerAxis::Y,
            axis_x_m: 0.0,
            vertical_axis: "z".to_owned(),
            assignment: HammerAssignment::NearestYCenter,
            default_roi_half_x_m: 0.15,
            default_roi_half_y_m: 0.10,
            default_z_min_m: 0.0,
            default_z_max_m: 5.0,
            minimum_points: 100,
            hammers: vec![
                {
                    let mut h = make_h("H01", 0.0);
                    h.enabled = false;
                    h
                },
                make_h("H02", 1.0),
            ],
        };

        // point at y=0.0: in both ROIs, closest to disabled H01 → falls back to H02
        assert_eq!(layout.assign_nearest_y(0.0, 0.0, 2.0), Some(1));
    }

    #[test]
    fn validate_non_empty_rejects_empty() {
        let layout = HammerLayout {
            axis: SupportedHammerAxis::Y,
            axis_x_m: 0.0,
            vertical_axis: "z".to_owned(),
            assignment: HammerAssignment::NearestYCenter,
            default_roi_half_x_m: 0.15,
            default_roi_half_y_m: 0.10,
            default_z_min_m: 0.0,
            default_z_max_m: 5.0,
            minimum_points: 100,
            hammers: vec![],
        };
        assert!(layout.validate_non_empty().is_err());
    }

    #[test]
    fn validate_rejects_duplicate_ids() {
        let layout = HammerLayout {
            axis: SupportedHammerAxis::Y,
            axis_x_m: 0.0,
            vertical_axis: "z".to_owned(),
            assignment: HammerAssignment::NearestYCenter,
            default_roi_half_x_m: 0.15,
            default_roi_half_y_m: 0.10,
            default_z_min_m: 0.0,
            default_z_max_m: 5.0,
            minimum_points: 100,
            hammers: vec![make_hammer("H01", 0.0), make_hammer("H01", 1.0)],
        };
        assert!(layout.validate_non_empty().is_err());
    }

    #[test]
    fn x_offset_hammer_respected() {
        let h = HammerGeometry {
            id: "H01_offset".to_owned(),
            center_x_m: 0.1,
            center_y_m: 0.0,
            roi: AxisAlignedRoi::from_center_xy(0.1, 0.0, 0.1, 0.1, 0.0, 5.0),
            enabled: true,
        };

        // x=0.0 should be inside (within ±0.1 of 0.1)
        assert!(h.contains(0.0, 0.0, 2.0));
        // x=0.25 should be outside
        assert!(!h.contains(0.25, 0.0, 2.0));
    }

    #[test]
    fn assign_overlap_deterministic() {
        // Two overlapping hammers: assign to nearest Y centre
        let make_w = |id: &str, y: f32| HammerGeometry {
            id: id.to_owned(),
            center_x_m: 0.0,
            center_y_m: y,
            roi: AxisAlignedRoi::from_center_xy(0.0, y, 0.5, 0.5, 0.0, 5.0),
            enabled: true,
        };

        let layout = HammerLayout {
            axis: SupportedHammerAxis::Y,
            axis_x_m: 0.0,
            vertical_axis: "z".to_owned(),
            assignment: HammerAssignment::NearestYCenter,
            default_roi_half_x_m: 0.5,
            default_roi_half_y_m: 0.5,
            default_z_min_m: 0.0,
            default_z_max_m: 5.0,
            minimum_points: 100,
            hammers: vec![make_w("H01", 0.0), make_w("H02", 0.5)],
        };

        // y=0.20: dist to H01=0.20, to H02=0.30 → H01 wins
        assert_eq!(layout.assign_nearest_y(0.0, 0.20, 2.0), Some(0));
        // y=0.30: dist to H01=0.30, to H02=0.20 → H02 wins
        assert_eq!(layout.assign_nearest_y(0.0, 0.30, 2.0), Some(1));
        // y=0.25: equidistant, H01 wins (first in order when distance ties)
        assert_eq!(layout.assign_nearest_y(0.0, 0.25, 2.0), Some(0));
    }
}
