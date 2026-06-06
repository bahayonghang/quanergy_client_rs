use std::{
    io,
    path::{Path, PathBuf},
    sync::mpsc::{sync_channel, SyncSender, TrySendError},
};

use clap::{Args, Parser, Subcommand};
use quanergy_client::{
    cloud::{Frame, PointXyzir},
    config::{DeviceInfo, EncoderMode, PipelineConfig},
    error::QuanergyError,
    net::{fetch_device_info_xml, TcpPacketSource},
    pipeline::SensorPipeline,
    replay::{current_time_string, QrawReader, QrawWriter, SidecarMetadata},
    storage::{write_qpcd, NewCaptureSession, NewScanFrame, SqliteStore},
    transform::{CoordinateTransform, TransformSnapshot, YawPitchRollPose},
};
use serde_json::{json, Value};
use thiserror::Error;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

type Result<T> = std::result::Result<T, CaptureStoreError>;

const DEFAULT_OUTPUT_DIR: &str = "capture-store-output";
const DEFAULT_COORD_FRAME: &str = "station";
const DEFAULT_STORAGE_QUEUE_CAPACITY: usize = 8;

#[derive(Debug, Error)]
enum CaptureStoreError {
    #[error("{0}")]
    Quanergy(#[from] QuanergyError),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("storage worker is not available")]
    StorageDisconnected,
}

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Cli {
    #[arg(long, global = true)]
    verbose: bool,

    #[arg(long, global = true)]
    strict: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Live(LiveArgs),
    Replay(ReplayArgs),
}

#[derive(Debug, Args, Clone)]
struct CommonArgs {
    #[arg(long = "settings-file", short = 's')]
    settings_file: Option<PathBuf>,

    #[arg(long)]
    host: Option<String>,

    #[arg(long, short = 'f')]
    frame: Option<String>,

    #[arg(long = "return", short = 'r')]
    return_selection: Option<String>,

    #[arg(long)]
    calibrate: bool,

    #[arg(long = "frame-rate")]
    frame_rate: Option<f32>,

    #[arg(long = "manual-correct", num_args = 2, value_names = ["AMPLITUDE", "PHASE"])]
    manual_correct: Option<Vec<f32>>,

    #[arg(long = "min-distance")]
    min_distance: Option<f32>,

    #[arg(long = "max-distance")]
    max_distance: Option<f32>,

    #[arg(long = "min-cloud-size")]
    min_cloud_size: Option<usize>,

    #[arg(long = "max-cloud-size")]
    max_cloud_size: Option<usize>,
}

#[derive(Debug, Args, Clone)]
struct TransformArgs {
    #[arg(long = "x-m", default_value_t = 0.0)]
    x_m: f32,

    #[arg(long = "y-m", default_value_t = 0.0)]
    y_m: f32,

    #[arg(long = "z-m", default_value_t = 0.0)]
    z_m: f32,

    #[arg(long = "yaw-deg", default_value_t = 0.0)]
    yaw_deg: f32,

    #[arg(long = "pitch-deg", default_value_t = 0.0)]
    pitch_deg: f32,

    #[arg(long = "roll-deg", default_value_t = 0.0)]
    roll_deg: f32,
}

#[derive(Debug, Args, Clone)]
struct StoreArgs {
    #[arg(long = "output-dir", default_value = DEFAULT_OUTPUT_DIR)]
    output_dir: PathBuf,

    #[arg(long)]
    database: Option<PathBuf>,

    #[arg(long = "session-id")]
    session_id: Option<String>,

    #[arg(long)]
    notes: Option<String>,

    #[arg(long = "coord-frame", default_value = DEFAULT_COORD_FRAME)]
    coord_frame: String,
}

#[derive(Debug, Args)]
struct LiveArgs {
    #[command(flatten)]
    common: CommonArgs,

    #[command(flatten)]
    transform: TransformArgs,

    #[command(flatten)]
    store: StoreArgs,

    #[arg(long = "record-raw")]
    record_raw: bool,

    #[arg(long = "storage-queue-capacity", default_value_t = DEFAULT_STORAGE_QUEUE_CAPACITY)]
    storage_queue_capacity: usize,
}

#[derive(Debug, Args)]
struct ReplayArgs {
    input: PathBuf,

