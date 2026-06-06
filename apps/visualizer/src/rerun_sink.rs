use std::path::{Path, PathBuf};

use crate::{
    cloud::{Frame, PointXyzir},
    error::{QuanergyError, Result},
};

#[derive(Debug, Clone, Default)]
pub enum RerunOutput {
    #[default]
    Spawn,
    Connect(String),
    Save(PathBuf),
}

#[derive(Debug, Clone)]
pub struct VisualizerConfig {
    pub output: RerunOutput,
    pub max_points: usize,
}

impl Default for VisualizerConfig {
    fn default() -> Self {
        Self {
            output: RerunOutput::Spawn,
            max_points: 300_000,
        }
    }
}

pub trait VisualizerSink {
    fn log_frame(&mut self, frame: &Frame<PointXyzir>) -> Result<()>;
}

pub struct RerunSink {
    rec: rerun::RecordingStream,
    max_points: usize,
}

impl RerunSink {
    pub fn new(config: &VisualizerConfig) -> Result<Self> {
        let builder = rerun::RecordingStreamBuilder::new("quanergy_client");
        let rec =
            match &config.output {
                RerunOutput::Spawn => builder
                    .spawn()
                    .map_err(|error| QuanergyError::Visualizer(error.to_string()))?,
                RerunOutput::Connect(addr) => builder
                    .connect_grpc_opts(normalize_connect_addr(addr))
                    .map_err(|error| QuanergyError::Visualizer(error.to_string()))?,
                RerunOutput::Save(path) => builder
                    .save(path)
                    .map_err(|error| QuanergyError::Visualizer(error.to_string()))?,
            };

        Ok(Self {
            rec,
            max_points: config.max_points,
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
            .map_err(|error| QuanergyError::Visualizer(error.to_string()))
    }
}

impl VisualizerSink for RerunSink {
    fn log_frame(&mut self, frame: &Frame<PointXyzir>) -> Result<()> {
        let step = display_stride(frame.points.len(), self.max_points);
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

        self.rec.set_time_sequence("frame", frame.sequence as i64);
        self.rec
            .log(
                format!("{}/points", frame.frame_id),
                &rerun::Points3D::new(points).with_colors(colors),
            )
            .map_err(|error| QuanergyError::Visualizer(error.to_string()))?;
        Ok(())
    }
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
