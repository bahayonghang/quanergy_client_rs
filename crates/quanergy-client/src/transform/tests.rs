use serde_json::json;

use super::*;
use crate::cloud::{Frame, PointXyzir};

fn point(x: f32, y: f32, z: f32) -> PointXyzir {
    PointXyzir {
        x,
        y,
        z,
        intensity: 42.0,
        ring: 7,
    }
}

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 1e-5,
        "expected {expected}, got {actual}"
    );
}

fn assert_point_close(actual: PointXyzir, expected: PointXyzir) {
    assert_close(actual.x, expected.x);
    assert_close(actual.y, expected.y);
    assert_close(actual.z, expected.z);
    assert_eq!(actual.intensity, expected.intensity);
    assert_eq!(actual.ring, expected.ring);
}

#[test]
fn identity_transform_leaves_points_unchanged() {
    let transform = Matrix4Transform::identity();
    let input = point(1.0, 2.0, 3.0);

    assert_eq!(transform.transform_point(input), input);
}

#[test]
fn pose_translation_moves_sensor_origin_to_station_position() {
    let transform = YawPitchRollPose {
        x_m: 1.0,
        y_m: 2.0,
        z_m: 3.0,
        ..YawPitchRollPose::default()
    }
    .to_transform();

    assert_point_close(
        transform.transform_point(point(0.5, 0.25, 1.0)),
        point(1.5, 2.25, 4.0),
    );
}

#[test]
fn yaw_rotates_sensor_x_to_station_y() {
    let transform = YawPitchRollPose {
        yaw_deg: 90.0,
        ..YawPitchRollPose::default()
    }
    .to_transform();

    assert_point_close(
        transform.transform_point(point(1.0, 0.0, 0.0)),
        point(0.0, 1.0, 0.0),
    );
}

#[test]
fn pitch_rotates_sensor_x_to_negative_station_z() {
    let transform = YawPitchRollPose {
        pitch_deg: 90.0,
        ..YawPitchRollPose::default()
    }
    .to_transform();

    assert_point_close(
        transform.transform_point(point(1.0, 0.0, 0.0)),
        point(0.0, 0.0, -1.0),
    );
}

#[test]
fn roll_rotates_sensor_y_to_station_z() {
    let transform = YawPitchRollPose {
        roll_deg: 90.0,
        ..YawPitchRollPose::default()
    }
    .to_transform();

    assert_point_close(
        transform.transform_point(point(0.0, 1.0, 0.0)),
        point(0.0, 0.0, 1.0),
    );
}

