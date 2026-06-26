# Tamping Station Point Cloud Storage Design

## Feasibility Judgment

The proposed acquisition and storage path is feasible with the current Rust SDK
as the foundation.

Current SDK capability already covers the live Quanergy capture and point-cloud
pipeline through `Frame<PointXyzir>`:

```text
TCP 4141
  -> PacketHeader validation
  -> RawPacket
  -> parser dispatch for 0x00 / 0x01 / 0x04 / 0x06
  -> HVDIR frames
  -> encoder correction / filters
  -> XYZIR frames in sensor coordinates
```

The missing business layer is:

```text
Frame<PointXyzir> in sensor frame
  -> T_station_sensor rigid transform
  -> Frame<PointXyzir> in station frame
  -> one binary point-cloud file per frame
  -> database metadata row per frame
  -> downstream Rust reader
```

This does not require returning to the C++ SDK or PCL. The existing Rust code
already uses the official SDK behavior as the reference for packet parsing,
calibration, frame assembly, and HVDIR-to-XYZIR conversion.

## Architecture Boundaries

Recommended modules for the first storage slice:

```text
crates/quanergy-client/src/transform/
    mod.rs                # CoordinateTransform trait and frame helpers
    pose.rs               # default yaw/pitch/roll pose transform
    matrix.rs             # reusable rigid 4x4 representation

crates/quanergy-client/src/storage/
    qpcd.rs               # binary frame file format
    metadata.rs           # database DTOs and frame metadata contracts
    sqlite.rs             # embedded SQLite writer/reader if SQLite is chosen

apps/capture-store/
    src/main.rs           # thin CLI around SDK pipeline + transform + storage
```

The core library should own reusable transform and storage contracts. The app
should own CLI parsing, host config, output directory selection, and lifecycle
orchestration.

The visualizer should remain separate. It can keep using `Frame<PointXyzir>`
directly and should not become responsible for station storage.

## Data Flow

Live storage path:

```text
TcpPacketSource::next_packet()
  -> optional QrawWriter only when --record-raw is set
  -> SensorPipeline::process_raw()
  -> for each complete Frame<PointXyzir>:
       apply T_station_sensor
       write frames/YYYY-MM-DD/frame_#########.qpcd
       insert scan_frame metadata row in SQLite
```

Replay storage path:

```text
QrawReader + qraw sidecar
  -> SensorPipeline
  -> station transform
  -> qpcd + metadata
```

The replay path matters because field validation can be repeated without the
sensor connected.

## Coordinate Contract

The transform should be a rigid 4x4 homogeneous matrix:

```text
p_station = T_station_sensor * [x_sensor, y_sensor, z_sensor, 1]
```

Do not require users to hand-author the matrix for normal field operation.
Expose a pose-based API that builds the matrix from scanner installation
parameters:

```text
scanner_position_m = { x, y, z } relative to station origin
scanner_orientation = { yaw_deg, pitch_deg, roll_deg }
T_station_sensor = pose_to_transform(scanner_position_m, scanner_orientation)
```

The station origin is a field-chosen fixed lower-left point on the station/site.
Because the scanner is fixed and the station is not moving relative to the
coordinate frame, the first version does not need any vehicle-motion or
time-varying platform transform. The `x_m`, `y_m`, and `z_m` pose fields mean
the scanner origin measured from this lower-left station origin.

Station axes:

```text
X: right from the lower-left origin
Y: upward / deeper into the station drawing from the lower-left origin
Z: vertically upward
unit: meter
```

This should be implemented as a normal SDK method so live capture, qraw replay,
and later height measurement can all apply the exact same transform logic. A
matrix import can still be supported later for advanced calibration, but the
first interface should match how the field team adjusts the scanner: origin
offset plus tilt/orientation angles.

The default algorithm should be named and treated as a default strategy, for
example `YawPitchRollTransform`. The storage app and SDK frame helper should
depend on a replaceable interface:

```rust
pub trait CoordinateTransform {
    fn name(&self) -> &'static str;
    fn transform_point(&self, point: PointXyzir) -> PointXyzir;
    fn matrix_4x4(&self) -> Option<[[f32; 4]; 4]>;
    fn config_snapshot(&self) -> TransformSnapshot;
}
```

The exact trait shape can be adjusted during implementation, but the boundary is
required: capture, replay, qpcd writing, and database metadata must not call the
yaw/pitch/roll math directly. This keeps future algorithms possible, including
hand-authored matrices, calibration-board solutions, station-specific correction
maps, or other field-defined transforms.

Store the matrix with every frame metadata record or reference an immutable
extrinsic calibration record by id. For the first implementation, storing a
copy in `scan_frame.transform_4x4` is simplest and avoids ambiguity if config
changes during a run.

Also store a JSON/TOML snapshot of the human-readable pose parameters used to
derive that matrix. If field staff adjust the scanner pose during a run, frames
before and after the adjustment remain traceable.

For the default transform, the snapshot should include:

```text
algorithm = "yaw_pitch_roll_pose"
x_m
y_m
z_m
yaw_deg
pitch_deg
roll_deg
```

Custom algorithms should store their algorithm name and algorithm-specific
configuration in the same snapshot field.

Point fields after transform:

```text
x, y, z      station-frame coordinates in meters using the agreed X/Y/Z axes
intensity    unchanged
ring         unchanged
```

The station-axis convention must be documented in code comments or config
schema text near the transform config so future field adjustments do not invert
ROI or height semantics.

## Storage Recommendation

Use an embedded metadata database plus one binary point-cloud file per frame.

Confirmed first database: local SQLite via `rusqlite`.

Reasons:

