//! Static station geometry derived from a validated station config.
//!
//! This is the runtime representation used by the visualizer and
//! measurement tools. It is constructed from [`ValidatedStationConfig`].

use super::hammer::HammerGeometry;

/// Fixed geometric description of a tamping station.
///
/// Bundles the sensor-to-station transform, frame identities,
/// scanner origin, and hammer layout into one struct for
/// visualization and measurement consumers.
#[derive(Debug, Clone)]
pub struct StationGeometry {
    /// Station identifier from config.
    pub station_id: String,
    /// Source coordinate frame (e.g. "quanergy_sensor").
    pub source_frame: String,
    /// Target coordinate frame (e.g. "station").
    pub target_frame: String,
    /// Scanner laser origin expressed in station coordinates (meters).
    pub scanner_origin_station: [f32; 3],
    /// 4×4 rigid transform from sensor to station.
    pub sensor_to_station: [[f32; 4]; 4],
    /// All hammers (enabled + disabled).
    pub hammers: Vec<HammerGeometry>,
}

impl StationGeometry {
    /// Construct from a validated config.
    pub fn from_config(config: &super::config::ValidatedStationConfig) -> Self {
        let m = config.transform();
        let scanner_origin_station = [m[0][3], m[1][3], m[2][3]];

        Self {
            station_id: config.station_id.clone(),
            source_frame: config.source_frame.clone(),
            target_frame: config.target_frame.clone(),
            scanner_origin_station,
            sensor_to_station: m,
            hammers: config.hammer_layout.hammers.clone(),
        }
    }
}
