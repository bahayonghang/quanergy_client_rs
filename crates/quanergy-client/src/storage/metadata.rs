#[derive(Debug, Clone, PartialEq)]
pub struct NewCaptureSession {
    pub session_id: String,
    pub started_at: String,
    pub sensor_host: String,
    pub sensor_model: Option<String>,
    pub sdk_version: String,
    pub status: String,
    pub notes: Option<String>,
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
}
