//! Station extrinsic calibration using Arun's method (SVD).
//!
//! Input: CSV of corresponding 3D points:
//!   sensor_x,sensor_y,sensor_z,station_x,station_y,station_z
//!
//! Output: TOML `[scanner.extrinsic]` block with the solved 4×4 matrix,
//! plus RMS error, max error, and per-target residuals.

use std::{
    fs,
    io::{self, BufRead},
    path::PathBuf,
};

use clap::Parser;
use nalgebra::{Matrix3, Vector3, SVD};
use quanergy_client::transform::{validate_rigid_matrix, RigidTransformValidation};

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Cli {
    /// CSV file with corresponding points (sensor_x,sensor_y,sensor_z,station_x,station_y,station_z).
    input: PathBuf,

    /// Calibration ID for the output TOML.
    #[arg(long, default_value = "field-calibration")]
    calibration_id: String,

    /// Calibration method description.
    #[arg(long, default_value = "arun_svd")]
    method: String,

    /// Person performing the calibration.
    #[arg(long, default_value = "")]
    calibrated_by: String,

    /// Output TOML file (default: stdout).
    #[arg(long, short = 'o')]
    output: Option<PathBuf>,
}

#[derive(Debug)]
struct CorrespondingPoint {
    sensor: [f64; 3],
    station: [f64; 3],
}