    #[command(flatten)]
    common: CommonArgs,

    #[command(flatten)]
    transform: TransformArgs,

    #[command(flatten)]
    store: StoreArgs,
}

struct StorageContext {
    output_dir: PathBuf,
    database: PathBuf,
    session_id: String,
    started_at: String,
    sensor_host: String,
    sensor_model: Option<String>,
    coord_frame: String,
    calibration_json: String,
    qraw_path: Option<PathBuf>,
    notes: Option<String>,
}

struct FrameToStore {
    frame: Frame<PointXyzir>,
    transform: TransformSnapshot,
    packet_type_mask: Option<u32>,
}

struct FramePersister {
    store: SqliteStore,
    context: StorageContext,
    frames_dir: PathBuf,
}

impl FramePersister {
    fn new(context: StorageContext) -> Result<Self> {
        std::fs::create_dir_all(&context.output_dir)?;
        let frames_dir = context.output_dir.join("frames").join(&context.session_id);
        std::fs::create_dir_all(&frames_dir)?;
        let store = SqliteStore::open(&context.database)?;
        store.insert_capture_session(&NewCaptureSession {
            session_id: context.session_id.clone(),
            started_at: context.started_at.clone(),
            sensor_host: context.sensor_host.clone(),
            sensor_model: context.sensor_model.clone(),
            sdk_version: env!("CARGO_PKG_VERSION").to_owned(),
            status: "running".to_owned(),
            notes: context.notes.clone(),
        })?;

        Ok(Self {
            store,
            context,
            frames_dir,
        })
    }

    fn persist_frame(&mut self, input: FrameToStore) -> Result<()> {
        let qpcd_path = self
            .frames_dir
            .join(format!("frame_{:012}.qpcd", input.frame.sequence));
        let qpcd_header = write_qpcd(&qpcd_path, &input.frame, &self.context.coord_frame)?;
        let transform_json = serde_json::to_string(&input.transform)?;
        self.store.insert_scan_frame(&NewScanFrame {
            session_id: self.context.session_id.clone(),
            sequence: input.frame.sequence,
            timestamp_micros: input.frame.stamp_micros,
            sensor_host: self.context.sensor_host.clone(),
            sensor_model: self.context.sensor_model.clone(),
            packet_type_mask: input.packet_type_mask,
            point_count: qpcd_header.point_count,
            coord_frame: self.context.coord_frame.clone(),
            transform_4x4: input.transform.matrix_4x4,
            transform_json,
            calibration_json: self.context.calibration_json.clone(),
            cloud_path: qpcd_path.to_string_lossy().into_owned(),
            qraw_path: self
                .context
                .qraw_path
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            status: "complete".to_owned(),
            created_at: current_time_string(),
        })?;
        Ok(())
    }

    fn finish(&self, status: &str) -> Result<()> {
        self.store.finish_capture_session(
            &self.context.session_id,
            &current_time_string(),
            status,
        )?;
        Ok(())
    }
}

fn main() {
    let cli = Cli::parse();
    init_logging(cli.verbose);

    if let Err(error) = run(cli) {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Live(args) => run_live(args, cli.strict),
        Command::Replay(args) => run_replay(args, cli.strict),
    }
}

fn init_logging(verbose: bool) {
    let default = if verbose { "debug" } else { "info" };
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default)),
        )
        .try_init();
}

