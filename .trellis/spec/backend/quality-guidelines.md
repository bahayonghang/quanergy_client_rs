# Quality Guidelines

> Code quality standards for backend development.

---

## Overview

This is a Rust workspace with a core library crate (`quanergy-client`) and
multiple binary apps (`visualizer`, `capture-store`, `station-calibrate`,
`sensor-connect-test`, `tamping-analyzer`). Quality rules apply at both
levels, but library code is held to stricter standards.

- Library code: no `unwrap()`/`expect()`, no unbounded resource use, no
  platform-specific code that breaks `x86_64-pc-windows-msvc`.
- Apps: may call `expect()` at startup for configuration that is fatal if
  missing, but must not silently swallow errors in operational loops.
- The workspace uses `cargo clippy` and `cargo fmt` as lint/formatter. All
  code must pass `cargo clippy -- -D warnings` before commit.
- Tests live in `#[cfg(test)] mod tests;` blocks within the crate, not in
  a separate `tests/` directory, unless they are integration tests that
  exercise the public API only.

---

## Forbidden Patterns

- **`unwrap()` / `expect()` in library code** — always propagate with `?` or
  handle with `match`. Apps may `expect()` only at startup for
  fatal-configuration paths.
- **`Box<dyn Error>` or `.into()` error erasure** — all errors must go through
  `QuanergyError` so callers can match on variants.
- **Unbounded channels** — ingestion, storage, and visualization queues must
  use bounded channels with explicit drop-and-count behavior on overflow.
- **Tokio without a clear need** — the first milestone uses synchronous I/O
  (`std::net::TcpStream`, `ureq`). Do not introduce an async runtime unless
  a task explicitly requires it.
- **`println!` / `eprintln!` for operational logging** — use `tracing` macros
  only.
- **Platform-specific `#[cfg]` gates for Linux/macOS** — the first milestone
  targets `x86_64-pc-windows-msvc` only. Cross-platform code is welcome but
  must not complicate the Windows build.
- **One SQL row per point** — storage uses one `.qpcd` binary file per frame
  plus SQLite metadata rows. Never create a `point` table.
- **Hard-coded default sensor host** — the C++ reference explicitly forbids
  this. Missing host must produce an actionable configuration error.
- **Calling `tracing_subscriber::init()` from library code** — only binary
  `main()` functions initialize subscribers.

---

## Required Patterns

- **Result propagation via `?`** — every fallible function returns
  `crate::error::Result<T>` and uses `?` for error propagation.
- **Bounded channels for pipeline stages** — `std::sync::mpsc::sync_channel`
  with an explicit bound. Drop-and-log on overflow; never block the ingestion
  thread on a full storage/visualization queue.
- **Two-phase file writes** — write to `<path>.tmp`, flush/close, then rename
  to the final path. Never write directly to the final path.
- **`.qraw.toml` sidecar for raw recordings** — every `.qraw` file must have
  a sidecar with deviceInfo/calibration metadata, or an explicit
  calibration-incomplete marker.
- **Module-level test organization** — unit tests go in the same file as the
  code they test under `#[cfg(test)] mod tests;`. Integration tests that span
  modules go in `tests.rs` files within the crate.
- **Structured tracing fields** — use `tracing` key-value syntax
  (`warn!(%error, "msg")`), never `format!()` inside a macro.
- **Strict vs lenient parsing** — expose a `strict: bool` config flag. Strict
  mode returns the first error; lenient mode drops bad packets with a warning.

---

## Testing Requirements

### Visualizer Application Contract

#### 1. Scope / Trigger

- Trigger: changes to CLI argument parsing, executable startup behavior,
  visualizer live defaults, qraw recording, or C++ app compatibility.
- Scope: the Rust rewrite keeps the SDK library separate from the visualizer
  app. The visualizer executable follows the original SDK app naming while
  still supporting Rust replay/record workflows.

#### 2. Signatures

- `visualizer.exe`
- `visualizer.exe --host <SENSOR_IP>`
- `visualizer.exe live [OPTIONS]`
- `visualizer.exe replay <INPUT> [OPTIONS]`
- `visualizer.exe record [OPTIONS] <OUTPUT>`