#[test]
fn frame_transform_preserves_metadata_and_non_xyz_fields() {
    let frame = Frame {
        stamp_micros: 123,
        sequence: 9,
        frame_id: "sensor".to_owned(),
        width: 1,
        height: 1,
        is_dense: true,
        points: vec![point(1.0, 0.0, 0.0)],
    };
    let transform = Matrix4Transform::new(
        "test",
        [
            [1.0, 0.0, 0.0, 5.0],
            [0.0, 1.0, 0.0, 6.0],
            [0.0, 0.0, 1.0, 7.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    );

    let output = transform.transform_frame(&frame);

    assert_eq!(output.stamp_micros, frame.stamp_micros);
    assert_eq!(output.sequence, frame.sequence);
    assert_eq!(output.frame_id, frame.frame_id);
    assert_point_close(output.points[0], point(6.0, 6.0, 7.0));
}

struct OffsetTransform;

impl CoordinateTransform for OffsetTransform {
    fn name(&self) -> &str {
        "offset_test"
    }

    fn transform_point(&self, point: PointXyzir) -> PointXyzir {
        PointXyzir {
            x: point.x + 10.0,
            ..point
        }
    }

    fn matrix_4x4(&self) -> [[f32; 4]; 4] {
        [
            [1.0, 0.0, 0.0, 10.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }

    fn snapshot(&self) -> TransformSnapshot {
        TransformSnapshot {
            algorithm: self.name().to_owned(),
            matrix_4x4: self.matrix_4x4(),
            config: json!({"kind": "test"}),
        }
    }
}

#[test]
fn frame_helper_accepts_custom_transform_strategy() {
    let frame = Frame::with_points("station", vec![point(1.0, 2.0, 3.0)]);

    let output = apply_transform(&frame, &OffsetTransform);

    assert_point_close(output.points[0], point(11.0, 2.0, 3.0));
    assert_eq!(OffsetTransform.snapshot().algorithm, "offset_test");
}

#[test]
fn snapshot_includes_pose_fields_and_matrix() {
    let transform = YawPitchRollPose {
        x_m: 1.0,
        y_m: 2.0,
        z_m: 3.0,
        yaw_deg: 4.0,
        pitch_deg: 5.0,
        roll_deg: 6.0,
    }
    .to_transform();

    let snapshot = transform.snapshot();

    assert_eq!(snapshot.algorithm, "yaw_pitch_roll_pose");
    assert_eq!(snapshot.config["yaw_deg"], json!(4.0));
    assert_eq!(snapshot.config["pitch_deg"], json!(5.0));
    assert_eq!(snapshot.config["roll_deg"], json!(6.0));
    assert_eq!(snapshot.matrix_4x4, transform.matrix_4x4());
}

// ---------------------------------------------------------------------------
// StationTransform tests
// ---------------------------------------------------------------------------

#[test]
fn station_transform_sets_target_frame_id() {
    let transform = StationTransform::new(
        "quanergy_sensor",
        "station",
        "test-xform",
        [
            [1.0, 0.0, 0.0, 0.20],
            [0.0, -1.0, 0.0, 0.68],
            [0.0, 0.0, -1.0, 7.85],
            [0.0, 0.0, 0.0, 1.00],
        ],
    );

    let frame = Frame {
        stamp_micros: 123,
        sequence: 9,
        frame_id: "quanergy_sensor".to_owned(),
        width: 1,
        height: 1,
        is_dense: true,
        points: vec![point(1.0, 0.0, 0.0)],
    };

    let output = transform.transform_frame_to_target(&frame);

    assert_eq!(output.frame_id, "station");
    assert_eq!(output.stamp_micros, 123);
    assert_eq!(output.sequence, 9);
    assert_eq!(output.width, 1);
    assert_eq!(output.height, 1);
    assert!(output.is_dense);
}

#[test]
fn station_transform_preserves_intensity_and_ring() {
    let transform = StationTransform::new(
        "sensor",
        "station",
        "test",
        [
            [1.0, 0.0, 0.0, 10.0],
            [0.0, 1.0, 0.0, 20.0],
            [0.0, 0.0, 1.0, 30.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    );

    let input = PointXyzir {
        x: 1.0,
        y: 2.0,
        z: 3.0,
        intensity: 255.0,
        ring: 8,
    };

    let output = transform.transform_point(input);
    assert_eq!(output.intensity, 255.0);
    assert_eq!(output.ring, 8);
    assert_close(output.x, 11.0);
    assert_close(output.y, 22.0);
    assert_close(output.z, 33.0);
}

#[test]
fn station_transform_snapshot_includes_frame_provenance() {
    let transform = StationTransform::new(
        "quanergy_sensor",
        "station",
        "extrinsic-001",
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    );

    let snapshot = transform.snapshot();
    assert_eq!(snapshot.config["source_frame"], json!("quanergy_sensor"));
    assert_eq!(snapshot.config["target_frame"], json!("station"));
    assert_eq!(snapshot.config["transform_id"], json!("extrinsic-001"));
}

#[test]
fn station_transform_candidate_downward_matrix() {
    // From plan §4.4
    let transform = StationTransform::new(
        "quanergy_sensor",
        "station",
        "candidate",
        [
            [1.0, 0.0, 0.0, 0.20],
            [0.0, -1.0, 0.0, 0.68],
            [0.0, 0.0, -1.0, 7.85],
            [0.0, 0.0, 0.0, 1.00],
        ],
    );

    // sensor origin -> station (0.20, 0.68, 7.85)
    let origin = transform.transform_point(point(0.0, 0.0, 0.0));
    assert_close(origin.x, 0.20);
    assert_close(origin.y, 0.68);
    assert_close(origin.z, 7.85);

    // sensor +X -> station +X
    let px = transform.transform_point(point(1.0, 0.0, 0.0));
    assert_close(px.x, 1.20);
    assert_close(px.y, 0.68);
    assert_close(px.z, 7.85);

    // sensor +Y -> station -Y
    let py = transform.transform_point(point(0.0, 1.0, 0.0));
    assert_close(py.x, 0.20);
    assert_close(py.y, -0.32); // 0.68 - 1.0
    assert_close(py.z, 7.85);

    // sensor +Z -> station -Z
    let pz = transform.transform_point(point(0.0, 0.0, 1.0));
    assert_close(pz.x, 0.20);
    assert_close(pz.y, 0.68);
    assert_close(pz.z, 6.85); // 7.85 - 1.0
}

#[test]
fn station_transform_accessors() {
    let transform = StationTransform::new(
        "src",
        "dst",
        "xform-42",
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    );

    assert_eq!(transform.source_frame(), "src");
    assert_eq!(transform.target_frame(), "dst");
    assert_eq!(transform.transform_id(), "xform-42");
}
