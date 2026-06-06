# Implementation Details

This page describes the current Rust data path and the contracts future work
should preserve.

## Packet Ingestion

Live capture starts with `TcpPacketSource`:

```text
sensor_ip:4141
  -> 20-byte PacketHeader
  -> signature check 0x75bd7e97
  -> packet body
  -> RawPacket
```

The SDK also supports qraw replay:

```text
.qraw + optional .qraw.toml sidecar
  -> QrawReader
  -> RawPacket
```

When live recording is enabled, `.qraw` captures preserve raw packet bytes and
the sidecar records calibration metadata when available. If deviceInfo cannot be
read, the sidecar records an incomplete-calibration marker and error reason.

## Calibration Inputs

The library can fetch calibration metadata from:

```text
http://<sensor-host>:7780/PSIA/System/deviceInfo
```

The parsed fields include sensor model, vertical angles, encoder amplitude, and
encoder phase. Settings-file and CLI overrides can adjust the pipeline when
field conditions require manual configuration.

## Pipeline Flow

`SensorPipeline` owns parser dispatch and frame processing:

```text
RawPacket
  -> parser dispatch for 0x00 / 0x01 / 0x04 / 0x06
  -> Frame<PointHvdir>
  -> optional automatic or manual encoder correction
  -> distance filter
  -> ring/intensity filter
  -> Frame<PointXyzir>
```

In non-strict mode, bad packets are counted and dropped with a warning. In
strict mode, parser errors are returned to the caller.

## Frame Semantics

Frames are assembled by the M-Series sweep/wrap logic. A TCP packet is not
treated as a complete scan unless a caller is explicitly doing packet-level
debugging. This matters for both visualizer parity and storage quality.

`Frame<PointXyzir>` keeps:

- `frame_id`
- `stamp_micros`
- `sequence`
- organized cloud dimensions
- dense flag
- point data with `x`, `y`, `z`, `intensity`, and `ring`

## Station Transform

The storage path can transform sensor-frame XYZIR points into a station frame.
The default app-facing transform is a yaw/pitch/roll pose:

```text
x_m, y_m, z_m
yaw_deg, pitch_deg, roll_deg
```

The SDK exposes a transform boundary so later calibration algorithms can replace
the default pose transform without rewriting capture or storage orchestration.

## Point-Cloud Storage

Production storage uses one binary `.qpcd` file per completed frame. A `.qpcd`
file starts with magic `QPCDv1`, a JSON header, and repeated 20-byte XYZIR point
records:

```text
x: f32
y: f32
z: f32
intensity: f32
ring: u16
flags: u16
```

Files are written through a temporary path and renamed to the final `.qpcd`
path after the write completes.

## SQLite Metadata

`SqliteStore` creates two current tables:

| Table | Purpose |
| --- | --- |
| `capture_session` | Session identity, start/end time, sensor host/model, SDK version, status, and notes. |
| `scan_frame` | Frame sequence, timestamp, point count, coordinate frame, transform snapshot, calibration snapshot, `.qpcd` path, optional qraw path, and status. |

Per-point SQL rows are deliberately avoided. Downstream tools should enumerate
frames from SQLite, then read the referenced `.qpcd` files.