#### 3. Contracts

- A missing visualizer subcommand maps to `live`.
- Top-level live visualizer flags such as `--host <SENSOR_IP>` are accepted
  through that default-live resolution path.
- Root metadata flags such as `--help` and `--version` must remain root-level
  metadata and must not be remapped to `live`.
- Explicit `live`, `replay`, and `record` subcommands keep their own parsing
  and error behavior.
- The visualizer app must not invent a default sensor host. The C++ reference
  states that no default host makes sense.
- Do not reintroduce a `quanergy-client.exe` default-to-visualizer shim.
- Do not add `dynamic_connection` to this visualizer-focused app split.

#### 4. Validation & Error Matrix

- Bare launch with no host -> actionable missing-host configuration error and
  wait for Enter so a double-clicked console remains readable.
- Explicit `visualizer.exe live` with no host -> same actionable missing-host
  configuration error, but no pause.
- Root `--help` / `--version` -> root metadata output, exit success.
- Top-level `--host <SENSOR_IP>` -> resolves to `live --host <SENSOR_IP>` and
  reaches connection logic.
- `visualizer.exe record --host <SENSOR_IP> <OUTPUT>` -> reaches qraw recording
  logic and writes the `.qraw.toml` sidecar.
- Old nested `visualizer.exe visualizer live` -> invalid subcommand.
- `visualizer.exe dynamic-connection` -> invalid subcommand.

#### 5. Good / Base / Bad Cases

- Good: `visualizer.exe --host 192.0.2.10` is equivalent to
  `visualizer.exe live --host 192.0.2.10`.
- Base: `visualizer.exe --help` prints root help.
- Bad: `visualizer.exe visualizer live` is accepted.

#### 6. Tests Required

- Unit or command tests for bare launch defaulting to live.
- Tests for top-level `--host` being parsed as live visualizer host.
- Tests proving explicit `live` is not treated as a double-click launch and
  does not request the pause behavior.
- Tests for `record --host <SENSOR_IP> <OUTPUT>` ownership of qraw capture.
- Tests for root `--help` staying root help.
- Tests rejecting old nested `visualizer` and out-of-scope `dynamic-connection`
  subcommands.
- Manual command checks with the built debug binary when changing startup
  behavior.

#### 7. Wrong vs Correct

Wrong: add a hard-coded fallback host to make double-click launch connect.

Correct: preserve the no-default-host C++ contract, default only the visualizer
app mode to `live`, and show actionable host guidance.

### Quanergy Replay Visualizer Evidence

#### 1. Scope / Trigger

- Trigger: changes to packet parsing, replay, pipeline processing, or Rerun
  visualization cross library and CLI boundaries.
- Scope: the first Rust rewrite milestone validates behavior without sensor
  hardware through generated packet fixtures, algorithm unit tests, and replay
  visualizer smoke tests.

#### 2. Signatures

- `SensorPipeline::process_packet_bytes(&[u8]) -> Result<Vec<Frame<PointXyzir>>>`
- `QrawWriter::write_packet(delta_ns, packet_bytes)`
- `QrawReader::next_packet() -> Result<Option<RawPacket>>`
- `RerunSink::save(path)` plus `RerunSink::flush_blocking()`

#### 3. Contracts

- Packet fixtures must include the complete 20-byte Quanergy header and full
  packet body.
- M-Series parser fixtures must feed enough firings to complete a 360-degree
  frame; a single TCP packet is not a scan frame.
- Rerun save-mode smoke tests must not spawn a GUI and must flush before
  asserting on the `.rrd` file.

#### 4. Validation & Error Matrix

- Invalid signature or size in lenient mode -> no frames, `bad_packets`
  increments.
- Invalid signature or size in strict mode -> return an error immediately.
- Empty replay or replay packets that emit no frames -> smoke test must fail by
  producing no nonempty `.rrd`.

#### 5. Good / Base / Bad Cases

- Good: generated `0x00`, `0x04`, and `0x06` fixtures cross the horizontal LUT
  wrap and emit one frame with expected dimensions.