fn run_live(args: LiveArgs, strict: bool) -> Result<()> {
    let mut config = build_config(&args.common, strict)?;
    let device_info_error = enrich_from_device_info(&mut config)
        .err()
        .map(|error| error.to_string());
    let host = require_host(&config)?;
    let transform = build_transform(&args.transform);
    let calibration_json = calibration_snapshot_json(&config, device_info_error.as_deref())?;
    let session_id = session_id_or_default(args.store.session_id.as_deref());
    let qraw_path = if args.record_raw {
        Some(
            args.store
                .output_dir
                .join("raw")
                .join(format!("{session_id}.qraw")),
        )
    } else {
        None
    };
    if let Some(path) = &qraw_path {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        write_sidecar(path, Some(host.clone()), &config, device_info_error);
    }

    let context = StorageContext {
        output_dir: args.store.output_dir.clone(),
        database: database_path(&args.store),
        session_id,
        started_at: current_time_string(),
        sensor_host: host.clone(),
        sensor_model: sensor_model(&config),
        coord_frame: args.store.coord_frame.clone(),
        calibration_json,
        qraw_path: qraw_path.clone(),
        notes: args.store.notes.clone(),
    };
    let storage_tx = spawn_storage_worker(
        FramePersister::new(context)?,
        args.storage_queue_capacity.max(1),
    );
    let mut dropped_frames = 0u64;
    let mut source = TcpPacketSource::connect(&host)?;
    let mut pipeline = SensorPipeline::new(config)?;
    let mut raw_writer = if let Some(path) = &qraw_path {
        Some(QrawWriter::create(path)?)
    } else {
        None
    };

    info!(%host, record_raw = args.record_raw, "starting live station-frame capture");
    loop {
        let packet = source.next_packet()?;
        if let Some(writer) = &mut raw_writer {
            writer.write_packet(packet.arrival_delta_ns, &packet.bytes)?;
        }
        let packet_mask = packet_type_mask(packet.header.packet_type);
        for frame in pipeline.process_raw(&packet)? {
            let station_frame = transform.transform_frame(&frame);
            let item = FrameToStore {
                frame: station_frame,
                transform: transform.snapshot(),
                packet_type_mask: packet_mask,
            };
            match storage_tx.try_send(item) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    dropped_frames += 1;
                    warn!(
                        dropped_frames,
                        "dropping frame because storage queue is full"
                    );
                }
                Err(TrySendError::Disconnected(_)) => {
                    return Err(CaptureStoreError::StorageDisconnected)
                }
            }
        }
    }
}

fn run_replay(args: ReplayArgs, strict: bool) -> Result<()> {
    let mut config = build_config(&args.common, strict)?;
    let sidecar = SidecarMetadata::load(SidecarMetadata::sidecar_path(&args.input)).ok();
    apply_sidecar_to_config(&mut config, sidecar.as_ref());
    let sensor_host = if config.host.is_empty() {
        "replay".to_owned()
    } else {
        config.host.clone()
    };
    let transform = build_transform(&args.transform);
    let calibration_json = replay_calibration_snapshot_json(&config, sidecar.as_ref())?;
    let session_id = session_id_or_default(args.store.session_id.as_deref());
    let context = StorageContext {
        output_dir: args.store.output_dir.clone(),
        database: database_path(&args.store),
        session_id,
        started_at: current_time_string(),
        sensor_host,
        sensor_model: sensor_model(&config),
        coord_frame: args.store.coord_frame.clone(),
        calibration_json,
        qraw_path: Some(args.input.clone()),
        notes: args.store.notes.clone(),
    };
    let mut persister = FramePersister::new(context)?;
    let mut reader = QrawReader::open(&args.input)?;
    let mut pipeline = SensorPipeline::new(config)?;
    let mut frames_written = 0u64;

    while let Some(packet) = reader.next_packet()? {
        let packet_mask = packet_type_mask(packet.header.packet_type);
        for frame in pipeline.process_raw(&packet)? {
            let item = FrameToStore {
                frame: transform.transform_frame(&frame),
                transform: transform.snapshot(),
                packet_type_mask: packet_mask,
            };
            persister.persist_frame(item)?;
            frames_written += 1;
        }
    }
    persister.finish("complete")?;
    info!(frames_written, "finished replay station-frame storage");
    Ok(())
}

fn spawn_storage_worker(
    mut persister: FramePersister,
    capacity: usize,
) -> SyncSender<FrameToStore> {
    let (tx, rx) = sync_channel(capacity);
    std::thread::spawn(move || {
        let mut frames_written = 0u64;
        while let Ok(item) = rx.recv() {
            if let Err(error) = persister.persist_frame(item) {
                error!(%error, frames_written, "failed to persist station-frame cloud");
                return;
            }
            frames_written += 1;
        }
        if let Err(error) = persister.finish("complete") {
            warn!(%error, "failed to finish capture session");
        }
    });
    tx
}

