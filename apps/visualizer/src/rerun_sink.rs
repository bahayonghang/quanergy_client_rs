use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use quanergy_client::cloud::{Frame, PointXyzir};
use quanergy_client::station::StationGeometry;
use tracing::warn;

use crate::{Result, VisualizerError};

#[derive(Debug, Clone)]
pub enum RerunOutput {
    Spawn(Option<PathBuf>),
    Connect(String),
    Save(PathBuf),
}

impl Default for RerunOutput {
    fn default() -> Self {
        RerunOutput::Spawn(None)
    }
}

#[derive(Debug, Clone)]
pub struct VisualizerConfig {
    pub output: RerunOutput,
    pub max_points: usize,
}

impl Default for VisualizerConfig {
    fn default() -> Self {
        Self {
            output: RerunOutput::Spawn(None),
            max_points: 300_000,
        }
    }
}

pub trait VisualizerSink {
    /// Log a point-cloud frame at the default entity path `{frame_id}/points`.
    fn log_frame(&mut self, frame: &Frame<PointXyzir>) -> Result<()>;

    /// Log the static station scene: axes, scanner origin, Y axis, hammer centres/ROIs.
    /// This should be called once at the start of a session.
    fn log_static_station(&mut self, geometry: &StationGeometry) -> Result<()>;
}

impl RerunSink {
    /// Log a point-cloud frame at an explicit entity path.
    pub fn log_frame_to_entity(
        rec: &rerun::RecordingStream,
        entity_path: &str,
        frame: &Frame<PointXyzir>,
        max_points: usize,
    ) -> Result<()> {
        let step = display_stride(frame.points.len(), max_points);
        let points: Vec<rerun::Position3D> = frame
            .points
            .iter()
            .step_by(step)
            .map(|point| rerun::Position3D::new(point.x, point.y, point.z))
            .collect();
        let colors: Vec<rerun::Color> = frame
            .points
            .iter()
            .step_by(step)
            .map(|point| intensity_color(point.intensity))
            .collect();

        rec.set_time_sequence("frame", frame.sequence as i64);
        rec.log(
            format!("{entity_path}/points"),
            &rerun::Points3D::new(points).with_colors(colors),
        )
        .map_err(|error| VisualizerError::Rerun(error.to_string()))?;
        Ok(())
    }
}

pub struct RerunSink {
    pub(crate) rec: rerun::RecordingStream,
    pub(crate) max_points: usize,
    station_logged: bool,
}

impl RerunSink {
    pub fn new(config: &VisualizerConfig) -> Result<Self> {
        let rec = match &config.output {
            RerunOutput::Spawn(explicit_path) => match spawn_viewer(explicit_path.as_deref()) {
                Ok(rec) => rec,
                Err(spawn_error) => {
                    let fallback_path = default_fallback_path();
                    warn!(
                        "Failed to spawn Rerun Viewer ({}); \
                                 falling back to save mode: {}",
                        spawn_error,
                        fallback_path.display()
                    );
                    eprintln!(
                        "Warning: Rerun Viewer not found. \
                                 Recording to {} instead.",
                        fallback_path.display()
                    );
                    eprintln!(
                        "Install the viewer or use --rerun-viewer-path to specify its location."
                    );
                    rerun::RecordingStreamBuilder::new("quanergy_client")
                        .save(&fallback_path)
                        .map_err(|e| VisualizerError::Rerun(e.to_string()))?
                }
            },
            RerunOutput::Connect(addr) => rerun::RecordingStreamBuilder::new("quanergy_client")
                .connect_grpc_opts(normalize_connect_addr(addr))
                .map_err(|error| VisualizerError::Rerun(error.to_string()))?,
            RerunOutput::Save(path) => rerun::RecordingStreamBuilder::new("quanergy_client")
                .save(path)
                .map_err(|error| VisualizerError::Rerun(error.to_string()))?,
        };

        Ok(Self {
            rec,
            max_points: config.max_points,
            station_logged: false,
        })
    }

