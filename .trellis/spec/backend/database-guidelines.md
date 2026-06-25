# Database Guidelines

> Database and production point-cloud storage contracts for this project.

---

## Overview

The first production storage path uses local SQLite metadata with one binary
point-cloud file per completed frame. Do not store individual point-cloud
points as SQL rows in production paths; keep SQLite as the frame/session index
and provenance store.

---

## Scenario: Tamping Station Frame Metadata

### 1. Scope / Trigger

- Trigger: changes to station-frame capture, transformed point-cloud storage,
  `.qpcd` files, SQLite metadata, or downstream frame enumeration.
- Scope: `crates/quanergy-client/src/storage/*` owns reusable storage
  contracts. `apps/capture-store` may orchestrate capture/replay, but it must
  not redefine the database schema or binary point layout.

### 2. Signatures

- App commands:
  - `capture-store live --host <SENSOR_IP> [OPTIONS]`
  - `capture-store replay <INPUT.qraw> [OPTIONS]`
- Default transform fields:
  - `--x-m`, `--y-m`, `--z-m`
  - `--yaw-deg`, `--pitch-deg`, `--roll-deg`
- Storage switches:
  - `--output-dir <DIR>`
  - `--database <PATH>`
  - `--session-id <TEXT>`
  - `--notes <TEXT>`
  - `--coord-frame <TEXT>`
  - `--record-raw` only for live debug/calibration raw capture
- SDK APIs:
  - `write_qpcd(path, &Frame<PointXyzir>, coord_frame) -> Result<QpcdHeader>`
  - `read_qpcd(path) -> Result<(QpcdHeader, Frame<PointXyzir>)>`
  - `SqliteStore::open(path) -> Result<SqliteStore>`
  - `SqliteStore::insert_capture_session(&NewCaptureSession)`
  - `SqliteStore::insert_scan_frame(&NewScanFrame)`

### 3. Contracts

- Production frame data is stored as one `.qpcd` file per complete transformed
  `Frame<PointXyzir>`.
- SQLite stores `capture_session` and `scan_frame` rows only. First-version
  work-cycle/pass tables are out of scope.
- `scan_frame.cloud_path` points to the `.qpcd` file. `scan_frame.qraw_path`
  is nullable and is populated only when the capture/replay source has a raw
  artifact.
- `scan_frame.transform_4x4` stores the sensor-to-station matrix as a 64-byte
  little-endian `f32` blob. `scan_frame.transform_json` stores the algorithm
  snapshot, including `yaw_deg`, `pitch_deg`, and `roll_deg` for the default
  transform.
- `scan_frame.calibration_json` records calibration provenance. Do not require
  downstream readers to inspect `.qraw.toml` to understand a transformed frame.
- `.qpcd` v1 files must include magic `QPCDv1\0\0`, JSON header metadata,
  fixed point stride `20`, and repeated `x/y/z/intensity/ring/flags` records.
- `.qpcd` writes are two-phase: write `<path>.tmp`, flush/close, rename to
  final path, then insert complete metadata.
- Station coordinates are meters from the field-chosen lower-left station
  origin: `X` right, `Y` deeper/upward in the station drawing, and `Z`
  vertically upward.

### 4. Validation & Error Matrix

- Invalid `.qpcd` magic -> `QuanergyError::StorageFormat`.
- Unsupported `.qpcd` version or point stride -> `QuanergyError::StorageFormat`.
- `transform_4x4` blob length other than 64 bytes -> storage format error when
  reading SQLite metadata.
- SQLite integer overflow for `sequence`, `timestamp_micros`, or `point_count`
  -> storage format error before insert.
- Missing live `--host` -> actionable config error; do not invent a default
  sensor host.
- `--record-raw` absent -> no continuous `.qraw` recording in live mode.
- Storage queue full in live mode -> count and log dropped frames instead of
  using an unbounded queue.

### 5. Good / Base / Bad Cases

- Good: live capture writes transformed `.qpcd` files plus complete SQLite
  metadata and only writes `.qraw` when `--record-raw` is set.
- Base: replay from `.qraw` applies sidecar calibration, transforms frames, and
  writes the same `.qpcd + SQLite` shape.
- Bad: storing every point as a SQL row, or making downstream readers infer
  station transforms from mutable field config rather than frame metadata.

### 6. Tests Required

- `.qpcd` round-trip preserves frame metadata and `PointXyzir` fields.
- Invalid `.qpcd` magic is rejected.
- SQLite insert/read returns a frame row whose `cloud_path` points to a readable
  `.qpcd` file.
- CLI tests prove `--record-raw` is opt-in and `yaw_deg` / `pitch_deg` /
  `roll_deg` parse through the field-facing names.

### 7. Wrong vs Correct

Wrong: add a `point` SQL table and insert one row per XYZIR point for each
scan frame.

Correct: write one `.qpcd` file per transformed frame, then insert a
`scan_frame` row with path, transform, calibration, and count metadata.

Wrong: have `apps/capture-store` hand-build SQL separately from the SDK storage
module.

Correct: keep schema creation and metadata DTOs in `quanergy_client::storage`
and keep the app as lifecycle orchestration.

---

## Query Patterns

- Use `rusqlite` directly for local embedded metadata.
- Keep table and column names in snake_case.
- Keep frame enumeration ordered by `(session_id, sequence)`.
- Store large point-cloud payloads outside SQLite and store only paths plus
  provenance in SQLite.

---

## Migrations

The first storage slice initializes schema with `CREATE TABLE IF NOT EXISTS`.
When schema evolution becomes necessary, add explicit versioning before changing
existing table contracts.

---

## Naming Conventions

- Table names: singular domain nouns such as `capture_session` and
  `scan_frame`.
- Status fields are text values such as `running` and `complete`.
- File path columns end in `_path`.
- JSON snapshot columns end in `_json`.
- Matrix blobs use explicit shape names such as `transform_4x4`.

---

## Common Mistakes

- Do not record a `scan_frame` row before the `.qpcd` file has been fully
  written and renamed into place.
- Do not treat `.qraw` as the production transformed-frame storage format.
  `.qraw` is for replay/debug/calibration and is opt-in for live storage.