fn build_transform(args: &TransformArgs) -> Box<dyn CoordinateTransform> {
    Box::new(
        YawPitchRollPose {
            x_m: args.x_m,
            y_m: args.y_m,
            z_m: args.z_m,
            yaw_deg: args.yaw_deg,
            pitch_deg: args.pitch_deg,
            roll_deg: args.roll_deg,
        }
        .to_transform(),
    )
}

fn build_config(args: &CommonArgs, strict: bool) -> Result<PipelineConfig> {
    let mut config = PipelineConfig {
        strict,
        ..PipelineConfig::default()
    };
    if let Some(path) = &args.settings_file {
        config.load_settings_file(path)?;
    }
    if let Some(host) = &args.host {
        config.host = host.clone();
    }
    if let Some(frame) = &args.frame {
        config.frame_id = frame.clone();
    }
    if let Some(value) = &args.return_selection {
        config.return_selection = quanergy_client::protocol::ReturnSelection::parse(value)?;
        config.return_selection_set = true;
    }
    if let Some(frame_rate) = args.frame_rate {
        config.frame_rate = frame_rate;
    }
    if let Some(values) = &args.manual_correct {
        if values.len() != 2 {
            return Err(QuanergyError::Config(
                "manual encoder correction expects exactly 2 parameters".to_owned(),
            )
            .into());
        }
        config.encoder_mode = EncoderMode::Manual {
            amplitude: values[0],
            phase: values[1],
        };
    }
    if args.calibrate {
        config.encoder_mode = EncoderMode::Automatic;
    }
    if let Some(value) = args.min_distance {
        config.min_distance = value;
    }
    if let Some(value) = args.max_distance {
        config.max_distance = value;
    }
    if let Some(value) = args.min_cloud_size {
        config.min_cloud_size = value;
    }
    if let Some(value) = args.max_cloud_size {
        config.max_cloud_size = value;
    }
    Ok(config)
}

fn enrich_from_device_info(config: &mut PipelineConfig) -> Result<()> {
    if config.host.is_empty() {
        return Ok(());
    }
    match fetch_device_info_xml(&config.host).and_then(|xml| DeviceInfo::parse_xml(&xml)) {
        Ok(info) => {
            config.apply_device_info(&info);
            Ok(())
        }
        Err(error) => {
            warn!(%error, "deviceInfo unavailable; continuing with configured/default calibration");
            Err(error.into())
        }
    }
}

fn require_host(config: &PipelineConfig) -> Result<String> {
    if config.host.is_empty() {
        Err(QuanergyError::Config(
            [
                "no host provided",
                "",
                "Provide a sensor host, for example:",
                "  capture-store live --host <SENSOR_IP>",
                "  capture-store live --settings-file <client.xml>",
            ]
            .join("\n"),
        )
        .into())
    } else {
        Ok(config.host.clone())
    }
}

fn apply_sidecar_to_config(config: &mut PipelineConfig, sidecar: Option<&SidecarMetadata>) {
    if let Some(sidecar) = sidecar {
        if config.host.is_empty() {
            if let Some(host) = &sidecar.sensor_host {
                config.host = host.clone();
            }
        }
        if let Some(vertical_angles) = &sidecar.vertical_angles {
            config.vertical_angles = Some(vertical_angles.clone());
        }
        if let (Some(amplitude), Some(phase)) = (sidecar.encoder_amplitude, sidecar.encoder_phase) {
            config.encoder_mode = EncoderMode::Manual { amplitude, phase };
        }
    }
}

fn write_sidecar(
    path: &Path,
    host: Option<String>,
    config: &PipelineConfig,
    error: Option<String>,
) {
    let metadata = if let Some(error) = error {
        SidecarMetadata::incomplete(host, error)
    } else {
        SidecarMetadata::from_config(host, config)
    };
    if let Err(error) = metadata.save(SidecarMetadata::sidecar_path(path)) {
        warn!(%error, "failed to write qraw sidecar");
    }
}

