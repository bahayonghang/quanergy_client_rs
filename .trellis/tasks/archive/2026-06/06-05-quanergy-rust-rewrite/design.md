# Quanergy Rust Rewrite Design

## Architecture

The first milestone is a Rust functional rewrite of the original Quanergy SDK
surface used by `visualizer` and `dynamic_connection`. It does not include
tamping-station storage, station-frame transforms, ROI processing, or height
measurement.

The crate exposes a reusable library plus a thin CLI:

```text
src/
  lib.rs
  main.rs
  cloud.rs
  config.rs
  error.rs
  net.rs
  replay.rs
  protocol/
  calibration/
  filters.rs
  pipeline.rs
  visualizer.rs
```

CLI commands call library APIs only. Protocol, parsing, calibration, filtering,
frame assembly, replay, and visualization logic must not live in `main.rs`.

## Data Flow

Live visualizer:

```text
TCP 4141 -> PacketSource -> ParserDispatch -> HVDIR frame
  -> encoder calibration/correction -> filters -> XYZIR frame
  -> display-only downsample -> Rerun sink
```

Replay visualizer:

```text
.qraw + .qraw.toml -> PacketSource -> same parser/pipeline/Rerun sink
```

Record:

```text
TCP 4141 -> .qraw packet stream + .qraw.toml sidecar
```

Dynamic connection reuses the same source and pipeline, with start/stop control
and calibration reset behavior.

## Protocol Contracts

- Packet header is 20 bytes, big-endian, signature `0x75bd7e97`.
- Parser dispatch supports packet types `0x00`, `0x01`, `0x04`, and `0x06`.
- M-Series parser logic owns the 10400 horizontal-angle LUT, vertical angles,
  return selection, cloud size limits, direction detection, and sweep-based
  frame completion.
- `.qraw` v1 stores magic/version, then repeated records:
  `delta_ns`, `packet_len`, complete original Quanergy packet bytes.
- `.qraw.toml` sidecar stores capture metadata, deviceInfo/calibration fields
  when available, and calibration-incomplete error details when unavailable.

## Calibration And Config

- Live mode obtains deviceInfo from HTTP port `7780` path
  `/PSIA/System/deviceInfo` when possible.
- Replay calibration precedence is explicit CLI/XML override, sidecar metadata,
  M8 defaults, MQ error without vertical angles, and M1 no M-Series vertical
  angle requirement.
- Automatic M-Series encoder calibration implements the C++ SDK core loop:
  complete revolution detection, timeout, phase convergence, amplitude
  threshold, parameter calculation, and post-completion correction. It excludes
  `calibrateOnly` CSV/debug output.
- XML settings compatibility is required. Ring filter parsing accepts both
  lowercase `range0/intensity0` and sample XML uppercase `Range0/Intensity0`.

## Visualization

- Rerun is the first visualizer backend.
- Default output mode spawns Rerun Viewer.
- CLI also supports connecting to an existing viewer and saving `.rrd` files.
- Points are colored by intensity. Display output is capped to 300000 points per
  frame by default; `--visualizer-max-points 0` disables display downsampling.
- Display downsampling never changes pipeline, replay, or recorded data.

## Failure Modes

- Recoverable parser/packet errors are lenient by default: skip, count, and log.
- `--strict` fails on first recoverable error.
- Unrecoverable live stream errors close and reconnect when the source supports
  it; replay stops and reports the error.
- Packet ingestion uses bounded queues so visualization cannot stall network
  reads indefinitely.
