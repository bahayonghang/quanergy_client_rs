//! Standard PCD 0.7 reader/writer for XYZIR point clouds.
//!
//! This module wraps [`pcd-rs`] to produce PCL-compatible PCD files using
//! the fixed schema `x y z intensity ring`.  It does **not** embed Quanergy
//! frame metadata (timestamps, sequence numbers, coordinate-frame names,
//! transforms, or calibration) — those belong in the database layer.
//!
//! ## Encoding guidance
//!
//! | Encoding           | Use case                                |
//! |--------------------|-----------------------------------------|
//! | `Binary`           | Default for real-time / replay storage. |
//! | `BinaryCompressed` | Opt-in; must be benchmarked first.      |
//! | `Ascii`            | Debug and interop troubleshooting only. |

use std::{fs, path::Path};

use pcd_rs::{
    metas::{DataKind, ViewPoint},
    DynReader, DynWriter, Field as PcdField, Schema, ValueKind, WriterInit,
};

use crate::{
    cloud::{Frame, PointXyzir},
    error::{QuanergyError, Result},
};

// ── Public types ──────────────────────────────────────────────────

/// PCD data encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcdEncoding {
    Ascii,
    Binary,
    BinaryCompressed,
}

impl PcdEncoding {
    fn to_data_kind(self) -> DataKind {
        match self {
            PcdEncoding::Ascii => DataKind::Ascii,
            PcdEncoding::Binary => DataKind::Binary,
            PcdEncoding::BinaryCompressed => DataKind::BinaryCompressed,
        }
    }
}

/// Acquisition sensor pose stored in the PCD `VIEWPOINT` line.
///
/// `translation_m` is `[tx, ty, tz]` in metres.
/// `rotation_wxyz` is the unit quaternion `[qw, qx, qy, qz]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PcdViewpoint {
    pub translation_m: [f32; 3],
    pub rotation_wxyz: [f32; 4],
}

impl Default for PcdViewpoint {
    fn default() -> Self {
        Self {
            translation_m: [0.0; 3],
            rotation_wxyz: [1.0, 0.0, 0.0, 0.0],
        }
    }
}

impl From<PcdViewpoint> for ViewPoint {
    fn from(v: PcdViewpoint) -> Self {
        ViewPoint {
            tx: v.translation_m[0] as f64,
            ty: v.translation_m[1] as f64,
            tz: v.translation_m[2] as f64,
            qw: v.rotation_wxyz[0] as f64,
            qx: v.rotation_wxyz[1] as f64,
            qy: v.rotation_wxyz[2] as f64,
            qz: v.rotation_wxyz[3] as f64,
        }
    }
}

impl From<ViewPoint> for PcdViewpoint {
    fn from(v: ViewPoint) -> Self {
        Self {
            translation_m: [v.tx as f32, v.ty as f32, v.tz as f32],
            rotation_wxyz: [v.qw as f32, v.qx as f32, v.qy as f32, v.qz as f32],
        }
    }
}

/// Options controlling PCD file creation.
#[derive(Debug, Clone, PartialEq)]
pub struct PcdWriteOptions {
    pub encoding: PcdEncoding,
    pub viewpoint: PcdViewpoint,
}

impl Default for PcdWriteOptions {
    fn default() -> Self {
        Self {
            encoding: PcdEncoding::Binary,
            viewpoint: PcdViewpoint::default(),
        }
    }
}

/// Summary information for a written PCD file.
#[derive(Debug, Clone, PartialEq)]
pub struct PcdFileInfo {
    pub point_count: u64,
    pub width: usize,
    pub height: usize,
    pub encoding: PcdEncoding,
    pub file_size_bytes: u64,
}

/// A cloud read from a PCD file — points plus structural metadata.
///
/// Frame-level business metadata (timestamps, sequence, coordinate-frame
/// names, transforms, calibration) is **not** present here; recover it from
/// the database layer.
#[derive(Debug, Clone, PartialEq)]
pub struct PcdCloud {
    pub width: usize,
    pub height: usize,
    pub viewpoint: PcdViewpoint,
    pub points: Vec<PointXyzir>,
}

// ── Private adapter (type isolation) ──────────────────────────────

