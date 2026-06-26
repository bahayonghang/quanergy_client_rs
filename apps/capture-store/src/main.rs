use std::{
    io,
    path::{Path, PathBuf},
    sync::mpsc::{sync_channel, SyncSender, TrySendError},
};

use clap::{Args, Parser, Subcommand, ValueEnum};
use quanergy_client::{
    cloud::{Frame, PointXyzir},
    config::{DeviceInfo, EncoderMode, PipelineConfig},
    error::QuanergyError,
    net::{fetch_device_info_xml, TcpPacketSource},
    pipeline::SensorPipeline,
    replay::{current_time_string, QrawReader, QrawWriter, SidecarMetadata},
    station::{load_station_config, StationGeometry, ValidatedStationConfig},
    storage::{write_qpcd_with_metadata, NewCaptureSession, NewScanFrame, SqliteStore},
    transform::{CoordinateTransform, StationTransform, TransformSnapshot, YawPitchRollPose},
};
use serde_json::{json, Value};
use thiserror::Error;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

type Result<T> = std::result::Result<T, CaptureStoreError>;

const DEFAULT_OUTPUT_DIR: &str = "capture-store-output";
const DEFAULT_COORD_FRAME: &str = "station";
const DEFAULT_STORAGE_QUEUE_CAPACITY: usize = 8;
const DEFAULT_RAW_QUEUE_CAPACITY: usize = 32;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OverflowPolicy {
    /// Fail the session immediately when a queue is full (formal measurement default).
    Fail,
    /// Drop the newest item and continue (monitoring mode).
    DropNewest,
    /// Block until space is available (offline replay only; warns in live mode).
    Block,
}

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

    #[error("raw recorder is not available")]
    RawRecorderDisconnected,

    #[error("deviceInfo required but unavailable: {0}")]
    DeviceInfoRequired(String),

    #[error("session aborted: {reason}")]
    SessionFailed { reason: String },
}

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Cli {
    #[arg(long, global = true)]
    verbose: bool,

    #[arg(long, global = true)]
    strict: bool,

    /// Path to a station.toml defining the fixed sensor-to-station transform.
    /// When set, --x-m/--y-m/--z-m/--yaw-deg/--pitch-deg/--roll-deg are ignored.
    #[arg(long = "station-config", global = true)]
    station_config: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Live(LiveArgs),
    Replay(ReplayArgs),
    /// Validate a station.toml file and print its effective transform and hammer layout.
    ValidateStationConfig(ValidateStationConfigArgs),
}

#[derive(Debug, Args)]
struct ValidateStationConfigArgs {
    /// Path to the station TOML file.
    file: PathBuf,
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

    /// Require successful deviceInfo fetch; refuse to start formal capture without it.
    #[arg(long = "require-device-info")]
    require_device_info: bool,

    /// Queue overflow behavior for storage and raw queues.
    #[arg(long = "overflow-policy", default_value = "fail")]
    overflow_policy: OverflowPolicy,

    #[arg(long = "storage-queue-capacity", default_value_t = DEFAULT_STORAGE_QUEUE_CAPACITY)]
    storage_queue_capacity: usize,

    #[arg(long = "raw-queue-capacity", default_value_t = DEFAULT_RAW_QUEUE_CAPACITY)]
    raw_queue_capacity: usize,
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
    // v2 station provenance
    station_id: Option<String>,
    source_frame: Option<String>,
    target_frame: Option<String>,
    transform_id: Option<String>,
    station_config_json: Option<String>,
    station_config_sha256: Option<String>,
}

struct FrameToStore {
    frame: Frame<PointXyzir>,
    transform: TransformSnapshot,
    packet_type_mask: Option<u32>,
}

#[allow(dead_code)]
enum StorageCommand {
    Persist(Box<FrameToStore>),
    Finish,
}

#[allow(dead_code)]
struct StorageStats {
    frames_written: u64,
}

// ---------------------------------------------------------------------------
// Raw recorder worker
// ---------------------------------------------------------------------------

