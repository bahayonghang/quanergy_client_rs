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
#[allow(deprecated)]
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
#[allow(deprecated)]
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
            station_id: None,
            source_frame: None,
            target_frame: None,
            transform_id: None,
            station_config_json: None,
            station_config_sha256: None,
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
            source_frame: None,
            target_frame: None,
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

// ── Phase 1 spike: pcd-rs gate validation ──────────────────────────

#[cfg(test)]
mod pcd_spike {
    use std::fs;

    use pcd_rs::{
        metas::{DataKind, ViewPoint},
        DynReader, DynWriter, Field, Schema, ValueKind, WriterInit,
    };

    use crate::cloud::{Frame, PointXyzir};

    /// Helper: build a Field from a slice of f32 values.
    fn field_f32(values: &[f32]) -> Field {
        Field::F32(values.to_vec())
    }

    /// Helper: build a Field from a slice of u16 values.
    fn field_u16(values: &[u16]) -> Field {
        Field::U16(values.to_vec())
    }

    /// Convert PointXyzir → pcd_rs DynRecord.
    fn to_dyn_record(p: &PointXyzir) -> pcd_rs::DynRecord {
        pcd_rs::DynRecord(vec![
            field_f32(&[p.x]),
            field_f32(&[p.y]),
            field_f32(&[p.z]),
            field_f32(&[p.intensity]),
            field_u16(&[p.ring]),
        ])
    }

    /// Convert DynRecord → PointXyzir.
    fn from_dyn_record(rec: &pcd_rs::DynRecord) -> PointXyzir {
        let x = if let pcd_rs::Field::F32(v) = &rec.0[0] {
            v[0]
        } else {
            panic!("expected F32")
        };
        let y = if let pcd_rs::Field::F32(v) = &rec.0[1] {
            v[0]
        } else {
            panic!("expected F32")
        };
        let z = if let pcd_rs::Field::F32(v) = &rec.0[2] {
            v[0]
        } else {
            panic!("expected F32")
        };
        let intensity = if let pcd_rs::Field::F32(v) = &rec.0[3] {
            v[0]
        } else {
            panic!("expected F32")
        };
        let ring = if let pcd_rs::Field::U16(v) = &rec.0[4] {
            v[0]
        } else {
            panic!("expected U16")
        };
        PointXyzir {
            x,
            y,
            z,
            intensity,
            ring,
        }
    }

    fn write_pcd_spike(
        path: &std::path::Path,
        frame: &Frame<PointXyzir>,
        encoding: DataKind,
        viewpoint: ViewPoint,
    ) -> pcd_rs::Result<()> {
        let schema = Schema::from_iter(vec![
            ("x", ValueKind::F32, 1),
            ("y", ValueKind::F32, 1),
            ("z", ValueKind::F32, 1),
            ("intensity", ValueKind::F32, 1),
            ("ring", ValueKind::U16, 1),
        ]);

        let mut writer: DynWriter<_> = WriterInit {
            width: frame.width as u64,
            height: frame.height as u64,
            viewpoint,
            data_kind: encoding,
            schema: Some(schema),
            version: Some("0.7".to_owned()),
        }
        .create(path)?;

        for p in &frame.points {
            writer.push(&to_dyn_record(p))?;
        }

        writer.finish()?;
        Ok(())
    }

    fn read_pcd_spike(path: &std::path::Path) -> pcd_rs::Result<Vec<PointXyzir>> {
        let reader = DynReader::open(path)?;
        reader.map(|r| r.map(|rec| from_dyn_record(&rec))).collect()
    }

    // ── tests ──────────────────────────────────────────────────

    #[test]
    fn spike_binary_roundtrip() {
        let dir = super::tempfile_dir("pcd_spike_binary");
        let path = dir.join("spike_binary.pcd");

        let frame = Frame {
            stamp_micros: 1000,
            sequence: 1,
            frame_id: "test".into(),
            width: 3,
            height: 1,
            is_dense: true,
            points: vec![
                PointXyzir {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                    intensity: 0.5,
                    ring: 0,
                },
                PointXyzir {
                    x: -1.0,
                    y: -2.0,
                    z: -3.0,
                    intensity: 0.8,
                    ring: u16::MAX,
                },
                PointXyzir {
                    x: f32::NAN,
                    y: f32::NAN,
                    z: f32::NAN,
                    intensity: 0.0,
                    ring: 7,
                },
            ],
        };

        write_pcd_spike(&path, &frame, DataKind::Binary, ViewPoint::default()).unwrap();
        let points = read_pcd_spike(&path).unwrap();

        assert_eq!(points.len(), 3);
        assert_eq!(points[0].ring, 0);
        assert_eq!(points[1].ring, u16::MAX);
        assert!(points[2].x.is_nan());
        assert_eq!(points[2].ring, 7);

        let file_size = fs::metadata(&path).unwrap().len();
        println!("binary file size: {} bytes ({} points)", file_size, 3);
    }

    #[test]
    fn spike_ascii_roundtrip() {
        let dir = super::tempfile_dir("pcd_spike_ascii");
        let path = dir.join("spike_ascii.pcd");

        let frame = Frame {
            stamp_micros: 0,
            sequence: 0,
            frame_id: "test".into(),
            width: 2,
            height: 1,
            is_dense: true,
            points: vec![
                PointXyzir {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                    intensity: 0.5,
                    ring: 5,
                },
                PointXyzir {
                    x: -1.0,
                    y: -2.0,
                    z: -3.0,
                    intensity: 0.8,
                    ring: 9,
                },
            ],
        };

        write_pcd_spike(&path, &frame, DataKind::Ascii, ViewPoint::default()).unwrap();
        let points = read_pcd_spike(&path).unwrap();

        assert_eq!(points.len(), 2);
        assert_eq!(points[0].ring, 5);
        assert_eq!(points[1].ring, 9);
    }