fn main() {
    let cli = Cli::parse();
    if let Err(error) = run(cli) {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let points = load_csv(&cli.input)?;
    if points.len() < 3 {
        return Err("need at least 3 non-collinear corresponding points".into());
    }

    let (rotation, translation, rms_error, max_error, residuals) = solve_arun(&points)?;

    // Convert to f32 and validate
    let mut mat_4x4 = [[0.0f32; 4]; 4];
    for i in 0..3 {
        for j in 0..3 {
            mat_4x4[i][j] = rotation[(i, j)] as f32;
        }
        mat_4x4[i][3] = translation[i] as f32;
    }
    mat_4x4[3][3] = 1.0;

    let validation = RigidTransformValidation::default();
    if let Err(e) = validate_rigid_matrix(mat_4x4, &validation) {
        return Err(format!("solved matrix failed rigid-body validation: {e}").into());
    }

    let det = rotation.determinant();
    let hand = if det > 0.0 {
        "right-handed"
    } else {
        "LEFT-HANDED (mirror!)"
    };

    let now = chrono_like_now();

    let toml = format!(
        r##"# Extrinsic calibration produced by station-calibrate
# Method: {method} (det = {det:.4}, {hand})
# Calibrated at: {now}
# RMS error: {rms_error:.6} m
# Max error: {max_error:.6} m
# Points: {n_points}
{residuals_section}
[scanner.extrinsic]
kind = "matrix"
extrinsic_id = "{calib_id}"
matrix_4x4 = [
  [{r00:>10.6}, {r01:>10.6}, {r02:>10.6}, {tx:>10.6}],
  [{r10:>10.6}, {r11:>10.6}, {r12:>10.6}, {ty:>10.6}],
  [{r20:>10.6}, {r21:>10.6}, {r22:>10.6}, {tz:>10.6}],
  [    0.0,     0.0,     0.0,     1.0],
]

[scanner.calibration]
calibrated_at = "{now}"
method = "{method}"
calibrated_by = "{calibrated_by}"
rms_error_m = {rms_error:.6}
max_error_m = {max_error:.6}
notes = "Field calibration from {n_points} corresponding points."
"##,
        method = cli.method,
        calib_id = cli.calibration_id,
        calibrated_by = cli.calibrated_by,
        now = now,
        det = det,
        hand = hand,
        n_points = points.len(),
        rms_error = rms_error,
        max_error = max_error,
        residuals_section = format_residuals(&residuals, &points),
        r00 = mat_4x4[0][0],
        r01 = mat_4x4[0][1],
        r02 = mat_4x4[0][2],
        tx = mat_4x4[0][3],
        r10 = mat_4x4[1][0],
        r11 = mat_4x4[1][1],
        r12 = mat_4x4[1][2],
        ty = mat_4x4[1][3],
        r20 = mat_4x4[2][0],
        r21 = mat_4x4[2][1],
        r22 = mat_4x4[2][2],
        tz = mat_4x4[2][3],
    );

    match &cli.output {
        Some(path) => fs::write(path, &toml)?,
        None => println!("{toml}"),
    }

    eprintln!(
        "calibration solved: {n} points, RMS={rms_error:.4} m, max={max_error:.4} m, det={det:.4}",
        n = points.len(),
        rms_error = rms_error,
        max_error = max_error,
        det = det,
    );
    Ok(())
}

type ArunResult = (Matrix3<f64>, Vector3<f64>, f64, f64, Vec<f64>);

fn solve_arun(points: &[CorrespondingPoint]) -> Result<ArunResult, Box<dyn std::error::Error>> {
    let n = points.len() as f64;

    // Centroids
    let mut sensor_centroid = Vector3::zeros();
    let mut station_centroid = Vector3::zeros();
    for p in points {
        sensor_centroid += Vector3::new(p.sensor[0], p.sensor[1], p.sensor[2]);
        station_centroid += Vector3::new(p.station[0], p.station[1], p.station[2]);
    }
    sensor_centroid /= n;
    station_centroid /= n;

    // Cross-covariance matrix H
    let mut h = Matrix3::zeros();
    for p in points {
        let s = Vector3::new(p.sensor[0], p.sensor[1], p.sensor[2]) - sensor_centroid;
        let t = Vector3::new(p.station[0], p.station[1], p.station[2]) - station_centroid;
        h += s * t.transpose();
    }

    // SVD of H
    let svd = SVD::new(h, true, true);
    let u = svd.u.unwrap();
    let v_t = svd.v_t.unwrap();

    // Rotation: R = V * U^T, with determinant correction
    let mut r = v_t.transpose() * u.transpose();
    if r.determinant() < 0.0 {
        let mut v_corrected = v_t.transpose();
        // Negate the last column of V
        for i in 0..3 {
            v_corrected[(i, 2)] *= -1.0;
        }
        r = v_corrected * u.transpose();
    }

    let t = station_centroid - r * sensor_centroid;

    // Compute errors
    let mut residuals = Vec::with_capacity(points.len());
    let mut sum_sq = 0.0;
    let mut max_err = 0.0f64;
    for p in points {
        let s = Vector3::new(p.sensor[0], p.sensor[1], p.sensor[2]);
        let predicted = r * s + t;
        let actual = Vector3::new(p.station[0], p.station[1], p.station[2]);
        let err = (predicted - actual).norm();
        residuals.push(err);
        sum_sq += err * err;
        if err > max_err {
            max_err = err;
        }
    }
    let rms = (sum_sq / n).sqrt();

    Ok((r, t, rms, max_err, residuals))
}

fn load_csv(path: &PathBuf) -> Result<Vec<CorrespondingPoint>, Box<dyn std::error::Error>> {
    let file = fs::File::open(path)?;
    let reader = io::BufReader::new(file);
    let mut points = Vec::new();

    for (lineno, line) in reader.lines().enumerate() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if parts.len() != 6 {
            return Err(format!(
                "line {}: expected 6 comma-separated values, got {}",
                lineno + 1,
                parts.len()
            )
            .into());
        }
        let nums: Result<Vec<f64>, _> = parts.iter().map(|s| s.parse::<f64>()).collect();
        let nums = nums.map_err(|e| format!("line {}: {}", lineno + 1, e))?;
        points.push(CorrespondingPoint {
            sensor: [nums[0], nums[1], nums[2]],
            station: [nums[3], nums[4], nums[5]],
        });
    }
    Ok(points)
}

fn format_residuals(residuals: &[f64], points: &[CorrespondingPoint]) -> String {
    let mut out = String::from("# Per-target residuals:\n");
    for (i, (r, p)) in residuals.iter().zip(points.iter()).enumerate() {
        out.push_str(&format!(
            "#   target {:>3}: sensor=({:.3},{:.3},{:.3}) station=({:.3},{:.3},{:.3}) error={:.4} m\n",
            i + 1,
            p.sensor[0], p.sensor[1], p.sensor[2],
            p.station[0], p.station[1], p.station[2],
            r,
        ));
    }
    out
}

fn chrono_like_now() -> String {
    use std::time::SystemTime;
    let ts = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{ts}")
}