- Fits the current single-machine Windows deployment.
- Avoids requiring a database server at the acquisition machine.
- Provides simple transactional metadata updates.
- Is enough for frame enumeration, session filtering, provenance, and later
  measurement-result tables.

Do not store each point as a SQL row in production. At roughly 430k to 1.3M
points per second for M-Series, row-per-point storage creates avoidable write
amplification and query/index overhead.

## Binary Frame Format

Use a custom `.qpcd` v1 binary file for production transformed frames.

Suggested shape:

```text
magic:        "QPCDv1\0\0"
header_len:   u32
point_stride: u32
point_count:  u64
stamp_micros: u64
sequence:     u64
coord_frame:  utf8 string in header metadata
points:       repeated DiskPointXYZIR
```

Suggested point layout:

```rust
#[repr(C)]
struct DiskPointXYZIR {
    x: f32,
    y: f32,
    z: f32,
    intensity: f32,
    ring: u16,
    flags: u16,
}
```

The format should include enough header information to reject incompatible
files without consulting the database. The database remains the index and
provenance store.

PCD can be supported as an optional debug export, not the production write path.

## Initial Schema

Minimal schema:

```sql
CREATE TABLE capture_session (
    session_id        TEXT PRIMARY KEY,
    started_at        TEXT NOT NULL,
    ended_at          TEXT,
    sensor_host       TEXT NOT NULL,
    sensor_model      TEXT,
    sdk_version       TEXT NOT NULL,
    status            TEXT NOT NULL,
    notes             TEXT
);

CREATE TABLE scan_frame (
    frame_id          INTEGER PRIMARY KEY,
    session_id        TEXT NOT NULL,
    sequence          INTEGER NOT NULL,
    timestamp_micros  INTEGER NOT NULL,
    sensor_host       TEXT NOT NULL,
    sensor_model      TEXT,
    packet_type_mask  INTEGER,
    point_count       INTEGER NOT NULL,
    coord_frame       TEXT NOT NULL,
    transform_4x4     BLOB NOT NULL,
    transform_json    TEXT NOT NULL,
    calibration_json  TEXT NOT NULL,
    cloud_path        TEXT NOT NULL,
    qraw_path         TEXT,
    status            TEXT NOT NULL,
    created_at        TEXT NOT NULL,
    UNIQUE(session_id, sequence)
);
```

Later height-calculation work can add result tables such as
`hammer_group_height`, but this task should not require them.

The first version intentionally does not add `work_cycle`, `pass`, or
station-event tables. Work-cycle context may be captured as session metadata
such as `notes` or `status`, then normalized later if the measurement workflow
needs it.

## Write Safety

Frame persistence should be two-phase:

1. Write `.qpcd.tmp`.
2. Flush/close the file.
3. Rename to `.qpcd`.
4. Insert or update the `scan_frame` row as `complete`.

If the process dies mid-frame, the database should not advertise an incomplete
cloud file as complete. Startup cleanup can delete stale `.tmp` files.

## Backpressure

The live path should use bounded channels:

```text
capture thread -> parser/pipeline -> storage worker
```

If storage falls behind, the first implementation should prefer explicit
backpressure/drop accounting over unbounded memory growth. The exact policy
can be chosen during implementation, but counters must expose dropped or
skipped frames.

## Validation Strategy

Unit tests:

- Transform identity leaves coordinates unchanged.
- Simple translation/rotation maps known points to expected station points.
- Pose-to-transform maps a scanner origin offset and a known tilt angle to the
  expected 4x4 transform.
- Storage code can accept a test transform implementation that is not the
  default yaw/pitch/roll transform.
- `.qpcd` writer/reader round-trips a small `Frame<PointXyzir>`.
- SQLite metadata insert points to a readable `.qpcd` file.

Integration tests:

- Existing synthetic packet pipeline produces a frame, transforms it, writes
  `.qpcd`, inserts metadata, and reads the file back.
- Replay path consumes a `.qraw` fixture or synthetic qraw, then persists at
  least one transformed frame.

Field checks:

- Capture header/deviceInfo proves real packet type/version and calibration.
- Known plane/wall/target dimensions look correct in station coordinates.
- Height axis sign and units match physical measurement.

## Trade-offs

SQLite vs PostgreSQL:

- SQLite is confirmed for the first version because acquisition is local and
  embedded.
- PostgreSQL is deferred. It is only relevant if multiple machines must query
  live data concurrently or if long-term enterprise retention is centralized
  from day one.

Custom `.qpcd` vs PCD:

- `.qpcd` is better for high-rate production writes and exact binary contracts.
- PCD is useful for debug/tool interoperability but slower and bulkier.

Store transform copy vs transform id only:

- Copying `transform_4x4` into each frame is redundant but robust.
- Copying the editable pose config as `transform_json` makes field adjustments
  auditable without reverse-engineering the matrix.
- A normalized `extrinsic_calibration` table is cleaner for long runs and can be
  added later if the first version proves the workflow.

Replaceable transform interface vs direct yaw/pitch/roll function:

- A transform interface is slightly more structure than a single function, but
  it prevents capture/storage code from depending on one coordinate algorithm.
- A direct function is simpler for the first commit but would make future
  calibration-specific replacements harder and more invasive.

Raw `.qraw` retention:

- First-version production storage does not continuously save `.qraw` by
  default.
- `--record-raw` enables synchronized `.qraw` plus sidecar recording for
  debugging, parser verification, or calibration sessions.
- This keeps normal disk usage focused on transformed `.qpcd` frames and SQLite
  metadata while preserving a deliberate raw-replay path.

## Main Open Design Dependency

The repository can answer SDK feasibility, parser support, and current storage
gaps. The first-version product decisions are resolved for planning; final
artifact review is required before implementation starts.
