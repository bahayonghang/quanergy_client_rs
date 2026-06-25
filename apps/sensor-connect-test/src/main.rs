use std::{
    io::{self, Write},
    net::ToSocketAddrs,
};

use clap::Parser;
use quanergy_client::{
    cloud::{Frame, PointXyzir},
    config::{DeviceInfo, PipelineConfig},
    error::QuanergyError,
    net::{fetch_device_info_xml, TcpPacketSource},
    pipeline::SensorPipeline,
    protocol::DEFAULT_TCP_PORT,
};
use thiserror::Error;
use tracing::{debug, error, warn};
use tracing_subscriber::EnvFilter;

type Result<T> = std::result::Result<T, ConnectTestError>;

#[derive(Debug, Error)]
enum ConnectTestError {
    #[error("{0}")]
    Quanergy(#[from] QuanergyError),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Quanergy sensor connectivity diagnostic tool — prints partial point cloud"
)]
struct Cli {
    /// Sensor IP address or hostname
    #[arg(long, short = 'H')]
    host: Option<String>,

    /// TCP port (default: 4141)
    #[arg(long, default_value_t = DEFAULT_TCP_PORT)]
    port: u16,

    /// Number of frames to collect before exiting
    #[arg(long, default_value_t = 3)]
    max_frames: u64,

    /// Number of points to display per frame
    #[arg(long, default_value_t = 10)]
    max_points_per_frame: usize,

    /// Enable debug logging
    #[arg(long, short = 'v')]
    verbose: bool,
}