fn calibration_snapshot_json(config: &PipelineConfig, error: Option<&str>) -> Result<String> {
    Ok(serde_json::to_string(&json!({
        "source": "live",
        "calibration_complete": error.is_none(),
        "calibration_error": error,
        "model": sensor_model(config),
        "vertical_angles": config.vertical_angles,
        "encoder_mode": encoder_mode_json(&config.encoder_mode),
    }))?)
}

fn replay_calibration_snapshot_json(
    config: &PipelineConfig,
    sidecar: Option<&SidecarMetadata>,
) -> Result<String> {
    Ok(serde_json::to_string(&json!({
        "source": "qraw_sidecar",
        "sidecar": sidecar,
        "model": sensor_model(config),
        "vertical_angles": config.vertical_angles,
        "encoder_mode": encoder_mode_json(&config.encoder_mode),
    }))?)
}

fn encoder_mode_json(mode: &EncoderMode) -> Value {
    match mode {
        EncoderMode::Disabled => json!({"kind": "disabled"}),
        EncoderMode::Manual { amplitude, phase } => {
            json!({"kind": "manual", "amplitude": amplitude, "phase": phase})
        }
        EncoderMode::DeviceInfo => json!({"kind": "device_info"}),
        EncoderMode::Automatic => json!({"kind": "automatic"}),
    }
}

fn sensor_model(config: &PipelineConfig) -> Option<String> {
    config.model.as_ref().map(|model| format!("{model:?}"))
}

fn database_path(args: &StoreArgs) -> PathBuf {
    args.database
        .clone()
        .unwrap_or_else(|| args.output_dir.join("capture.sqlite"))
}

fn session_id_or_default(session_id: Option<&str>) -> String {
    session_id
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(default_session_id)
}

fn default_session_id() -> String {
    let sanitized: String = current_time_string()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect();
    format!("session-{sanitized}-{}", std::process::id())
}

fn packet_type_mask(packet_type: u8) -> Option<u32> {
    if packet_type < 32 {
        Some(1u32 << packet_type)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_transform_field_names() {
        let cli = Cli::try_parse_from([
            "capture-store",
            "live",
            "--host",
            "192.0.2.10",
            "--x-m",
            "1.0",
            "--y-m",
            "2.0",
            "--z-m",
            "3.0",
            "--yaw-deg",
            "4.0",
            "--pitch-deg",
            "5.0",
            "--roll-deg",
            "6.0",
        ])
        .unwrap();

        match cli.command {
            Command::Live(args) => {
                assert_eq!(args.transform.x_m, 1.0);
                assert_eq!(args.transform.y_m, 2.0);
                assert_eq!(args.transform.z_m, 3.0);
                assert_eq!(args.transform.yaw_deg, 4.0);
                assert_eq!(args.transform.pitch_deg, 5.0);
                assert_eq!(args.transform.roll_deg, 6.0);
                assert!(!args.record_raw);
            }
            Command::Replay(_) => panic!("expected live command"),
        }
    }

    #[test]
    fn record_raw_is_opt_in() {
        let cli = Cli::try_parse_from([
            "capture-store",
            "live",
            "--host",
            "192.0.2.10",
            "--record-raw",
        ])
        .unwrap();

        match cli.command {
            Command::Live(args) => assert!(args.record_raw),
            Command::Replay(_) => panic!("expected live command"),
        }
    }

    #[test]
    fn replay_uses_input_qraw_and_storage_args() {
        let cli = Cli::try_parse_from([
            "capture-store",
            "replay",
            "input.qraw",
            "--output-dir",
            "out",
            "--database",
            "out/meta.sqlite",
            "--session-id",
            "session-a",
            "--notes",
            "debug",
        ])
        .unwrap();

        match cli.command {
            Command::Replay(args) => {
                assert_eq!(args.input, PathBuf::from("input.qraw"));
                assert_eq!(args.store.output_dir, PathBuf::from("out"));
                assert_eq!(args.store.database, Some(PathBuf::from("out/meta.sqlite")));
                assert_eq!(args.store.session_id.as_deref(), Some("session-a"));
                assert_eq!(args.store.notes.as_deref(), Some("debug"));
            }
            Command::Live(_) => panic!("expected replay command"),
        }
    }
}