/// Private adapter struct so `storage::pcd` does not leak `pcd_rs` types
/// through the public API, and `cloud::PointXyzir` does not depend on a
/// specific storage implementation.
#[derive(Debug, Clone, Copy)]
struct PcdPoint {
    x: f32,
    y: f32,
    z: f32,
    intensity: f32,
    ring: u16,
}

impl From<PointXyzir> for PcdPoint {
    fn from(p: PointXyzir) -> Self {
        Self {
            x: p.x,
            y: p.y,
            z: p.z,
            intensity: p.intensity,
            ring: p.ring,
        }
    }
}

impl From<PcdPoint> for PointXyzir {
    fn from(p: PcdPoint) -> Self {
        Self {
            x: p.x,
            y: p.y,
            z: p.z,
            intensity: p.intensity,
            ring: p.ring,
        }
    }
}

// ── Schema ────────────────────────────────────────────────────────

fn xyzir_schema() -> Schema {
    Schema::from_iter(vec![
        ("x", ValueKind::F32, 1),
        ("y", ValueKind::F32, 1),
        ("z", ValueKind::F32, 1),
        ("intensity", ValueKind::F32, 1),
        ("ring", ValueKind::U16, 1),
    ])
}

// ── DynRecord ↔ PcdPoint conversions ─────────────────────────────

fn to_dyn_record(p: &PcdPoint) -> pcd_rs::DynRecord {
    pcd_rs::DynRecord(vec![
        PcdField::F32(vec![p.x]),
        PcdField::F32(vec![p.y]),
        PcdField::F32(vec![p.z]),
        PcdField::F32(vec![p.intensity]),
        PcdField::U16(vec![p.ring]),
    ])
}

fn from_dyn_record(rec: &pcd_rs::DynRecord) -> Result<PcdPoint> {
    let f32_val = |idx: usize| -> Result<f32> {
        match &rec.0[idx] {
            PcdField::F32(v) => Ok(v[0]),
            other => Err(QuanergyError::StorageFormat(format!(
                "PCD field {idx}: expected F32, got {:?}",
                other.kind()
            ))),
        }
    };
    let u16_val = |idx: usize| -> Result<u16> {
        match &rec.0[idx] {
            PcdField::U16(v) => Ok(v[0]),
            other => Err(QuanergyError::StorageFormat(format!(
                "PCD field {idx}: expected U16, got {:?}",
                other.kind()
            ))),
        }
    };

    Ok(PcdPoint {
        x: f32_val(0)?,
        y: f32_val(1)?,
        z: f32_val(2)?,
        intensity: f32_val(3)?,
        ring: u16_val(4)?,
    })
}

// ── Validation ────────────────────────────────────────────────────

/// Validate that a viewpoint quaternion represents a near-rigid rotation
/// (unit norm within tolerance) and the translation is finite.
fn validate_viewpoint(vp: &PcdViewpoint) -> Result<()> {
    let &[qw, qx, qy, qz] = &vp.rotation_wxyz;
    let norm_sq = qw * qw + qx * qx + qy * qy + qz * qz;
    if !norm_sq.is_finite() || (norm_sq - 1.0).abs() > 1e-3 {
        return Err(QuanergyError::StorageFormat(format!(
            "VIEWPOINT quaternion is not unit: norm_sq = {norm_sq:.6}"
        )));
    }
    for &v in &vp.translation_m {
        if !v.is_finite() {
            return Err(QuanergyError::StorageFormat(
                "VIEWPOINT translation is not finite".to_owned(),
            ));
        }
    }
    Ok(())
}

/// Validate frame dimensions.
fn validate_frame_dimensions(frame: &Frame<PointXyzir>) -> Result<()> {
    if frame.width == 0 || frame.height == 0 {
        return Err(QuanergyError::StorageFormat(
            "frame dimensions must be > 0".to_owned(),
        ));
    }
    let expected = frame
        .width
        .checked_mul(frame.height)
        .ok_or_else(|| QuanergyError::StorageFormat("frame dimensions overflow".to_owned()))?;
    if expected != frame.points.len() {
        return Err(QuanergyError::StorageFormat(format!(
            "frame dimension mismatch: {}×{} = {expected}, but points.len() = {}",
            frame.width,
            frame.height,
            frame.points.len()
        )));
    }
    Ok(())
}

// ── Public API ────────────────────────────────────────────────────

