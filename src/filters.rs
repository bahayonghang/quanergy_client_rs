use crate::cloud::{Frame, PointHvdir};

#[derive(Debug, Clone)]
pub struct DistanceFilter {
    pub min: f32,
    pub max: f32,
}

impl Default for DistanceFilter {
    fn default() -> Self {
        Self {
            min: 0.0,
            max: 500.0,
        }
    }
}

impl DistanceFilter {
    pub fn apply(&self, frame: &mut Frame<PointHvdir>) {
        for point in &mut frame.points {
            if point.d < self.min || point.d > self.max {
                point.d = f32::NAN;
                frame.is_dense = false;
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct RingIntensityFilter {
    pub min_range: [f32; crate::protocol::M_SERIES_NUM_LASERS],
    pub min_intensity: [u8; crate::protocol::M_SERIES_NUM_LASERS],
}

impl Default for RingIntensityFilter {
    fn default() -> Self {
        Self {
            min_range: [1.0; crate::protocol::M_SERIES_NUM_LASERS],
            min_intensity: [0; crate::protocol::M_SERIES_NUM_LASERS],
        }
    }
}

impl RingIntensityFilter {
    pub fn apply(&self, frame: &mut Frame<PointHvdir>) {
        for point in &mut frame.points {
            let ring = point.ring as usize;
            if ring < crate::protocol::M_SERIES_NUM_LASERS
                && point.d < self.min_range[ring]
                && point.intensity < self.min_intensity[ring] as f32
            {
                point.d = f32::NAN;
                frame.is_dense = false;
            }
        }
    }
}
