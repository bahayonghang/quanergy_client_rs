use std::{
    f32::consts::{PI, TAU},
    time::{Duration, Instant},
};

use crate::{
    cloud::{Frame, PointHvdir},
    error::{QuanergyError, Result},
};

const FIRING_RATE: f32 = 53_828.0;
const PI_TOLERANCE: f32 = 0.01;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct EncoderCorrection {
    pub amplitude: f32,
    pub phase: f32,
}

impl EncoderCorrection {
    pub fn new(amplitude: f32, phase: f32) -> Result<Self> {
        if !(-TAU..=TAU).contains(&amplitude) || !(-TAU..=TAU).contains(&phase) {
            return Err(QuanergyError::Calibration(
                "encoder amplitude or phase out of range [-2PI, 2PI]".to_owned(),
            ));
        }
        Ok(Self { amplitude, phase })
    }

    pub fn offset(self, angle: f32) -> f32 {
        self.amplitude * (angle + self.phase).sin()
    }

    pub fn zero_offset(self) -> f32 {
        self.offset(0.0)
    }
}

impl Default for EncoderCorrection {
    fn default() -> Self {
        Self {
            amplitude: 0.0,
            phase: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AutoCalibrationConfig {
    pub frame_rate: f32,
    pub timeout: Duration,
    pub required_samples: usize,
    pub encoder_count_tolerance: usize,
    pub moving_average_period_counts: usize,
    pub phase_convergence_threshold: f32,
    pub amplitude_threshold: f32,
}

impl Default for AutoCalibrationConfig {
    fn default() -> Self {
        Self {
            frame_rate: 10.0,
            timeout: Duration::from_secs(60),
            required_samples: 13,
            encoder_count_tolerance: 200,
            moving_average_period_counts: 300,
            phase_convergence_threshold: 0.1,
            amplitude_threshold: 0.006,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AutoCalibrator {
    config: AutoCalibrationConfig,
    started_at: Option<Instant>,
    current_period: Vec<f32>,
    amplitudes: Vec<f32>,
    phases: Vec<f32>,
    last_phase: Option<f32>,
    valid_samples: usize,
    pub complete_frames: usize,
    pub incomplete_frames: usize,
    pub divergent_phase_values: usize,
    correction: Option<EncoderCorrection>,
}

impl AutoCalibrator {
    pub fn new(config: AutoCalibrationConfig) -> Self {
        Self {
            config,
            started_at: None,
            current_period: Vec::new(),
            amplitudes: Vec::new(),
            phases: Vec::new(),
            last_phase: None,
            valid_samples: 0,
            complete_frames: 0,
            incomplete_frames: 0,
            divergent_phase_values: 0,
            correction: None,
        }
    }

    pub fn reset(&mut self) {
        let config = self.config.clone();
        *self = Self::new(config);
    }

    pub fn correction(&self) -> Option<EncoderCorrection> {
        self.correction
    }

    pub fn process_frame(
        &mut self,
        frame: &Frame<PointHvdir>,
    ) -> Result<Option<EncoderCorrection>> {
        if let Some(correction) = self.correction {
            return Ok(Some(correction));
        }

        let started_at = *self.started_at.get_or_insert_with(Instant::now);
        if started_at.elapsed() > self.config.timeout {
            let average_amplitude = average(&self.amplitudes).unwrap_or(0.0);
            if average_amplitude < self.config.amplitude_threshold {
                let correction = EncoderCorrection::default();
                self.correction = Some(correction);
                return Ok(Some(correction));
            }
            return Err(QuanergyError::Calibration(format!(
                "phase values did not converge before timeout; valid samples: {} / {}, incomplete frames: {}, divergent phase values: {}",
                self.valid_samples,
                self.config.required_samples,
                self.incomplete_frames,
                self.divergent_phase_values
            )));
        }

        let width = if frame.width == 0 {
            frame.points.len()
        } else {
            frame.width.min(frame.points.len())
        };

        for point in frame.points.iter().take(width) {
            if !self.current_period.is_empty()
                && (self.current_period[self.current_period.len() - 1] - point.h).abs() > PI
            {
                self.finish_period()?;
                self.current_period.clear();
            }
            self.current_period.push(point.h);
        }

        Ok(self.correction)
    }

    fn finish_period(&mut self) -> Result<()> {
        if !self.check_complete() {
            self.incomplete_frames += 1;
            return Ok(());
        }

        self.complete_frames += 1;
        let params = calculate(
            &self.current_period,
            self.config.moving_average_period_counts,
        )?;
        if let Some(last_phase) = self.last_phase {
            if angle_diff(params.phase, last_phase) < self.config.phase_convergence_threshold {
                self.amplitudes.push(params.amplitude);
                self.phases.push(params.phase);
                self.valid_samples += 1;
                if self.valid_samples > self.config.required_samples {
                    let correction = EncoderCorrection::new(
                        average(&self.amplitudes).unwrap_or(0.0),
                        average_angle(&self.phases),
                    )?;
                    self.correction = Some(correction);
                }
            } else {
                self.amplitudes.clear();
                self.phases.clear();
                self.valid_samples = 0;
                self.amplitudes.push(params.amplitude);
                self.phases.push(params.phase);
                self.divergent_phase_values += 1;
            }
        } else {
            self.amplitudes.push(params.amplitude);
            self.phases.push(params.phase);
        }
        self.last_phase = Some(params.phase);
        Ok(())
    }

    fn check_complete(&self) -> bool {
        if self.current_period.len() < 2 {
            return false;
        }

        let first = self.current_period[0];
        let last = self.current_period[self.current_period.len() - 1];
        let min = first.min(last);
        let max = first.max(last);
        if max < PI - PI_TOLERANCE || min > -PI + PI_TOLERANCE {
            return false;
        }

        let expected = FIRING_RATE / self.config.frame_rate;
        let len = self.current_period.len() as f32;
        if len > expected + self.config.encoder_count_tolerance as f32
            || len < expected - self.config.encoder_count_tolerance as f32
        {
            return false;
        }

        let rads_per_count = TAU / expected;
        self.current_period
            .windows(2)
            .all(|pair| (pair[1] - pair[0]).abs() <= 5.0 * rads_per_count)
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct SineParameters {
    pub amplitude: f32,
    pub phase: f32,
}

pub fn calculate(
    encoder_angles: &[f32],
    moving_average_period_counts: usize,
) -> Result<SineParameters> {
    if encoder_angles.is_empty() {
        return Err(QuanergyError::Calibration(
            "cannot calculate sine parameters of empty angle set".to_owned(),
        ));
    }

    let slope = fit_line(encoder_angles);
    let mut sinusoid = calc_sinusoid(encoder_angles, slope, 0.0);
    let max = sinusoid.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let min = sinusoid.iter().copied().fold(f32::INFINITY, f32::min);
    let vertical_offset = (max + min) / 2.0;
    sinusoid = calc_sinusoid(encoder_angles, slope, vertical_offset);
    let smoothed = moving_average_filter(&sinusoid, moving_average_period_counts);
    find_sinusoid_parameters(&smoothed, slope < 0.0)
}

fn fit_line(encoder_angles: &[f32]) -> f32 {
    (encoder_angles[encoder_angles.len() - 1] - encoder_angles[0]) / encoder_angles.len() as f32
}

fn calc_sinusoid(encoder_angles: &[f32], slope: f32, y_intercept: f32) -> Vec<f32> {
    encoder_angles
        .iter()
        .enumerate()
        .map(|(count, angle)| *angle - slope * count as f32 - y_intercept)
        .collect()
}

fn moving_average_filter(values: &[f32], period: usize) -> Vec<f32> {
    let half_period = period / 2;
    values
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let begin = index.saturating_sub(half_period);
            let end = (index + half_period).min(values.len().saturating_sub(1));
            average(&values[begin..=end]).unwrap_or(0.0)
        })
        .collect()
}

fn find_sinusoid_parameters(signal: &[f32], clockwise: bool) -> Result<SineParameters> {
    if signal.is_empty() {
        return Err(QuanergyError::Calibration(
            "cannot find sinusoid parameters of empty signal".to_owned(),
        ));
    }

    let (max_index, amplitude) = signal
        .iter()
        .copied()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap();
    let (min_index, _) = signal
        .iter()
        .copied()
        .enumerate()
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap();

    if min_index == max_index {
        return Err(QuanergyError::Calibration(
            "peak detection found min and max peaks to be the same value".to_owned(),
        ));
    }

    let starting_angle = if clockwise { PI } else { -PI };
    let direction = if clockwise { -1.0 } else { 1.0 };
    let max_error_angle =
        starting_angle + direction * (TAU * max_index as f32 / signal.len() as f32);
    let min_error_angle =
        starting_angle + direction * (TAU * min_index as f32 / signal.len() as f32);
    let mut negative_phase = (min_error_angle + max_error_angle) / 2.0;

    if max_error_angle < min_error_angle {
        if negative_phase <= 0.0 {
            negative_phase += PI;
        } else {
            negative_phase -= PI;
        }
    }

    Ok(SineParameters {
        amplitude,
        phase: -negative_phase,
    })
}

fn angle_diff(a: f32, b: f32) -> f32 {
    let mut angle = (a - b).abs();
    if angle > PI {
        angle = TAU - angle;
    }
    angle
}

fn average(values: &[f32]) -> Option<f32> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f32>() / values.len() as f32)
    }
}

fn average_angle(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }

    let (sum_x, sum_y) = values.iter().fold((0.0, 0.0), |(x, y), angle| {
        (x + angle.cos(), y + angle.sin())
    });
    sum_y.atan2(sum_x)
}

pub fn apply_correction(frame: &mut Frame<PointHvdir>, correction: EncoderCorrection) {
    for point in &mut frame.points {
        *point = point.corrected(correction);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculate_rejects_empty_input() {
        assert!(matches!(
            calculate(&[], 8),
            Err(QuanergyError::Calibration(message)) if message.contains("empty")
        ));
    }

    #[test]
    fn calculate_finds_synthetic_phase_and_amplitude() {
        let amplitude = 0.08;
        let phase = 0.7;
        let count = 2_000;
        let angles: Vec<f32> = (0..count)
            .map(|i| {
                let base = -PI + TAU * i as f32 / count as f32;
                base + amplitude * (base + phase).sin()
            })
            .collect();

        let params = calculate(&angles, 15).unwrap();
        assert!((params.amplitude - amplitude).abs() < 0.02);
        assert!(angle_diff(params.phase, phase) < 0.2);
    }

    #[test]
    fn correction_keeps_zero_position_offset() {
        let correction = EncoderCorrection::new(0.1, 0.5).unwrap();
        let point = PointHvdir {
            h: 0.4,
            v: 0.0,
            d: 1.0,
            intensity: 0.0,
            ring: 0,
        };

        let corrected = point.corrected(correction);
        assert_ne!(corrected.h, point.h);
    }
}
