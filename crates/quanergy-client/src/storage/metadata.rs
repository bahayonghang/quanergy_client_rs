#[derive(Debug, Clone, PartialEq)]
pub struct NewCaptureSession {
    pub session_id: String,
    pub started_at: String,
    pub sensor_host: String,
    pub sensor_model: Option<String>,
    pub sdk_version: String,
    pub status: String,
    pub notes: Option<String>,
    // --- v2 fields (station provenance) ---
    pub station_id: Option<String>,
    pub source_frame: Option<String>,
    pub target_frame: Option<String>,
    pub transform_id: Option<String>,
    pub station_config_json: Option<String>,
    pub station_config_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CaptureSession {
    pub session_id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub sensor_host: String,
    pub sensor_model: Option<String>,
    pub sdk_version: String,
    pub status: String,
    pub notes: Option<String>,
    // --- v2 fields ---
    pub station_id: Option<String>,
    pub source_frame: Option<String>,
    pub target_frame: Option<String>,
    pub transform_id: Option<String>,
    pub station_config_json: Option<String>,
    pub station_config_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewScanFrame {
    pub session_id: String,
    pub sequence: u64,
    pub timestamp_micros: u64,
    pub sensor_host: String,
    pub sensor_model: Option<String>,
    pub packet_type_mask: Option<u32>,
    pub point_count: u64,
    pub coord_frame: String,
    pub transform_4x4: [[f32; 4]; 4],
    pub transform_json: String,
    pub calibration_json: String,
    pub cloud_path: String,
    pub qraw_path: Option<String>,
    pub status: String,
    pub created_at: String,
    // --- v2 fields ---
    pub source_frame: Option<String>,
    pub target_frame: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScanFrameRecord {
    pub frame_id: i64,
    pub session_id: String,
    pub sequence: u64,
    pub timestamp_micros: u64,
    pub sensor_host: String,
    pub sensor_model: Option<String>,
    pub packet_type_mask: Option<u32>,
    pub point_count: u64,
    pub coord_frame: String,
    pub transform_4x4: [[f32; 4]; 4],
    pub transform_json: String,
    pub calibration_json: String,
    pub cloud_path: String,
    pub qraw_path: Option<String>,
    pub status: String,
    pub created_at: String,
    // --- v2 fields ---
    pub source_frame: Option<String>,
    pub target_frame: Option<String>,
}

/// Row returned from hammer_measurement queries (schema v3).
#[derive(Debug, Clone, PartialEq)]
pub struct HammerMeasurementRow {
    pub measurement_id: i64,
    pub session_id: String,
    pub sequence: u64,
    pub hammer_id: String,
    pub roi_point_count: usize,
    pub valid_point_count: usize,
    pub top_z_m: Option<f32>,
    pub reference_z_m: Option<f32>,
    pub height_m: Option<f32>,
    pub z_spread_m: Option<f32>,
    pub quality: f32,
    pub estimator: String,
    pub status: String,
    pub created_at: String,
}