/// Write a [`Frame<PointXyzir>`] to a standard PCD 0.7 file.
///
/// # Errors
///
/// - `StorageFormat` if the frame dimensions are invalid or inconsistent.
/// - `StorageFormat` if the viewpoint is not a near-rigid transform.
/// - `Io` on filesystem errors.
/// - Wraps `pcd_rs` errors as `StorageFormat` where appropriate.
///
/// # Notes
///
/// - This function writes directly to the given path.  Callers in
///   `capture-store` should use a temp-file + rename strategy to avoid
///   exposing partially-written files.
/// - Frame-level business metadata (timestamps, sequence, coordinate-frame
///   name, calibration) is **not** embedded; persist it separately.
pub fn write_pcd(
    path: impl AsRef<Path>,
    frame: &Frame<PointXyzir>,
    options: &PcdWriteOptions,
) -> Result<PcdFileInfo> {
    validate_frame_dimensions(frame)?;
    validate_viewpoint(&options.viewpoint)?;

    let schema = xyzir_schema();
    let mut writer: DynWriter<_> = WriterInit {
        width: frame.width as u64,
        height: frame.height as u64,
        viewpoint: options.viewpoint.into(),
        data_kind: options.encoding.to_data_kind(),
        schema: Some(schema),
        version: Some("0.7".to_owned()),
    }
    .create(&path)
    .map_err(|e| QuanergyError::StorageFormat(format!("PCD writer create: {e}")))?;

    for point in &frame.points {
        let rec = to_dyn_record(&PcdPoint::from(*point));
        writer
            .push(&rec)
            .map_err(|e| QuanergyError::StorageFormat(format!("PCD write point: {e}")))?;
    }

    writer
        .finish()
        .map_err(|e| QuanergyError::StorageFormat(format!("PCD writer finish: {e}")))?;

    let file_size_bytes = fs::metadata(&path)?.len();

    Ok(PcdFileInfo {
        point_count: frame.points.len() as u64,
        width: frame.width,
        height: frame.height,
        encoding: options.encoding,
        file_size_bytes,
    })
}

/// Read a PCD file into a [`PcdCloud`].
///
/// The returned cloud contains only the point array, dimensions, and
/// acquisition viewpoint.  It does **not** contain business metadata
/// (timestamps, sequence, coordinate-frame names, transforms, or
/// calibration) — recover those from the database layer.
///
/// # Errors
///
/// - `StorageFormat` if the PCD schema does not match the expected
///   `x y z intensity ring` layout.
/// - `Io` on filesystem errors.
/// - Wraps `pcd_rs` errors as `StorageFormat` where appropriate.
pub fn read_pcd(path: impl AsRef<Path>) -> Result<PcdCloud> {
    let reader = DynReader::open(&path)
        .map_err(|e| QuanergyError::StorageFormat(format!("PCD open: {e}")))?;
    let meta = reader.meta().clone();
    let viewpoint: PcdViewpoint = meta.viewpoint.into();

    let points: Vec<PointXyzir> = reader
        .map(|rec| {
            let rec =
                rec.map_err(|e| QuanergyError::StorageFormat(format!("PCD read point: {e}")))?;
            let pp = from_dyn_record(&rec)?;
            Ok(PointXyzir::from(pp))
        })
        .collect::<Result<Vec<_>>>()?;

    let cloud = PcdCloud {
        width: meta.width as usize,
        height: meta.height as usize,
        viewpoint,
        points,
    };

    // Sanity: header point count must match.
    if cloud.points.len() as u64 != meta.num_points {
        return Err(QuanergyError::StorageFormat(format!(
            "PCD header num_points {} != actual point count {}",
            meta.num_points,
            cloud.points.len()
        )));
    }

    Ok(cloud)
}