    pub fn save(path: impl AsRef<Path>) -> Result<Self> {
        Self::new(&VisualizerConfig {
            output: RerunOutput::Save(path.as_ref().to_path_buf()),
            ..VisualizerConfig::default()
        })
    }

    pub fn flush_blocking(&self) -> Result<()> {
        self.rec
            .flush_blocking()
            .map_err(|error| VisualizerError::Rerun(error.to_string()))
    }
}

impl VisualizerSink for RerunSink {
    fn log_frame(&mut self, frame: &Frame<PointXyzir>) -> Result<()> {
        Self::log_frame_to_entity(&self.rec, &frame.frame_id, frame, self.max_points)
    }

    fn log_static_station(&mut self, geometry: &StationGeometry) -> Result<()> {
        if self.station_logged {
            return Ok(());
        }
        self.station_logged = true;

        let rec = &self.rec;
        rec.set_time_sequence("frame", 0);

        // Origin point
        rec.log(
            "station/origin",
            &rerun::Points3D::new([rerun::Position3D::new(0.0, 0.0, 0.0)])
                .with_colors([rerun::Color::from_rgb(255, 255, 255)]),
        )
        .map_err(|e| VisualizerError::Rerun(e.to_string()))?;

        // Coordinate axes (1-meter arrows)
        log_arrow(
            rec,
            "station/axes/x",
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [255, 0, 0],
        )?;
        log_arrow(
            rec,
            "station/axes/y",
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0, 255, 0],
        )?;
        log_arrow(
            rec,
            "station/axes/z",
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0, 0, 255],
        )?;

        // Y-axis extended line
        rec.log(
            "station/y_axis",
            &rerun::Arrows3D::from_vectors([(0.0, 10.0, 0.0)])
                .with_origins([(0.0, -10.0, 0.0)])
                .with_colors([rerun::Color::from_rgb(128, 128, 128)]),
        )
        .map_err(|e| VisualizerError::Rerun(e.to_string()))?;

        // Scanner origin
        let so = geometry.scanner_origin_station;
        rec.log(
            "station/scanner/origin",
            &rerun::Points3D::new([rerun::Position3D::new(so[0], so[1], so[2])])
                .with_colors([rerun::Color::from_rgb(255, 255, 0)]),
        )
        .map_err(|e| VisualizerError::Rerun(e.to_string()))?;

        // Scanner local axes (after transform)
        let m = geometry.sensor_to_station;
        log_arrow(
            rec,
            "station/scanner/axes/x",
            [m[0][3], m[1][3], m[2][3]],
            [m[0][3] + m[0][0], m[1][3] + m[1][0], m[2][3] + m[2][0]],
            [255, 128, 128],
        )?;
        log_arrow(
            rec,
            "station/scanner/axes/y",
            [m[0][3], m[1][3], m[2][3]],
            [m[0][3] + m[0][1], m[1][3] + m[1][1], m[2][3] + m[2][1]],
            [128, 255, 128],
        )?;
        log_arrow(
            rec,
            "station/scanner/axes/z",
            [m[0][3], m[1][3], m[2][3]],
            [m[0][3] + m[0][2], m[1][3] + m[1][2], m[2][3] + m[2][2]],
            [128, 128, 255],
        )?;

        // Hammers
        for hammer in &geometry.hammers {
            let entity = format!("station/hammers/{}/center", hammer.id);
            rec.log(
                entity.as_str(),
                &rerun::Points3D::new([rerun::Position3D::new(
                    hammer.center_x_m,
                    hammer.center_y_m,
                    0.0,
                )])
                .with_colors([if hammer.enabled {
                    rerun::Color::from_rgb(0, 255, 0)
                } else {
                    rerun::Color::from_rgb(100, 100, 100)
                }]),
            )
            .map_err(|e| VisualizerError::Rerun(e.to_string()))?;

            // ROI box
            let roi = &hammer.roi;
            let half_x = (roi.max_x - roi.min_x) / 2.0;
            let half_y = (roi.max_y - roi.min_y) / 2.0;
            let half_z = (roi.max_z - roi.min_z) / 2.0;
            let cx = roi.min_x + half_x;
            let cy = roi.min_y + half_y;
            let cz = roi.min_z + half_z;

            rec.log(
                format!("station/hammers/{}/roi", hammer.id).as_str(),
                &rerun::Boxes3D::from_half_sizes([rerun::HalfSize3D::new(half_x, half_y, half_z)])
                    .with_centers([(cx, cy, cz)])
                    .with_colors([rerun::Color::from_rgb(0, 200, 200)]),
            )
            .map_err(|e| VisualizerError::Rerun(e.to_string()))?;
        }

        Ok(())
    }
}

