//! ROI-based point cloud segmentation for static Y-axis hammer layouts.
//!
//! Uses [`HammerLayout`] from the `station` module to assign points
//! to individual hammers, respecting the nearest-Y-center strategy.

use crate::cloud::PointXyzir;
use crate::station::HammerLayout;

/// Result of segmenting a single frame: one Vec of Z values per hammer,
/// in the same order as the layout's hammer list.
#[derive(Debug, Clone)]
pub struct FrameSegmentation {
    /// Z values for each hammer (same order as `layout.hammers`).
    /// Points outside all ROIs are discarded.
    pub hammer_z_values: Vec<Vec<f32>>,
    /// Total points assigned (sum of all per-hammer counts).
    pub assigned_points: usize,
    /// Points that fell outside all ROIs.
    pub unassigned_points: usize,
}

/// Segment a frame of station-frame points into per-hammer Z values.
///
/// Points are assigned using the layout's [`HammerLayout::assign_nearest_y`]
/// strategy. Overlap warnings are logged once per layout instance.
pub fn segment_frame(points: &[PointXyzir], layout: &HammerLayout) -> FrameSegmentation {
    let n_hammers = layout.hammers.len();
    let mut hammer_z_values: Vec<Vec<f32>> = (0..n_hammers).map(|_| Vec::new()).collect();
    let mut assigned = 0usize;
    let mut unassigned = 0usize;

    for point in points {
        if let Some(idx) = layout.assign_nearest_y(point.x, point.y, point.z) {
            hammer_z_values[idx].push(point.z);
            assigned += 1;
        } else {
            unassigned += 1;
        }
    }

    FrameSegmentation {
        hammer_z_values,
        assigned_points: assigned,
        unassigned_points: unassigned,
    }
}

#[cfg(test)]
mod tests {
    use crate::cloud::PointXyzir;
    use crate::station::{
        AxisAlignedRoi, HammerAssignment, HammerGeometry, HammerLayout, SupportedHammerAxis,
    };

    use super::*;

    fn make_point(x: f32, y: f32, z: f32) -> PointXyzir {
        PointXyzir {
            x,
            y,
            z,
            intensity: 1.0,
            ring: 0,
        }
    }

    fn test_layout() -> HammerLayout {
        HammerLayout {
            axis: SupportedHammerAxis::Y,
            axis_x_m: 0.0,
            vertical_axis: "z".to_owned(),
            assignment: HammerAssignment::NearestYCenter,
            default_roi_half_x_m: 0.5,
            default_roi_half_y_m: 0.5,
            default_z_min_m: 0.0,
            default_z_max_m: 10.0,
            minimum_points: 5,
            hammers: vec![
                HammerGeometry {
                    id: "H01".to_owned(),
                    center_x_m: 0.0,
                    center_y_m: 0.0,
                    roi: AxisAlignedRoi::from_center_xy(0.0, 0.0, 0.5, 0.5, 0.0, 10.0),
                    enabled: true,
                },
                HammerGeometry {
                    id: "H02".to_owned(),
                    center_x_m: 0.0,
                    center_y_m: 2.0,
                    roi: AxisAlignedRoi::from_center_xy(0.0, 2.0, 0.5, 0.5, 0.0, 10.0),
                    enabled: true,
                },
            ],
        }
    }

    #[test]
    fn segment_assigns_points_to_nearest_y() {
        let layout = test_layout();
        let points = vec![
            make_point(0.0, 0.1, 1.0),    // H01
            make_point(0.0, 1.9, 2.0),    // H02
            make_point(0.0, 0.0, 0.5),    // H01
            make_point(10.0, 10.0, 10.0), // unassigned
        ];
        let seg = segment_frame(&points, &layout);
        assert_eq!(seg.hammer_z_values[0].len(), 2);
        assert_eq!(seg.hammer_z_values[1].len(), 1);
        assert_eq!(seg.assigned_points, 3);
        assert_eq!(seg.unassigned_points, 1);
    }
}