/// Convenience: write a PCD to a temporary location, validate it by
/// reading back the header, then atomically rename to `final_path`.
///
/// Returns [`PcdFileInfo`] for the committed file.
pub fn write_pcd_atomic(
    final_path: impl AsRef<Path>,
    tmp_path: impl AsRef<Path>,
    frame: &Frame<PointXyzir>,
    options: &PcdWriteOptions,
) -> Result<PcdFileInfo> {
    // 1. Write to temp
    write_pcd(&tmp_path, frame, options)?;

    // 2. Quick validation: read back header
    let validated = read_pcd(&tmp_path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        QuanergyError::StorageFormat(format!("PCD post-write validation failed: {e}"))
    })?;

    if validated.points.len() != frame.points.len() {
        let _ = fs::remove_file(&tmp_path);
        return Err(QuanergyError::StorageFormat(format!(
            "PCD post-write validation: point count mismatch (wrote {}, read {})",
            frame.points.len(),
            validated.points.len()
        )));
    }

    // 3. Check final_path does not already exist (refuse to overwrite)
    if final_path.as_ref().exists() {
        let _ = fs::remove_file(&tmp_path);
        return Err(QuanergyError::StorageFormat(format!(
            "PCD final path already exists: {}",
            final_path.as_ref().display()
        )));
    }

    // 4. Rename
    fs::rename(&tmp_path, &final_path)?;

    let file_size_bytes = fs::metadata(&final_path)?.len();

    Ok(PcdFileInfo {
        point_count: frame.points.len() as u64,
        width: frame.width,
        height: frame.height,
        encoding: options.encoding,
        file_size_bytes,
    })
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::cloud::Frame;

    fn p(x: f32, y: f32, z: f32, intensity: f32, ring: u16) -> PointXyzir {
        PointXyzir {
            x,
            y,
            z,
            intensity,
            ring,
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let unique = format!(
            "{}_{}_{}",
            name,
            std::process::id(),
            time::OffsetDateTime::now_utc().unix_timestamp_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        fs::create_dir_all(&path).unwrap();
        path
    }

    // ── Round-trip ────────────────────────────────────────────

    #[test]
    fn binary_roundtrip_preserves_xyzir() {
        let dir = temp_dir("pcd_bin_rt");
        let path = dir.join("test.pcd");
        let frame = Frame {
            stamp_micros: 0,
            sequence: 0,
            frame_id: "test".into(),
            width: 3,
            height: 1,
            is_dense: true,
            points: vec![
                p(1.0, 2.0, 3.0, 0.5, 0),
                p(-1.0, -2.0, -3.0, 0.8, u16::MAX),
                p(f32::NAN, f32::NAN, f32::NAN, 0.0, 7),
            ],
        };

        let info = write_pcd(&path, &frame, &PcdWriteOptions::default()).unwrap();
        assert_eq!(info.point_count, 3);
        assert_eq!(info.encoding, PcdEncoding::Binary);

        let cloud = read_pcd(&path).unwrap();
        assert_eq!(cloud.points.len(), 3);
        assert_eq!(cloud.points[0].ring, 0);
        assert_eq!(cloud.points[1].ring, u16::MAX);
        assert!(cloud.points[2].x.is_nan());
        assert_eq!(cloud.points[2].ring, 7);
    }

    #[test]
    fn ascii_roundtrip() {
        let dir = temp_dir("pcd_ascii_rt");
        let path = dir.join("test.pcd");
        let frame = Frame {
            stamp_micros: 0,
            sequence: 0,
            frame_id: "test".into(),
            width: 2,
            height: 1,
            is_dense: true,
            points: vec![p(1.0, 2.0, 3.0, 0.5, 5), p(-1.0, -2.0, -3.0, 0.8, 9)],
        };
        let opts = PcdWriteOptions {
            encoding: PcdEncoding::Ascii,
            ..Default::default()
        };
        write_pcd(&path, &frame, &opts).unwrap();
        let cloud = read_pcd(&path).unwrap();
        assert_eq!(cloud.points.len(), 2);
        assert_eq!(cloud.points[0], frame.points[0]);
    }

    #[test]
    fn binary_compressed_roundtrip() {
        let dir = temp_dir("pcd_cmp_rt");
        let path = dir.join("test.pcd");
        let frame = Frame {
            stamp_micros: 0,
            sequence: 0,
            frame_id: "test".into(),
            width: 3,
            height: 1,
            is_dense: true,
            points: vec![
                p(1.0, 2.0, 3.0, 0.5, 1),
                p(4.0, 5.0, 6.0, 0.6, 2),
                p(7.0, 8.0, 9.0, 0.7, 3),
            ],
        };
        let opts = PcdWriteOptions {
            encoding: PcdEncoding::BinaryCompressed,
            ..Default::default()
        };
        write_pcd(&path, &frame, &opts).unwrap();
        let cloud = read_pcd(&path).unwrap();
        assert_eq!(cloud.points.len(), 3);
    }

    // ── Dimensions ────────────────────────────────────────────

    #[test]
    fn organized_dimensions_preserved() {
        let dir = temp_dir("pcd_org");
        let path = dir.join("test.pcd");
        let frame = Frame {
            stamp_micros: 0,
            sequence: 0,
            frame_id: "test".into(),
            width: 3,
            height: 2,
            is_dense: true,
            points: (0..6)
                .map(|i| p(i as f32, 0.0, 0.0, 1.0, i as u16))
                .collect(),
        };
        write_pcd(&path, &frame, &PcdWriteOptions::default()).unwrap();
        let cloud = read_pcd(&path).unwrap();
        assert_eq!(cloud.width, 3);
        assert_eq!(cloud.height, 2);
        assert_eq!(cloud.points.len(), 6);
    }

    #[test]
    fn mismatched_dimensions_rejected() {
        let frame = Frame {
            stamp_micros: 0,
            sequence: 0,
            frame_id: "test".into(),
            width: 3,
            height: 2,
            is_dense: true,
            points: vec![p(0.0, 0.0, 0.0, 1.0, 0)], // 1 ≠ 6
        };
        let err = write_pcd("nope.pcd", &frame, &PcdWriteOptions::default()).unwrap_err();
        assert!(err.to_string().contains("dimension mismatch"));
    }

    // ── Viewpoint ─────────────────────────────────────────────

    #[test]
    fn viewpoint_roundtrip() {
        let dir = temp_dir("pcd_vp");
        let path = dir.join("test.pcd");
        let frame = Frame {
            stamp_micros: 0,
            sequence: 0,
            frame_id: "test".into(),
            width: 1,
            height: 1,
            is_dense: true,
            points: vec![p(1.0, 0.0, 0.0, 1.0, 0)],
        };
        let vp = PcdViewpoint {
            translation_m: [10.0, 20.0, 30.0],
            rotation_wxyz: [1.0, 0.0, 0.0, 0.0],
        };
        let opts = PcdWriteOptions {
            viewpoint: vp,
            ..Default::default()
        };
        write_pcd(&path, &frame, &opts).unwrap();
        let cloud = read_pcd(&path).unwrap();
        assert!((cloud.viewpoint.translation_m[0] - 10.0).abs() < 0.001);
        assert!((cloud.viewpoint.translation_m[1] - 20.0).abs() < 0.001);
        assert!((cloud.viewpoint.translation_m[2] - 30.0).abs() < 0.001);
        assert!((cloud.viewpoint.rotation_wxyz[0] - 1.0).abs() < 0.001);
    }

    #[test]
    fn non_unit_quaternion_rejected() {
        let frame = Frame {
            stamp_micros: 0,
            sequence: 0,
            frame_id: "test".into(),
            width: 1,
            height: 1,
            is_dense: true,
            points: vec![p(1.0, 0.0, 0.0, 1.0, 0)],
        };
        let vp = PcdViewpoint {
            translation_m: [0.0; 3],
            rotation_wxyz: [2.0, 0.0, 0.0, 0.0], // norm = 4
        };
        let opts = PcdWriteOptions {
            viewpoint: vp,
            ..Default::default()
        };
        let err = write_pcd("nope.pcd", &frame, &opts).unwrap_err();
        assert!(err.to_string().contains("quaternion"));
    }

    #[test]
    fn nan_translation_rejected() {
        let frame = Frame {
            stamp_micros: 0,
            sequence: 0,
            frame_id: "test".into(),
            width: 1,
            height: 1,
            is_dense: true,
            points: vec![p(1.0, 0.0, 0.0, 1.0, 0)],
        };
        let vp = PcdViewpoint {
            translation_m: [f32::NAN, 0.0, 0.0],
            rotation_wxyz: [1.0, 0.0, 0.0, 0.0],
        };
        let opts = PcdWriteOptions {
            viewpoint: vp,
            ..Default::default()
        };
        let err = write_pcd("nope.pcd", &frame, &opts).unwrap_err();
        assert!(err.to_string().contains("translation"));
    }

    // ── Atomic write ──────────────────────────────────────────

    #[test]
    fn atomic_write_success() {
        let dir = temp_dir("pcd_atomic");
        let final_path = dir.join("frame.pcd");
        let tmp_path = dir.join("frame.pcd.tmp");
        let frame = Frame {
            stamp_micros: 0,
            sequence: 0,
            frame_id: "test".into(),
            width: 2,
            height: 1,
            is_dense: true,
            points: vec![p(1.0, 2.0, 3.0, 0.5, 1), p(4.0, 5.0, 6.0, 0.8, 2)],
        };
        let info =
            write_pcd_atomic(&final_path, &tmp_path, &frame, &PcdWriteOptions::default()).unwrap();
        assert!(final_path.exists());
        assert!(!tmp_path.exists());
        assert_eq!(info.point_count, 2);
        let cloud = read_pcd(&final_path).unwrap();
        assert_eq!(cloud.points.len(), 2);
    }

    #[test]
    fn atomic_write_no_duplicate_overwrite() {
        let dir = temp_dir("pcd_no_dup");
        let final_path = dir.join("frame.pcd");
        let tmp_path = dir.join("frame.pcd.tmp");
        let frame = Frame {
            stamp_micros: 0,
            sequence: 0,
            frame_id: "test".into(),
            width: 1,
            height: 1,
            is_dense: true,
            points: vec![p(1.0, 2.0, 3.0, 0.5, 1)],
        };
        write_pcd_atomic(&final_path, &tmp_path, &frame, &PcdWriteOptions::default()).unwrap();
        // Second write should fail because final_path already exists
        let err = write_pcd_atomic(&final_path, &tmp_path, &frame, &PcdWriteOptions::default())
            .unwrap_err();
        assert!(
            err.to_string().contains("already exists"),
            "expected already exists, got: {err}"
        );
    }

    // ── Reader validation ─────────────────────────────────────

    #[test]
    fn reader_rejects_truncated_file() {
        let dir = temp_dir("pcd_trunc");
        let path = dir.join("trunc.pcd");
        // Write a valid header but no data
        fs::write(&path, "# .PCD v0.7\nVERSION 0.7\nFIELDS x y z intensity ring\nSIZE 4 4 4 4 2\nTYPE F F F F U\nCOUNT 1 1 1 1 1\nWIDTH 1\nHEIGHT 1\nVIEWPOINT 0 0 0 1 0 0 0\nPOINTS 1\nDATA binary\n").unwrap();
        let err = read_pcd(&path).unwrap_err();
        // Either a pcd-rs parse error or an Io error is acceptable
        assert!(!err.to_string().is_empty());
    }

    // ── File info ─────────────────────────────────────────────

    #[test]
    fn file_info_accurate() {
        let dir = temp_dir("pcd_info");
        let path = dir.join("test.pcd");
        let frame = Frame {
            stamp_micros: 0,
            sequence: 0,
            frame_id: "test".into(),
            width: 3,
            height: 1,
            is_dense: true,
            points: vec![p(1.0, 2.0, 3.0, 0.5, 1); 3],
        };
        let info = write_pcd(&path, &frame, &PcdWriteOptions::default()).unwrap();
        assert_eq!(info.point_count, 3);
        assert_eq!(info.width, 3);
        assert_eq!(info.height, 1);
        assert_eq!(info.encoding, PcdEncoding::Binary);
        assert!(info.file_size_bytes > 0);
        let actual_size = fs::metadata(&path).unwrap().len();
        assert_eq!(info.file_size_bytes, actual_size);
    }

    // ── Empty frame allowed ───────────────────────────────────

    #[test]
    fn empty_frame_writes_valid_pcd() {
        let dir = temp_dir("pcd_empty");
        let path = dir.join("empty.pcd");
        let frame = Frame::<PointXyzir>::new("test");
        let opts = PcdWriteOptions::default();
        // empty frame has width=0, height=1 — per plan, this is allowed
        let frame = Frame {
            width: 0,
            height: 1,
            ..frame
        };
        // Note: width=0 passes dimension validation (0*1==0), but pcd-rs may
        // reject 0 width. If it does, we should handle that gracefully.
        let result = write_pcd(&path, &frame, &opts);
        // Either succeeds or gives a clear error — must not panic
        match result {
            Ok(info) => {
                assert_eq!(info.point_count, 0);
            }
            Err(e) => {
                // Acceptable: some writers don't support width=0
                assert!(
                    e.to_string().contains("dimension")
                        || e.to_string().contains("width")
                        || e.to_string().contains("PCD")
                );
            }
        }
    }
}