/// Try to spawn the Rerun Viewer, checking the explicit path first,
/// then the directory next to the current executable, then PATH.
fn spawn_viewer(
    explicit_path: Option<&Path>,
) -> std::result::Result<rerun::RecordingStream, VisualizerError> {
    let resolved = resolve_viewer_path(explicit_path);
    let opts = rerun::SpawnOptions {
        executable_path: resolved,
        ..Default::default()
    };
    rerun::RecordingStreamBuilder::new("quanergy_client")
        .spawn_opts(&opts)
        .map_err(|error| VisualizerError::Rerun(error.to_string()))
}

/// Resolve the viewer executable path.
///
/// Priority:
/// 1. Explicit `--rerun-viewer-path` from the user.
/// 2. `<exe_dir>/rerun.exe` (Windows) or `<exe_dir>/rerun` if that file exists.
/// 3. `None` — let the rerun crate search PATH.
fn resolve_viewer_path(explicit_path: Option<&Path>) -> Option<String> {
    if let Some(path) = explicit_path {
        return Some(path.to_string_lossy().into_owned());
    }
    // Check next to the current executable.
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            let neighbor = dir.join(rerun_exe_name());
            if neighbor.is_file() {
                return Some(neighbor.to_string_lossy().into_owned());
            }
        }
    }
    None
}

fn rerun_exe_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "rerun.exe"
    } else {
        "rerun"
    }
}

fn default_fallback_path() -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    PathBuf::from(format!("quanergy_{ts}.rrd"))
}

fn display_stride(point_count: usize, max_points: usize) -> usize {
    if max_points == 0 || point_count <= max_points {
        1
    } else {
        point_count.div_ceil(max_points)
    }
}

fn intensity_color(intensity: f32) -> rerun::Color {
    let value = intensity.clamp(0.0, 255.0) as u8;
    rerun::Color::from_rgb(value, value, 255u8.saturating_sub(value))
}

fn log_arrow(
    rec: &rerun::RecordingStream,
    entity_path: &str,
    origin: [f32; 3],
    tip: [f32; 3],
    color: [u8; 3],
) -> std::result::Result<(), VisualizerError> {
    let vector = (tip[0] - origin[0], tip[1] - origin[1], tip[2] - origin[2]);
    rec.log(
        entity_path,
        &rerun::Arrows3D::from_vectors([vector])
            .with_origins([(origin[0], origin[1], origin[2])])
            .with_colors([rerun::Color::from_rgb(color[0], color[1], color[2])]),
    )
    .map_err(|error| VisualizerError::Rerun(error.to_string()))
}

fn normalize_connect_addr(addr: &str) -> String {
    if addr.contains("://") {
        addr.to_owned()
    } else {
        format!("rerun+http://{addr}/proxy")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_max_points_disables_downsampling() {
        assert_eq!(display_stride(1_000_000, 0), 1);
    }

    #[test]
    fn display_stride_caps_points() {
        assert_eq!(display_stride(10, 3), 4);
    }

    #[test]
    fn bare_connect_addr_is_promoted_to_rerun_url() {
        assert_eq!(
            normalize_connect_addr("127.0.0.1:9876"),
            "rerun+http://127.0.0.1:9876/proxy"
        );
        assert_eq!(
            normalize_connect_addr("rerun+http://127.0.0.1:9876/proxy"),
            "rerun+http://127.0.0.1:9876/proxy"
        );
    }
}
