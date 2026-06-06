use std::{fs::File, io::Write};

use serde_json::json;

use super::*;
use crate::{
    cloud::{Frame, PointXyzir},
    replay::current_time_string,
    transform::{CoordinateTransform, YawPitchRollPose},
};

fn point(x: f32, y: f32, z: f32, intensity: f32, ring: u16) -> PointXyzir {
    PointXyzir {
        x,
        y,
        z,
        intensity,
        ring,
    }
}

fn frame() -> Frame<PointXyzir> {
    Frame {
        stamp_micros: 123_456,
        sequence: 42,
        frame_id: "station".to_owned(),
        width: 2,
        height: 1,
        is_dense: true,
        points: vec![
            point(1.0, 2.0, 3.0, 4.0, 5),
            point(-1.0, -2.0, -3.0, 8.0, 9),
        ],
    }
}

#[test]
fn qpcd_roundtrip_preserves_header_and_points() {
    let dir = tempfile_dir("qpcd_roundtrip");
    let path = dir.join("frame.qpcd");
    let input = frame();

    let header = write_qpcd(&path, &input, "station").unwrap();
    let (read_header, output) = read_qpcd(&path).unwrap();

    assert_eq!(header, read_header);
    assert_eq!(read_header.point_stride, QPCD_POINT_STRIDE);
    assert_eq!(read_header.coord_frame, "station");
    assert_eq!(output, input);
}

#[test]
fn qpcd_rejects_invalid_magic() {
    let dir = tempfile_dir("qpcd_bad_magic");
    let path = dir.join("bad.qpcd");
    let mut file = File::create(&path).unwrap();
    file.write_all(b"not-qpcd").unwrap();

    let error = read_qpcd(&path).unwrap_err();

    assert!(error.to_string().contains("invalid qpcd magic"));
}

#[test]
fn sqlite_insert_read_points_to_readable_qpcd() {
    let dir = tempfile_dir("sqlite_store");
    let qpcd_path = dir.join("frames").join("frame_000001.qpcd");
    let input = frame();
    let qpcd_header = write_qpcd(&qpcd_path, &input, "station").unwrap();
    let store = SqliteStore::open(dir.join("capture.sqlite")).unwrap();
    let now = current_time_string();
    let transform = YawPitchRollPose {
        x_m: 1.0,
        ..YawPitchRollPose::default()
    }
    .to_transform();
    let snapshot = transform.snapshot();
    let transform_json = serde_json::to_string(&snapshot).unwrap();

    store
        .insert_capture_session(&NewCaptureSession {
            session_id: "session-1".to_owned(),
            started_at: now.clone(),
            sensor_host: "192.0.2.10".to_owned(),
            sensor_model: Some("M8".to_owned()),
            sdk_version: env!("CARGO_PKG_VERSION").to_owned(),
            status: "running".to_owned(),
            notes: Some("first pass".to_owned()),
        })
        .unwrap();
    let frame_id = store
        .insert_scan_frame(&NewScanFrame {
            session_id: "session-1".to_owned(),
            sequence: input.sequence,
            timestamp_micros: input.stamp_micros,
            sensor_host: "192.0.2.10".to_owned(),
            sensor_model: Some("M8".to_owned()),
            packet_type_mask: Some(0x04),
            point_count: input.points.len() as u64,
            coord_frame: "station".to_owned(),
            transform_4x4: snapshot.matrix_4x4,
            transform_json,
            calibration_json: json!({"calibration_complete": true}).to_string(),
            cloud_path: qpcd_path.to_string_lossy().into_owned(),
            qraw_path: None,
            status: "complete".to_owned(),
            created_at: now.clone(),
        })
        .unwrap();

    let session = store.get_capture_session("session-1").unwrap().unwrap();
    let record = store
        .get_scan_frame("session-1", input.sequence)
        .unwrap()
        .unwrap();
    let listed = store.list_scan_frames("session-1").unwrap();
    let (stored_header, stored_frame) = read_qpcd(&record.cloud_path).unwrap();

    assert_eq!(session.notes.as_deref(), Some("first pass"));
    assert_eq!(record.frame_id, frame_id);
    assert_eq!(record.point_count, qpcd_header.point_count);
    assert_eq!(record.transform_4x4, snapshot.matrix_4x4);
    assert_eq!(listed.len(), 1);
    assert_eq!(stored_header, qpcd_header);
    assert_eq!(stored_frame, input);
}

fn tempfile_dir(name: &str) -> std::path::PathBuf {
    let unique = format!(
        "{}_{}_{}",
        name,
        std::process::id(),
        time::OffsetDateTime::now_utc().unix_timestamp_nanos()
    );
    let path = std::env::temp_dir().join(unique);
    std::fs::create_dir_all(&path).unwrap();
    path
}