- Base: `0x01` HVDIR-list fixture emits one unorganized XYZIR frame.
- Bad: malformed packet bytes are covered in both strict and lenient modes.

#### 6. Tests Required

- Parser tests for packet types `0x00`, `0x01`, `0x04`, and `0x06`.
- Strict/lenient invalid packet behavior tests.
- Replay visualizer smoke that writes `.qraw`, replays through the pipeline,
  saves `.rrd`, flushes, and asserts the `.rrd` is nonempty.

#### 7. Wrong vs Correct

Wrong: asserting visualizer save output immediately after logging a frame.

Correct: call `RerunSink::flush_blocking()` before checking the `.rrd` file.

### Tamping Station Storage Contract

#### 1. Scope / Trigger

- Trigger: changes to station transforms, `.qpcd` read/write, SQLite storage,
  or `capture-store` command parsing/orchestration.
- Scope: storage and transform contracts live in the reusable SDK. The
  `capture-store` app is a thin lifecycle wrapper over SDK modules.

#### 2. Signatures

- `CoordinateTransform::transform_frame(&Frame<PointXyzir>) -> Frame<PointXyzir>`
- `YawPitchRollPose { x_m, y_m, z_m, yaw_deg, pitch_deg, roll_deg }`
- `write_qpcd(path, frame, coord_frame)`
- `SqliteStore::insert_scan_frame(&NewScanFrame)`
- `capture-store live --host <SENSOR_IP> [--record-raw]`
- `capture-store replay <INPUT.qraw>`

#### 3. Contracts

- Transform tests must prove that intensity and ring are unchanged.
- Default pose tests must cover translation plus simple yaw, pitch, and roll
  rotations. The default rotation composition is `Rz(yaw) * Ry(pitch) *
  Rx(roll)`.
- At least one test must use a custom `CoordinateTransform` implementation so
  capture/storage code cannot silently depend on the yaw/pitch/roll concrete
  type.
- Storage tests must verify the database row points to a readable `.qpcd` file.
- `capture-store` tests must prove raw recording is opt-in and the field-facing
  angle names are `yaw_deg`, `pitch_deg`, and `roll_deg` via CLI flags
  `--yaw-deg`, `--pitch-deg`, and `--roll-deg`.

#### 4. Validation & Error Matrix

- Invalid `.qpcd` magic -> explicit storage format error.
- Unsupported `.qpcd` point stride/version -> explicit storage format error.
- Missing live sensor host -> actionable config error.
- Storage queue full -> dropped-frame accounting/logging, not unbounded memory
  growth.

#### 5. Good / Base / Bad Cases

- Good: transform, qpcd, SQLite, and CLI parsing have focused unit tests.
- Base: replay storage can be validated without sensor hardware.
- Bad: a test that only asserts a row exists without reading the referenced
  `.qpcd` file.

#### 6. Tests Required

- Transform identity, translation, yaw, pitch, roll, metadata preservation, and
  custom strategy tests.
- `.qpcd` round-trip and invalid-magic tests.
- SQLite insert/read plus readable-cloud-path test.
- CLI parsing tests for `live`, `replay`, transform flags, and `--record-raw`.

#### 7. Wrong vs Correct

Wrong: test `capture-store` by checking only that Clap accepts the subcommand.

Correct: assert that parsed CLI fields map to the exact storage and transform
contracts used by the SDK.

---

## Code Review Checklist

- [ ] No `unwrap()` or `expect()` in library code.
- [ ] All new fallible functions return `Result<T>` (not `Option<T>` for
  errors).
- [ ] Errors use existing `QuanergyError` variants or add justified new ones.
- [ ] Logging uses structured `tracing` macros (not `println!`).
- [ ] Channels have explicit bounds and overflow behavior.
- [ ] File writes are two-phase (tmp → rename).
- [ ] `.qraw` recording paths produce a `.qraw.toml` sidecar.
- [ ] New public types are re-exported from `lib.rs`.
- [ ] No Tokio or async runtime introduced without explicit task approval.
- [ ] Tests exist for the new behavior (unit or integration as appropriate).
- [ ] `cargo clippy -- -D warnings` passes on the changed crate.
- [ ] `cargo fmt --check` passes.
