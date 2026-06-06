use std::{
    env,
    ffi::{OsStr, OsString},
    io::{self, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use clap::{Args, Parser, Subcommand};
use quanergy_client::{
    config::{DeviceInfo, EncoderMode, PipelineConfig},
    error::QuanergyError,
    net::{fetch_device_info_xml, TcpPacketSource},
    pipeline::SensorPipeline,
    replay::{QrawReader, QrawWriter, SidecarMetadata},
};
use thiserror::Error;
use tracing::warn;
use tracing_subscriber::EnvFilter;

pub mod rerun_sink;

use rerun_sink::{RerunOutput, RerunSink, VisualizerConfig, VisualizerSink};

const DEFAULT_COMMAND: &str = "live";

pub type Result<T> = std::result::Result<T, VisualizerError>;

#[derive(Debug, Error)]
pub enum VisualizerError {
    #[error("{0}")]
    Quanergy(#[from] QuanergyError),

    #[error("visualizer error: {0}")]
    Rerun(String),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
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
    Live(LiveVisualizerArgs),
    Replay(ReplayVisualizerArgs),
    Record(RecordArgs),
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

#[derive(Debug, Args)]
struct RerunArgs {
    #[arg(long = "rerun-connect")]
    rerun_connect: Option<String>,

    #[arg(long = "rerun-save")]
    rerun_save: Option<PathBuf>,

    #[arg(long = "visualizer-max-points", default_value_t = 300_000)]
    visualizer_max_points: usize,
}

#[derive(Debug, Args)]
struct LiveVisualizerArgs {
    #[command(flatten)]
    common: CommonArgs,

    #[command(flatten)]
    rerun: RerunArgs,

    #[arg(long)]
    record: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct ReplayVisualizerArgs {
    input: PathBuf,

    #[command(flatten)]
    common: CommonArgs,

    #[command(flatten)]
    rerun: RerunArgs,

    #[arg(long)]
    realtime: bool,
}

#[derive(Debug, Args)]
struct RecordArgs {
    #[command(flatten)]
    common: CommonArgs,

    output: PathBuf,
}

#[derive(Debug)]
struct Launch {
    cli: Cli,
    pause_on_missing_host: bool,
}

pub fn main_entry() {
    let launch = match parse_launch_from(env::args_os()) {
        Ok(launch) => launch,
        Err(error) => error.exit(),
    };

    init_logging(launch.cli.verbose);
    if let Err(error) = run(launch.cli) {
        eprintln!("Error: {error}");
        if launch.pause_on_missing_host && is_missing_host_error(&error) {
            pause_for_enter();
        }
        std::process::exit(1);
    }
}

fn parse_launch_from<I, T>(args: I) -> std::result::Result<Launch, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    let no_user_args = args.len() == 1;
    let (args, defaulted_to_live) = default_live_args(args);
    Ok(Launch {
        cli: Cli::try_parse_from(args)?,
        pause_on_missing_host: no_user_args && defaulted_to_live,
    })
}

fn default_live_args(mut args: Vec<OsString>) -> (Vec<OsString>, bool) {
    if args.is_empty() || requests_root_metadata(&args[1..]) || !should_default_to_live(&args[1..])
    {
        return (args, false);
    }

    args.insert(1, OsString::from(DEFAULT_COMMAND));
    (args, true)
}

fn requests_root_metadata(args: &[OsString]) -> bool {
    for arg in args {
        if is_global_flag(arg, "verbose") || is_global_flag(arg, "strict") {
            continue;
        }
        return is_help_or_version(arg);
    }
    false
}

fn should_default_to_live(args: &[OsString]) -> bool {
    if args.is_empty() {
        return true;
    }

    for arg in args {
        if is_global_flag(arg, "verbose") || is_global_flag(arg, "strict") {
            continue;
        }
        if arg.to_string_lossy().starts_with('-') {
            return true;
        }
        return false;
    }
    true
}

fn is_global_flag(arg: &OsStr, name: &str) -> bool {
    let long = format!("--{name}");
    arg == long.as_str()
}

fn is_help_or_version(arg: &OsStr) -> bool {
    arg == "--help" || arg == "-h" || arg == "--version" || arg == "-V"
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Live(args) => run_visualizer_live(args, cli.strict),
        Command::Replay(args) => run_visualizer_replay(args, cli.strict),
        Command::Record(args) => run_record(args, cli.strict),
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

fn run_visualizer_live(args: LiveVisualizerArgs, strict: bool) -> Result<()> {
    let mut config = build_config(&args.common, strict)?;
    let device_info_error = enrich_from_device_info(&mut config)
        .err()
        .map(|error| error.to_string());
    let host = require_host(&config)?;
    let mut source = TcpPacketSource::connect(&host)?;
    let mut pipeline = SensorPipeline::new(config.clone())?;
    let mut sink = RerunSink::new(&visualizer_config(&args.rerun))?;
    let mut recorder = if let Some(path) = &args.record {
        write_sidecar(path, Some(host.clone()), &config, device_info_error);
        Some(QrawWriter::create(path)?)
    } else {
        None
    };

    loop {
        let packet = source.next_packet()?;
        if let Some(writer) = &mut recorder {
            writer.write_packet(packet.arrival_delta_ns, &packet.bytes)?;
        }
        for frame in pipeline.process_raw(&packet)? {
            sink.log_frame(&frame)?;
        }
    }
}

fn run_visualizer_replay(args: ReplayVisualizerArgs, strict: bool) -> Result<()> {
    let mut config = build_config(&args.common, strict)?;
    if let Ok(sidecar) = SidecarMetadata::load(SidecarMetadata::sidecar_path(&args.input)) {
        if let Some(vertical_angles) = sidecar.vertical_angles {
            config.vertical_angles = Some(vertical_angles);
        }
        if let (Some(amplitude), Some(phase)) = (sidecar.encoder_amplitude, sidecar.encoder_phase) {
            config.encoder_mode = EncoderMode::Manual { amplitude, phase };
        }
    }

    let mut reader = QrawReader::open(&args.input)?;
    let mut pipeline = SensorPipeline::new(config)?;
    let mut sink = RerunSink::new(&visualizer_config(&args.rerun))?;
    while let Some(packet) = reader.next_packet()? {
        if args.realtime && packet.arrival_delta_ns > 0 {
            std::thread::sleep(Duration::from_nanos(packet.arrival_delta_ns));
        }
        for frame in pipeline.process_raw(&packet)? {
            sink.log_frame(&frame)?;
        }
    }
    Ok(())
}

fn run_record(args: RecordArgs, strict: bool) -> Result<()> {
    let mut config = build_config(&args.common, strict)?;
    let device_info_error = enrich_from_device_info(&mut config)
        .err()
        .map(|error| error.to_string());
    let host = require_host(&config)?;
    let mut source = TcpPacketSource::connect(&host)?;
    write_sidecar(&args.output, Some(host), &config, device_info_error);
    let mut writer = QrawWriter::create(&args.output)?;
    loop {
        let packet = source.next_packet()?;
        writer.write_packet(packet.arrival_delta_ns, &packet.bytes)?;
        writer.flush()?;
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
        Err(QuanergyError::Config(missing_host_message()).into())
    } else {
        Ok(config.host.clone())
    }
}

fn missing_host_message() -> String {
    [
        "no host provided",
        "",
        "Provide a sensor host, for example:",
        "  visualizer --host <SENSOR_IP>",
        "  visualizer live --host <SENSOR_IP>",
        "  visualizer live --settings-file <client.xml>",
        "  visualizer record --host <SENSOR_IP> <OUTPUT>",
    ]
    .join("\n")
}

fn is_missing_host_error(error: &VisualizerError) -> bool {
    matches!(
        error,
        VisualizerError::Quanergy(QuanergyError::Config(message))
            if message.starts_with("no host provided")
    )
}

fn pause_for_enter() {
    eprint!("Press Enter to close this window...");
    let _ = io::stderr().flush();
    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);
}

fn visualizer_config(args: &RerunArgs) -> VisualizerConfig {
    let output = if let Some(path) = &args.rerun_save {
        RerunOutput::Save(path.clone())
    } else if let Some(addr) = &args.rerun_connect {
        RerunOutput::Connect(addr.clone())
    } else {
        RerunOutput::Spawn
    };
    VisualizerConfig {
        output,
        max_points: args.visualizer_max_points,
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

#[cfg(test)]
mod tests {
    use clap::error::ErrorKind;

    use super::*;

    #[test]
    fn no_args_default_to_live_and_pause_on_missing_host() {
        let launch = parse_launch_from(["visualizer"]).unwrap();
        assert!(launch.pause_on_missing_host);

        let error = run(launch.cli).unwrap_err();
        assert!(is_missing_host_error(&error));
        assert!(error.to_string().contains("visualizer --host"));
    }

    #[test]
    fn top_level_host_defaults_to_live_without_pause() {
        let launch = parse_launch_from(["visualizer", "--host", "192.0.2.10"]).unwrap();
        assert!(!launch.pause_on_missing_host);

        match launch.cli.command {
            Command::Live(args) => {
                assert_eq!(args.common.host.as_deref(), Some("192.0.2.10"));
            }
            Command::Replay(_) | Command::Record(_) => panic!("expected live visualizer"),
        }
    }

    #[test]
    fn explicit_live_is_not_marked_as_double_click_launch() {
        let launch = parse_launch_from(["visualizer", "live"]).unwrap();
        assert!(!launch.pause_on_missing_host);

        match launch.cli.command {
            Command::Live(_) => {}
            Command::Replay(_) | Command::Record(_) => panic!("expected live visualizer"),
        }
    }

    #[test]
    fn record_command_owns_qraw_capture() {
        let launch =
            parse_launch_from(["visualizer", "record", "--host", "192.0.2.10", "out.qraw"])
                .unwrap();

        match launch.cli.command {
            Command::Record(args) => {
                assert_eq!(args.common.host.as_deref(), Some("192.0.2.10"));
                assert_eq!(args.output, PathBuf::from("out.qraw"));
            }
            Command::Live(_) | Command::Replay(_) => panic!("expected record command"),
        }
    }

    #[test]
    fn root_help_stays_root_help() {
        let error = parse_launch_from(["visualizer", "--help"]).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::DisplayHelp);
    }

    #[test]
    fn old_nested_visualizer_command_is_not_supported() {
        let error = parse_launch_from(["visualizer", "visualizer", "live"]).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidSubcommand);
    }

    #[test]
    fn dynamic_connection_is_not_part_of_visualizer_app() {
        let error = parse_launch_from(["visualizer", "dynamic-connection"]).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidSubcommand);
    }
}
