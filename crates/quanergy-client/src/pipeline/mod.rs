use tracing::warn;

mod dispatch;
mod helpers;
mod m_series;
mod packet_01;

#[cfg(test)]
mod tests;

use dispatch::ParserDispatch;

use crate::{
    calibration::{apply_correction, AutoCalibrationConfig, AutoCalibrator, EncoderCorrection},
    cloud::{Frame, PointXyzir},
    config::{EncoderMode, PipelineConfig},
    error::Result,
    filters::DistanceFilter,
    protocol::RawPacket,
};

#[derive(Debug, Default, Clone)]
pub struct PipelineCounters {
    pub packets_seen: u64,
    pub frames_emitted: u64,
    pub bad_packets: u64,
    pub dropped_packets: u64,
}

pub struct SensorPipeline {
    config: PipelineConfig,
    parser: ParserDispatch,
    distance_filter: DistanceFilter,
    auto_calibrator: Option<AutoCalibrator>,
    manual_correction: Option<EncoderCorrection>,
    counters: PipelineCounters,
}

impl SensorPipeline {
    pub fn new(config: PipelineConfig) -> Result<Self> {
        let manual_correction = match config.encoder_mode {
            EncoderMode::Manual { amplitude, phase } => {
                Some(EncoderCorrection::new(amplitude, phase)?)
            }
            _ => None,
        };
        let auto_calibrator = match config.encoder_mode {
            EncoderMode::Automatic => {
                let auto_config = AutoCalibrationConfig {
                    frame_rate: config.frame_rate,
                    ..AutoCalibrationConfig::default()
                };
                Some(AutoCalibrator::new(auto_config))
            }
            _ => None,
        };

        Ok(Self {
            parser: ParserDispatch::new(&config)?,
            distance_filter: DistanceFilter {
                min: config.min_distance,
                max: config.max_distance,
            },
            config,
            auto_calibrator,
            manual_correction,
            counters: PipelineCounters::default(),
        })
    }

    pub fn reset_calibration(&mut self) {
        if let Some(calibrator) = &mut self.auto_calibrator {
            calibrator.reset();
        }
    }

    pub fn counters(&self) -> &PipelineCounters {
        &self.counters
    }

    pub fn process_raw(&mut self, packet: &RawPacket) -> Result<Vec<Frame<PointXyzir>>> {
        self.process_packet_bytes(&packet.bytes)
    }

    pub fn process_packet_bytes(&mut self, packet: &[u8]) -> Result<Vec<Frame<PointXyzir>>> {
        self.counters.packets_seen += 1;
        let frames = match self.parser.parse(packet) {
            Ok(frames) => frames,
            Err(error) if self.config.strict => return Err(error),
            Err(error) => {
                self.counters.bad_packets += 1;
                warn!(%error, "dropping bad packet");
                return Ok(Vec::new());
            }
        };

        let mut output = Vec::with_capacity(frames.len());
        for mut frame in frames {
            if let Some(calibrator) = &mut self.auto_calibrator {
                if let Some(correction) = calibrator.process_frame(&frame)? {
                    apply_correction(&mut frame, correction);
                } else {
                    continue;
                }
            } else if let Some(correction) = self.manual_correction {
                apply_correction(&mut frame, correction);
            }

            self.distance_filter.apply(&mut frame);
            self.config.ring_filter.apply(&mut frame);
            let xyz = frame.to_xyzir();
            self.counters.frames_emitted += 1;
            output.push(xyz);
        }

        Ok(output)
    }
}