fn main() {
    let cli = Cli::parse();

    init_logging(cli.verbose);

    let host = match &cli.host {
        Some(h) => h.clone(),
        None => {
            eprintln!(
                "Error: --host is required.\n\n\
                Usage: sensor-connect-test --host <SENSOR_IP>\n\
                Example: sensor-connect-test --host 192.168.1.100\n\
                         sensor-connect-test --host 192.168.1.100 --max-frames 5 --max-points-per-frame 20"
            );
            std::process::exit(1);
        }
    };

    if let Err(error) = run(&host, cli.port, cli.max_frames, cli.max_points_per_frame) {
        eprintln!("\nFATAL: {error}");
        std::process::exit(1);
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

fn run(host: &str, port: u16, max_frames: u64, max_points_per_frame: usize) -> Result<()> {
    println!("╔══════════════════════════════════════════════════════╗");
    println!("║  Quanergy Sensor Connectivity Test                  ║");
    println!("╚══════════════════════════════════════════════════════╝");
    println!();
    println!("Target: {host}:{port}");
    println!();

    // ── Step 1: DNS resolution ──
    print!("[1/4] Resolving hostname... ");
    io::stdout().flush().ok();
    let _addr = match format!("{host}:{port}").to_socket_addrs() {
        Ok(mut addrs) => {
            let first = match addrs.next() {
                Some(a) => a,
                None => {
                    println!("FAILED");
                    eprintln!("  → No addresses resolved for {host}");
                    std::process::exit(1);
                }
            };
            println!("OK → {first}");
            first
        }
        Err(e) => {
            println!("FAILED");
            eprintln!("  → DNS resolution error: {e}");
            std::process::exit(1);
        }
    };

    // ── Step 2: TCP connect ──
    print!("[2/4] Connecting to sensor TCP stream... ");
    io::stdout().flush().ok();
    let mut source = match TcpPacketSource::connect_port(host, port) {
        Ok(s) => {
            println!("OK");
            s
        }
        Err(e) => {
            println!("FAILED");
            eprintln!("  → Connection error: {e}");
            eprintln!("  → Check that:");
            eprintln!("      - The sensor is powered on and reachable at {host}");
            eprintln!("      - Port {port} is not blocked by a firewall");
            eprintln!("      - No other client is already connected");
            std::process::exit(1);
        }
    };

    // ── Step 3: DeviceInfo ──
    print!("[3/4] Fetching deviceInfo... ");
    io::stdout().flush().ok();
    let mut config = PipelineConfig::default();
    match fetch_device_info_xml(host).and_then(|xml| DeviceInfo::parse_xml(&xml)) {
        Ok(info) => {
            println!("OK");
            println!("  → Model: {:?}", info.model);
            if let Some(ref va) = info.vertical_angles {
                println!("  → Vertical angles ({}): {:?}", va.len(), va);
            }
            if let (Some(amp), Some(phase)) = (info.encoder_amplitude, info.encoder_phase) {
                println!("  → Encoder amplitude={amp}, phase={phase}");
            }
            config.apply_device_info(&info);
        }
        Err(e) => {
            println!("WARNING (continuing with defaults)");
            warn!("deviceInfo unavailable: {e}");
            println!("  → Using default M8 calibration parameters");
        }
    }

    // ── Step 4: Pipeline & capture ──
    println!("[4/4] Building pipeline and capturing frames...");
    let mut pipeline = SensorPipeline::new(config)?;
    let mut stats = CaptureStats::default();

    loop {
        let packet = match source.next_packet() {
            Ok(p) => {
                stats.packets_received += 1;
                stats.has_received_good_packet = true;
                p
            }
            Err(e) => {
                stats.packets_received += 1;

                if stats.has_received_good_packet {
                    stats.bad_packets += 1;
                    error!("[packet #{}] read error: {e}", stats.packets_received);
                } else {
                    stats.warmup_errors += 1;
                    println!(
                        "  (packet #{}) 等待传感器推送数据...",
                        stats.packets_received
                    );
                }

                if stats.consecutive_errors >= 10 {
                    if !stats.has_received_good_packet {
                        eprintln!(
                            "\n传感器在 {} 次尝试内未开始推送数据。请检查传感器是否正在运行。",
                            stats.consecutive_errors
                        );
                    } else {
                        eprintln!(
                            "\nToo many consecutive errors ({}). Aborting.",
                            stats.consecutive_errors
                        );
                    }
                    break;
                }
                stats.consecutive_errors += 1;
                continue;
            }
        };
        stats.consecutive_errors = 0;

        debug!(
            "[packet #{}] type=0x{:02x} size={}",
            stats.packets_received,
            packet.header.packet_type,
            packet.header.size
        );

        let frames = match pipeline.process_raw(&packet) {
            Ok(f) => f,
            Err(e) => {
                stats.bad_packets += 1;
                warn!(
                    "[packet #{}] parse error for type 0x{:02x}: {e}",
                    stats.packets_received, packet.header.packet_type
                );
                continue;
            }
        };

        for frame in frames {
            if frame.points.is_empty() {
                continue;
            }
            stats.frames_emitted += 1;
            print_frame(&frame, stats.frames_emitted, max_points_per_frame);

            if stats.frames_emitted >= max_frames {
                println!();
                println!("Reached target of {max_frames} frame(s).");
                print_stats(&stats);
                return Ok(());
            }
        }
    }

    print_stats(&stats);
    Ok(())
}

fn print_frame(frame: &Frame<PointXyzir>, index: u64, max_points: usize) {
    let sep = "─".repeat(60);
    println!();
    println!("{sep}");
    println!(
        "Frame #{index}  |  timestamp: {} μs  |  points: {}",
        frame.stamp_micros,
        frame.points.len()
    );
    println!("{sep}");

    if frame.points.is_empty() {
        println!("  (empty frame)");
        return;
    }

    println!(
        "  {:<4} {:>10} {:>10} {:>10} {:>10} {:>6}",
        "#", "x", "y", "z", "intensity", "ring"
    );
    println!("  {:<4} {:>10} {:>10} {:>10} {:>10} {:>6}", "───", "──", "──", "──", "────", "──");

    let count = max_points.min(frame.points.len());
    for (i, p) in frame.points.iter().take(count).enumerate() {
        let (x, y, z) = if p.x.is_nan() {
            ("   NaN".to_owned(), "   NaN".to_owned(), "   NaN".to_owned())
        } else {
            (
                format!("{:>10.3}", p.x),
                format!("{:>10.3}", p.y),
                format!("{:>10.3}", p.z),
            )
        };
        println!(
            "  {:<4} {} {} {} {:>10.1} {:>6}",
            i, x, y, z, p.intensity, p.ring
        );
    }

    if frame.points.len() > count {
        println!("  ... ({} more points omitted)", frame.points.len() - count);
    }
}

#[derive(Debug, Default)]
struct CaptureStats {
    packets_received: u64,
    bad_packets: u64,
    frames_emitted: u64,
    consecutive_errors: u32,
    warmup_errors: u64,
    has_received_good_packet: bool,
}

fn print_stats(stats: &CaptureStats) {
    let sep = "═".repeat(60);
    println!();
    println!("{sep}");
    println!("  Capture Summary");
    println!("{sep}");
    println!("  Packets received:  {}", stats.packets_received);
    println!("  Bad packets:       {}", stats.bad_packets);
    println!("  Frames emitted:    {}", stats.frames_emitted);
    let good = stats.packets_received.saturating_sub(stats.bad_packets);
    println!("  Good packets:      {good}");
    if stats.warmup_errors > 0 {
        println!("  Warm-up timeouts:  {}  (传感器启动期间正常)", stats.warmup_errors);
    }
    if stats.packets_received > 0 {
        let pct = good as f64 / stats.packets_received as f64 * 100.0;
        println!("  Success rate:      {pct:.1}%");
    }
    println!("{sep}");
}