    #[test]
    fn spike_binary_compressed_roundtrip() {
        let dir = super::tempfile_dir("pcd_spike_compressed");
        let path = dir.join("spike_compressed.pcd");

        let frame = Frame {
            stamp_micros: 0,
            sequence: 0,
            frame_id: "test".into(),
            width: 3,
            height: 1,
            is_dense: true,
            points: vec![
                PointXyzir {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                    intensity: 0.5,
                    ring: 1,
                },
                PointXyzir {
                    x: 4.0,
                    y: 5.0,
                    z: 6.0,
                    intensity: 0.6,
                    ring: 2,
                },
                PointXyzir {
                    x: 7.0,
                    y: 8.0,
                    z: 9.0,
                    intensity: 0.7,
                    ring: 3,
                },
            ],
        };

        write_pcd_spike(
            &path,
            &frame,
            DataKind::BinaryCompressed,
            ViewPoint::default(),
        )
        .unwrap();
        let points = read_pcd_spike(&path).unwrap();

        assert_eq!(points.len(), 3);
        let file_size = fs::metadata(&path).unwrap().len();
        println!(
            "binary_compressed file size: {} bytes ({} points)",
            file_size, 3
        );
    }

    #[test]
    fn spike_organized_dimensions() {
        let dir = super::tempfile_dir("pcd_spike_organized");
        let path = dir.join("spike_organized.pcd");

        // 3 columns × 2 rows = 6 points
        let frame = Frame {
            stamp_micros: 0,
            sequence: 0,
            frame_id: "test".into(),
            width: 3,
            height: 2,
            is_dense: true,
            points: (0..6)
                .map(|i| PointXyzir {
                    x: i as f32,
                    y: 0.0,
                    z: 0.0,
                    intensity: 1.0,
                    ring: i as u16,
                })
                .collect(),
        };

        write_pcd_spike(&path, &frame, DataKind::Binary, ViewPoint::default()).unwrap();

        // Read header to verify dimensions
        let meta_only = DynReader::open(&path).unwrap();
        let meta = meta_only.meta();
        assert_eq!(meta.width, 3);
        assert_eq!(meta.height, 2);
        assert_eq!(meta.num_points, 6);

        let points = read_pcd_spike(&path).unwrap();
        assert_eq!(points.len(), 6);
    }

    #[test]
    fn spike_viewpoint_roundtrip() {
        let dir = super::tempfile_dir("pcd_spike_viewpoint");
        let path = dir.join("spike_viewpoint.pcd");

        let frame = Frame {
            stamp_micros: 0,
            sequence: 0,
            frame_id: "test".into(),
            width: 1,
            height: 1,
            is_dense: true,
            points: vec![PointXyzir {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                intensity: 1.0,
                ring: 0,
            }],
        };

        let vp = ViewPoint {
            tx: 10.0,
            ty: 20.0,
            tz: 30.0,
            qw: 1.0,
            qx: 0.0,
            qy: 0.0,
            qz: 0.0,
        };

        write_pcd_spike(&path, &frame, DataKind::Binary, vp).unwrap();

        let reader = DynReader::open(&path).unwrap();
        let read_vp = reader.meta().viewpoint.clone();
        assert!((read_vp.tx - 10.0).abs() < 0.001);
        assert!((read_vp.ty - 20.0).abs() < 0.001);
        assert!((read_vp.tz - 30.0).abs() < 0.001);
    }

    #[test]
    fn spike_file_size_comparison() {
        let dir = super::tempfile_dir("pcd_spike_size");
        // Simulate a typical M8 frame: ~50k points organized as 400×125
        let n = 50_000usize;
        let width = 400;
        let height = n / width;

        let frame = Frame {
            stamp_micros: 0,
            sequence: 0,
            frame_id: "test".into(),
            width,
            height,
            is_dense: true,
            points: (0..n)
                .map(|i| PointXyzir {
                    x: i as f32 * 0.01,
                    y: (i as f32 * 0.02).sin(),
                    z: (i as f32 * 0.005).cos(),
                    intensity: (i % 256) as f32 / 255.0,
                    ring: (i % 128) as u16,
                })
                .collect(),
        };

        let path_bin = dir.join("size_binary.pcd");
        write_pcd_spike(&path_bin, &frame, DataKind::Binary, ViewPoint::default()).unwrap();
        let size_bin = fs::metadata(&path_bin).unwrap().len();

        let path_cmp = dir.join("size_compressed.pcd");
        write_pcd_spike(
            &path_cmp,
            &frame,
            DataKind::BinaryCompressed,
            ViewPoint::default(),
        )
        .unwrap();
        let size_cmp = fs::metadata(&path_cmp).unwrap().len();

        println!(
            "binary: {} bytes ({} per point), compressed: {} bytes ({} per point), ratio: {:.2}",
            size_bin,
            size_bin as f64 / n as f64,
            size_cmp,
            size_cmp as f64 / n as f64,
            size_cmp as f64 / size_bin as f64
        );

        // binary should be roughly 18 * n + header (each point = 4×4 + 2 = 18 bytes)
        let expected_binary_payload = (18 * n) as u64;
        assert!(size_bin > expected_binary_payload, "binary too small");
    }
}
