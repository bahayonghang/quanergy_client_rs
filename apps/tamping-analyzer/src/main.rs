use std::{
    io::{self, Write},
    path::PathBuf,
};

use clap::Parser;
use quanergy_client::{
    cloud::PointXyzir,
    measure::{
        measure_hammer, segment_frame, HammerMeasurement, HammerSessionStats, TopPercentileConfig,
    },
    station::{load_station_config, HammerLayout},
    storage::{read_qpcd, ScanFrameRecord, SqliteStore},
};
use thiserror::Error;
use tracing::info;

type Result<T> = std::result::Result<T, AnalyzerError>;

#[derive(Debug, Error)]
enum AnalyzerError {
    #[error("{0}")]
    Quanergy(#[from] quanergy_client::error::QuanergyError),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Parser)]
#[command(author, version, about = "Offline tamping-hammer height measurement")]
struct Cli {
    /// Path to the capture session SQLite database.
    #[arg(long, short = 'd')]
    database: PathBuf,

    /// Session ID to analyze.
    #[arg(long, short = 's')]
    session_id: String,

    /// Path to station.toml with hammer layout.
    #[arg(long)]
    station_config: PathBuf,

    /// Output CSV file path (default: stdout).
    #[arg(long, short = 'o')]
    output_csv: Option<PathBuf>,

    /// Top percentile ratio for height estimator (default 0.1 = top 10%).
    #[arg(long, default_value_t = 0.1)]
    top_ratio: f32,

    /// Minimum valid points per hammer required for a valid measurement.
    #[arg(long, default_value_t = 10)]
    min_points: usize,
}

fn main() {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    if let Err(error) = run(cli) {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    let station_cfg = load_station_config(&cli.station_config).map_err(|e| {
        quanergy_client::error::QuanergyError::Config(format!("station config: {e}"))
    })?;
    let layout = &station_cfg.hammer_layout;

    let estimator_config = TopPercentileConfig {
        top_ratio: cli.top_ratio,
        min_valid_points: cli.min_points,
    };

    let store = SqliteStore::open(&cli.database)?;
    let frames = store.list_scan_frames(&cli.session_id)?;

    info!(
        "analysing session {} with {} frames, {} hammers",
        cli.session_id,
        frames.len(),
        layout.hammers.len(),
    );

    let mut all_measurements: Vec<HammerMeasurement> = Vec::new();

    for frame in &frames {
        let measurements = measure_frame(frame, layout, &estimator_config)?;
        // Persist to DB
        for m in &measurements {
            store.insert_hammer_measurement(
                &cli.session_id,
                m.sequence,
                &m.hammer_id,
                m.roi_point_count,
                m.valid_point_count,
                m.top_z_m,
                m.reference_z_m,
                m.height_m,
                m.z_spread_m,
                m.quality,
                &m.estimator,
                &m.status,
                &chrono_now().to_string(),
            )?;
        }
        all_measurements.extend(measurements);
    }

    // Compute per-hammer session stats
    let stats = compute_stats(&all_measurements, layout);

    // Write CSV
    let mut writer: Box<dyn Write> = match &cli.output_csv {
        Some(path) => Box::new(std::fs::File::create(path)?),
        None => Box::new(io::stdout()),
    };

    writeln!(writer, "hammer_id,frame_count,valid_frame_count,mean_top_z_m,std_top_z_m,min_top_z_m,max_top_z_m,mean_point_count")?;
    for s in &stats {
        writeln!(
            writer,
            "{},{},{},{:.4},{:.4},{:.4},{:.4},{:.1}",
            s.hammer_id,
            s.frame_count,
            s.valid_frame_count,
            s.mean_top_z_m.unwrap_or(f32::NAN),
            s.std_top_z_m.unwrap_or(f32::NAN),
            s.min_top_z_m.unwrap_or(f32::NAN),
            s.max_top_z_m.unwrap_or(f32::NAN),
            s.mean_point_count,
        )?;
    }

    info!("done — {} frames, {} hammers", frames.len(), stats.len());
    Ok(())
}

fn measure_frame(
    frame: &ScanFrameRecord,
    layout: &HammerLayout,
    config: &TopPercentileConfig,
) -> Result<Vec<HammerMeasurement>> {
    let (_, cloud) = read_qpcd(&frame.cloud_path)?;
    let points: Vec<PointXyzir> = cloud.points;

    let seg = segment_frame(&points, layout);

    let mut measurements = Vec::with_capacity(layout.hammers.len());
    for (i, hammer) in layout.hammers.iter().enumerate() {
        let m = measure_hammer(
            frame.sequence,
            &hammer.id,
            &seg.hammer_z_values[i],
            config,
            None, // reference_z_m
        );
        measurements.push(m);
    }
    Ok(measurements)
}

fn compute_stats(
    measurements: &[HammerMeasurement],
    layout: &HammerLayout,
) -> Vec<HammerSessionStats> {
    let mut stats: Vec<HammerSessionStats> = layout
        .hammers
        .iter()
        .map(|h| HammerSessionStats {
            hammer_id: h.id.clone(),
            frame_count: 0,
            valid_frame_count: 0,
            mean_top_z_m: None,
            std_top_z_m: None,
            min_top_z_m: None,
            max_top_z_m: None,
            mean_point_count: 0.0,
        })
        .collect();

    for m in measurements {
        let idx = layout
            .hammers
            .iter()
            .position(|h| h.id == m.hammer_id)
            .unwrap_or(usize::MAX);
        if idx >= stats.len() {
            continue;
        }
        let s = &mut stats[idx];
        s.frame_count += 1;

        if let Some(tz) = m.top_z_m {
            let prev_valid = s.valid_frame_count;
            s.valid_frame_count += 1;

            // Welford-like online mean/std
            let delta = tz - s.mean_top_z_m.unwrap_or(0.0);
            s.mean_top_z_m =
                Some(s.mean_top_z_m.unwrap_or(0.0) + delta / s.valid_frame_count as f32);

            s.min_top_z_m = Some(s.min_top_z_m.map_or(tz, |v| v.min(tz)));
            s.max_top_z_m = Some(s.max_top_z_m.map_or(tz, |v| v.max(tz)));

            if prev_valid > 0 {
                // Simple cumulative std (not fully online but good enough)
                let mean = s.mean_top_z_m.unwrap_or(0.0);
                let sq_diff = (tz - mean) * (tz - mean);
                s.std_top_z_m = Some(
                    ((s.std_top_z_m.unwrap_or(0.0).powi(2) * prev_valid as f32 + sq_diff)
                        / s.valid_frame_count as f32)
                        .sqrt(),
                );
            }
        }
        s.mean_point_count += m.roi_point_count as f32;
    }

    for s in &mut stats {
        if s.frame_count > 0 {
            s.mean_point_count /= s.frame_count as f32;
        }
    }

    stats
}

fn chrono_now() -> String {
    use std::time::SystemTime;
    let ts = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{ts}")
}