struct RawPacketToRecord {
    delta_ns: u64,
    bytes: Box<[u8]>,
}

#[allow(dead_code)]
enum RawCommand {
    Write(RawPacketToRecord),
    Finish,
}

fn spawn_raw_worker(
    path: &Path,
    capacity: usize,
) -> Result<(SyncSender<RawCommand>, std::thread::JoinHandle<Result<()>>)> {
    let mut writer = QrawWriter::create(path)?;
    let (tx, rx) = sync_channel::<RawCommand>(capacity);
    let handle = std::thread::spawn(move || {
        for cmd in rx {
            match cmd {
                RawCommand::Write(packet) => {
                    writer.write_packet(packet.delta_ns, &packet.bytes)?;
                }
                RawCommand::Finish => {
                    writer.flush()?;
                    return Ok(());
                }
            }
        }
        // Channel closed without Finish
        writer.flush().ok();
        Ok(())
    });
    Ok((tx, handle))
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
            station_id: context.station_id.clone(),
            source_frame: context.source_frame.clone(),
            target_frame: context.target_frame.clone(),
            transform_id: context.transform_id.clone(),
            station_config_json: context.station_config_json.clone(),
            station_config_sha256: context.station_config_sha256.clone(),
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
        let qpcd_header = write_qpcd_with_metadata(
            &qpcd_path,
            &input.frame,
            &self.context.coord_frame,
            self.context.source_frame.clone(),
            self.context.target_frame.clone(),
            self.context.station_id.clone(),
            self.context.transform_id.clone(),
            self.context.station_config_sha256.clone(),
        )?;
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
            source_frame: self.context.source_frame.clone(),
            target_frame: self.context.target_frame.clone(),
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
        Command::Live(args) => run_live(args, cli.strict, cli.station_config.as_deref()),
        Command::Replay(args) => run_replay(args, cli.strict, cli.station_config.as_deref()),
        Command::ValidateStationConfig(args) => validate_station_config(&args.file),
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

fn run_live(args: LiveArgs, strict: bool, station_config_path: Option<&Path>) -> Result<()> {
    let mut config = build_config(&args.common, strict)?;
    let device_info_error = enrich_from_device_info(&mut config)
        .err()
        .map(|error| error.to_string());

    // --- require-device-info gate ---
    if args.require_device_info {
        if let Some(ref error) = device_info_error {
            return Err(CaptureStoreError::DeviceInfoRequired(error.clone()));
        }
    }

    let host = require_host(&config)?;

    let station_config = load_station_config_opt(station_config_path)?;
    check_transform_mutex(&args.transform, station_config.is_some())?;
    let strategy = build_transform_strategy(&args.transform, &station_config)?;

    let calibration_json = calibration_snapshot_json(&config, device_info_error.as_deref())?;
    let session_id = session_id_or_default(args.store.session_id.as_deref());

    // --- raw recording setup ---
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

    let (raw_tx, raw_handle) = if let Some(path) = &qraw_path {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        write_sidecar(path, Some(host.clone()), &config, device_info_error.clone());
        let (tx, handle) = spawn_raw_worker(path, args.raw_queue_capacity.max(1))?;
        (Some(tx), Some(handle))
    } else {
        (None, None)
    };

    let coord_frame = if let Some(ref sc) = station_config {
        sc.target_frame.clone()
    } else {
        args.store.coord_frame.clone()
    };

    copy_station_config_snapshot(&args.store.output_dir, &session_id, station_config_path);

    let station_json = station_config
        .as_ref()
        .map(|sc| sc.canonical_toml().to_owned());
    let station_sha256 = station_config
        .as_ref()
        .map(|sc| sc.config_hash().to_owned());

    let context = StorageContext {
        output_dir: args.store.output_dir.clone(),
        database: database_path(&args.store),
        session_id: session_id.clone(),
        started_at: current_time_string(),
        sensor_host: host.clone(),
        sensor_model: sensor_model(&config),
        coord_frame: coord_frame.clone(),
        calibration_json,
        qraw_path: qraw_path.clone(),
        notes: args.store.notes.clone(),
        station_id: station_config.as_ref().map(|sc| sc.station_id.clone()),
        source_frame: station_config.as_ref().map(|sc| sc.source_frame.clone()),
        target_frame: station_config.as_ref().map(|sc| sc.target_frame.clone()),
        transform_id: station_config.as_ref().map(|sc| sc.extrinsic_id.clone()),
        station_config_json: station_json,
        station_config_sha256: station_sha256,
    };
    let (storage_tx, storage_handle) = spawn_storage_worker(
        FramePersister::new(context)?,
        args.storage_queue_capacity.max(1),
    );

    let mut dropped_frames = 0u64;
    let mut source = TcpPacketSource::connect(&host)?;
    let mut pipeline = SensorPipeline::new(config)?;

    info!(%host, record_raw = args.record_raw, "starting live station-frame capture");

    // --- main ingestion loop ---
    let loop_result = run_ingestion_loop(
        &mut source,
        &mut pipeline,
        &strategy,
        &storage_tx,
        raw_tx.as_ref(),
        &mut dropped_frames,
        args.overflow_policy,
    );

    // --- graceful shutdown ---
    // 1. Stop storage: send Finish
    let _ = storage_tx.send(StorageCommand::Finish);
    // 2. Drop sender so storage worker sees EOF
    drop(storage_tx);
    // 3. Stop raw recorder: send Finish and drop sender so worker exits
    if let Some(ref raw) = raw_tx {
        let _ = raw.send(RawCommand::Finish);
    }
    drop(raw_tx);
    // 4. Join workers
    let storage_result = storage_handle
        .join()
        .map_err(|_| CaptureStoreError::StorageDisconnected)?;
    let raw_result = if let Some(handle) = raw_handle {
        match handle.join() {
            Ok(r) => Some(r),
            Err(_) => Some(Err(QuanergyError::Io(io::Error::other(
                "raw recorder thread panicked",
            ))
            .into())),
        }
    } else {
        None
    };

    // Determine final session status
    let session_failed = loop_result.is_err()
        || storage_result.is_err()
        || raw_result.as_ref().is_some_and(|r| r.is_err());

    if let Err(error) = loop_result {
        error!(%error, "ingestion loop error");
    }
    if let Err(ref error) = storage_result {
        error!(%error, "storage worker error");
    }
    if let Some(Err(ref error)) = raw_result {
        error!(%error, "raw recorder error");
    }

    if session_failed {
        Err(CaptureStoreError::SessionFailed {
            reason: "see previous errors".to_owned(),
        })
    } else {
        info!(
            dropped_frames,
            storage_stats = ?storage_result.as_ref().ok().map(|s| s.frames_written),
            "live capture finished successfully"
        );
        Ok(())
    }
}

fn run_ingestion_loop(
    source: &mut TcpPacketSource,
    pipeline: &mut SensorPipeline,
    strategy: &TransformStrategy,
    storage_tx: &SyncSender<StorageCommand>,
    raw_tx: Option<&SyncSender<RawCommand>>,
    dropped_frames: &mut u64,
    overflow_policy: OverflowPolicy,
) -> Result<()> {
    loop {
        let packet = source.next_packet()?;

        // Send raw packet to recorder
        if let Some(tx) = raw_tx {
            let raw_item = RawPacketToRecord {
                delta_ns: packet.arrival_delta_ns,
                bytes: packet.bytes.clone().into_boxed_slice(),
            };
            match overflow_policy {
                OverflowPolicy::Fail => {
                    tx.send(RawCommand::Write(raw_item))
                        .map_err(|_| CaptureStoreError::RawRecorderDisconnected)?;
                }
                OverflowPolicy::DropNewest => {
                    if let Err(_e) = tx.try_send(RawCommand::Write(raw_item)) {
                        *dropped_frames += 1;
                        warn!(dropped_frames, "raw queue full, dropping raw packet");
                    }
                }
                OverflowPolicy::Block => {
                    warn!("blocking on raw queue in live mode is not recommended");
                    tx.send(RawCommand::Write(raw_item))
                        .map_err(|_| CaptureStoreError::RawRecorderDisconnected)?;
                }
            }
        }

        let packet_mask = packet_type_mask(packet.header.packet_type);
        for frame in pipeline.process_raw(&packet)? {
            let (station_frame, snapshot) = strategy.transform(&frame);
            let item = FrameToStore {
                frame: station_frame,
                transform: snapshot,
                packet_type_mask: packet_mask,
            };
            match overflow_policy {
                OverflowPolicy::Fail => {
                    storage_tx
                        .send(StorageCommand::Persist(Box::new(item)))
                        .map_err(|_| CaptureStoreError::StorageDisconnected)?;
                }
                OverflowPolicy::DropNewest => {
                    match storage_tx.try_send(StorageCommand::Persist(Box::new(item))) {
                        Ok(()) => {}
                        Err(TrySendError::Full(_)) => {
                            *dropped_frames += 1;
                            warn!(dropped_frames, "storage queue full, dropping frame");
                        }
                        Err(TrySendError::Disconnected(_)) => {
                            return Err(CaptureStoreError::StorageDisconnected)
                        }
                    }
                }
                OverflowPolicy::Block => {
                    storage_tx
                        .send(StorageCommand::Persist(Box::new(item)))
                        .map_err(|_| CaptureStoreError::StorageDisconnected)?;
                }
            }
        }
    }
}

fn run_replay(args: ReplayArgs, strict: bool, station_config_path: Option<&Path>) -> Result<()> {
    let mut config = build_config(&args.common, strict)?;
    let sidecar = SidecarMetadata::load(SidecarMetadata::sidecar_path(&args.input)).ok();
    apply_sidecar_to_config(&mut config, sidecar.as_ref());
    let sensor_host = if config.host.is_empty() {
        "replay".to_owned()
    } else {
        config.host.clone()
    };

    let station_config = load_station_config_opt(station_config_path)?;
    check_transform_mutex(&args.transform, station_config.is_some())?;
    let strategy = build_transform_strategy(&args.transform, &station_config)?;

    let calibration_json = replay_calibration_snapshot_json(&config, sidecar.as_ref())?;
    let session_id = session_id_or_default(args.store.session_id.as_deref());

    let coord_frame = if let Some(ref sc) = station_config {
        sc.target_frame.clone()
    } else {
        args.store.coord_frame.clone()
    };

    copy_station_config_snapshot(&args.store.output_dir, &session_id, station_config_path);

    let station_json = station_config
        .as_ref()
        .map(|sc| sc.canonical_toml().to_owned());
    let station_sha256 = station_config
        .as_ref()
        .map(|sc| sc.config_hash().to_owned());

    let context = StorageContext {
        output_dir: args.store.output_dir.clone(),
        database: database_path(&args.store),
        session_id: session_id.clone(),
        started_at: current_time_string(),
        sensor_host,
        sensor_model: sensor_model(&config),
        coord_frame: coord_frame.clone(),
        calibration_json,
        qraw_path: Some(args.input.clone()),
        notes: args.store.notes.clone(),
        station_id: station_config.as_ref().map(|sc| sc.station_id.clone()),
        source_frame: station_config.as_ref().map(|sc| sc.source_frame.clone()),
        target_frame: station_config.as_ref().map(|sc| sc.target_frame.clone()),
        transform_id: station_config.as_ref().map(|sc| sc.extrinsic_id.clone()),
        station_config_json: station_json,
        station_config_sha256: station_sha256,
    };
    let mut persister = FramePersister::new(context)?;
    let mut reader = QrawReader::open(&args.input)?;
    let mut pipeline = SensorPipeline::new(config)?;
    let mut frames_written = 0u64;

    while let Some(packet) = reader.next_packet()? {
        let packet_mask = packet_type_mask(packet.header.packet_type);
        for frame in pipeline.process_raw(&packet)? {
            let (station_frame, snapshot) = strategy.transform(&frame);
            let item = FrameToStore {
                frame: station_frame,
                transform: snapshot,
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
) -> (
    SyncSender<StorageCommand>,
    std::thread::JoinHandle<Result<StorageStats>>,
) {
    let (tx, rx) = sync_channel(capacity);
    let handle = std::thread::spawn(move || {
        let mut frames_written = 0u64;
        for cmd in rx {
            match cmd {
                StorageCommand::Persist(item) => {
                    persister.persist_frame(*item).map_err(|error| {
                        error!(%error, frames_written, "failed to persist station-frame cloud");
                        error
                    })?;
                    frames_written += 1;
                }
                StorageCommand::Finish => {
                    persister.finish("complete").ok();
                    return Ok(StorageStats { frames_written });
                }
            }
        }
        // Channel closed without Finish → aborted
        persister.finish("aborted").ok();
        Ok(StorageStats { frames_written })
    });
    (tx, handle)
}

// ---------------------------------------------------------------------------
// Transform strategy: dispatch between legacy YPR and StationTransform
// ---------------------------------------------------------------------------

enum TransformStrategy {
    Legacy {
        transform: Box<dyn CoordinateTransform>,
    },
    Station {
        transform: StationTransform,
    },
}

impl TransformStrategy {
    fn transform(&self, frame: &Frame<PointXyzir>) -> (Frame<PointXyzir>, TransformSnapshot) {
        match self {
            Self::Legacy { transform } => (transform.transform_frame(frame), transform.snapshot()),
            Self::Station { transform } => {
                let out = transform.transform_frame_to_target(frame);
                let snapshot = transform.snapshot();
                (out, snapshot)
            }
        }
    }
}

fn load_station_config_opt(path: Option<&Path>) -> Result<Option<ValidatedStationConfig>> {
    match path {
        None => Ok(None),
        Some(p) => {
            let cfg = load_station_config(p)
                .map_err(|e| QuanergyError::Config(format!("station config error: {e}")))?;
            Ok(Some(cfg))
        }
    }
}

fn check_transform_mutex(args: &TransformArgs, has_station_config: bool) -> Result<()> {
    if !has_station_config {
        // Deprecation warning for old six-parameter path
        let using_old = args.x_m != 0.0
            || args.y_m != 0.0
            || args.z_m != 0.0
            || args.yaw_deg != 0.0
            || args.pitch_deg != 0.0
            || args.roll_deg != 0.0;
        if using_old {
            warn!(
                "using --x-m/--yaw-deg etc. is deprecated; prefer --station-config with a station.toml"
            );
        }
        return Ok(());
    }

    // --station-config is set: old params must be at their defaults
    let conflicts: Vec<&str> = [
        (args.x_m != 0.0, "--x-m"),
        (args.y_m != 0.0, "--y-m"),
        (args.z_m != 0.0, "--z-m"),
        (args.yaw_deg != 0.0, "--yaw-deg"),
        (args.pitch_deg != 0.0, "--pitch-deg"),
        (args.roll_deg != 0.0, "--roll-deg"),
    ]
    .iter()
    .filter_map(|(conflict, name)| if *conflict { Some(*name) } else { None })
    .collect();

    if !conflicts.is_empty() {
        return Err(QuanergyError::Config(format!(
            "--station-config is mutually exclusive with: {}. Remove these flags and define the transform in station.toml instead.",
            conflicts.join(", ")
        ))
        .into());
    }

    Ok(())
}

fn build_transform_strategy(
    args: &TransformArgs,
    station_config: &Option<ValidatedStationConfig>,
) -> Result<TransformStrategy> {
    match station_config {
        Some(sc) => {
            let transform = StationTransform::new(
                &sc.source_frame,
                &sc.target_frame,
                &sc.extrinsic_id,
                sc.sensor_to_station,
            );
            Ok(TransformStrategy::Station { transform })
        }
        None => {
            let transform = build_legacy_transform(args);
            Ok(TransformStrategy::Legacy { transform })
        }
    }
}

fn build_legacy_transform(args: &TransformArgs) -> Box<dyn CoordinateTransform> {
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

// ---------------------------------------------------------------------------
// Station config validation subcommand
// ---------------------------------------------------------------------------

fn validate_station_config(path: &Path) -> Result<()> {
    let cfg = load_station_config(path)
        .map_err(|e| QuanergyError::Config(format!("station config error: {e}")))?;

    println!("✅ station.toml is valid");
    println!();
    println!("  station_id:       {}", cfg.station_id);
    println!("  source_frame:     {}", cfg.source_frame);
    println!("  target_frame:     {}", cfg.target_frame);
    println!("  extrinsic_id:     {}", cfg.extrinsic_id);
    println!("  config_hash:      {}", cfg.config_hash());
    println!();

    let m = cfg.sensor_to_station;
    println!("  transform matrix:");
    for row in &m {
        println!(
            "    [{:>8.4}, {:>8.4}, {:>8.4}, {:>8.4}]",
            row[0], row[1], row[2], row[3]
        );
    }
    println!();

    let scanner_origin = [m[0][3], m[1][3], m[2][3]];
    println!(
        "  scanner origin (station): ({:.3}, {:.3}, {:.3}) m",
        scanner_origin[0], scanner_origin[1], scanner_origin[2]
    );
    println!();

    let geom = StationGeometry::from_config(&cfg);
    println!("  hammers ({} total):", geom.hammers.len());
    for h in &geom.hammers {
        let status = if h.enabled { "enabled" } else { "disabled" };
        println!(
            "    {}  y={:>8.3}  x={:>8.3}  roi_x=±{:.3}  roi_y=±{:.3}  z=[{:.3}, {:.3}]  {}",
            h.id,
            h.center_y_m,
            h.center_x_m,
            (h.roi.max_x - h.roi.min_x) / 2.0,
            (h.roi.max_y - h.roi.min_y) / 2.0,
            h.roi.min_z,
            h.roi.max_z,
            status,
        );
    }

    if let Some(calib) = &cfg.calibration {
        println!();
        println!("  calibration:");
        println!("    method:        {}", calib.method);
        println!("    calibrated_at: {}", calib.calibrated_at);
        println!("    rms_error_m:   {}", calib.rms_error_m);
        println!("    max_error_m:   {}", calib.max_error_m);
        println!("    notes:         {}", calib.notes);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Session config snapshot
// ---------------------------------------------------------------------------

fn copy_station_config_snapshot(
    output_dir: &Path,
    session_id: &str,
    station_config_path: Option<&Path>,
) {
    let source = match station_config_path {
        Some(p) => p,
        None => return,
    };

    let dest_dir = output_dir.join("sessions").join(session_id);
    if let Err(e) = std::fs::create_dir_all(&dest_dir) {
        warn!(
            "failed to create session config dir {}: {e}",
            dest_dir.display()
        );
        return;
    }

    let dest = dest_dir.join("station.toml");
    match std::fs::copy(source, &dest) {
        Ok(_) => info!("station config saved to {}", dest.display()),
        Err(e) => warn!("failed to copy station config to {}: {e}", dest.display()),
    }
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
            Command::ValidateStationConfig(_) => panic!("expected live command"),
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
            Command::ValidateStationConfig(_) => panic!("expected live command"),
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
            Command::ValidateStationConfig(_) => panic!("expected replay command"),
        }
    }

    #[test]
    fn station_config_global_arg_parses() {
        let cli = Cli::try_parse_from([
            "capture-store",
            "--station-config",
            "config/station.toml",
            "live",
            "--host",
            "192.0.2.10",
        ])
        .unwrap();

        assert_eq!(
            cli.station_config,
            Some(PathBuf::from("config/station.toml"))
        );
    }

    #[test]
    fn validate_station_config_subcommand_parses() {
        let cli = Cli::try_parse_from([
            "capture-store",
            "validate-station-config",
            "some/path/station.toml",
        ])
        .unwrap();

        match cli.command {
            Command::ValidateStationConfig(args) => {
                assert_eq!(args.file, PathBuf::from("some/path/station.toml"));
            }
            _ => panic!("expected validate-station-config"),
        }
    }
}
