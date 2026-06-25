# Logging Guidelines

> How logging is done in this project.

---

## Overview

This project uses the **`tracing`** crate for structured, async-aware logging.
The `tracing` facade is declared as a dependency in
`crates/quanergy-client/Cargo.toml`. Library code imports only the macros it
needs (e.g. `use tracing::warn;`). Apps wire a subscriber (e.g.
`tracing_subscriber::fmt`) at startup.

Do **not** use the `log` crate directly. `tracing` is the single logging
frontend.

---

## Log Levels

| Level | When to use | Example |
|---|---|---|
| `error!` | Fatal conditions that terminate a task or operation | `error!(%error, "ingestion loop error")` |
| `warn!` | Recoverable issues, degraded operation, missing optional data | `warn!(%error, "deviceInfo unavailable; continuing with configured/default calibration")` |
| `info!` | Operational milestones, session start/end, frame counts | `info!(%host, record_raw, "starting live station-frame capture")` |
| `debug!` | Packet-level diagnostics, per-packet metadata | `debug!(packet_type, size, "received packet")` |
| `trace!` | Per-point or per-firing detail — reserved for deep debugging | Not used in production paths |

### Decision Rule

- Can the system continue without data loss? → `warn!`
- Is the system unable to proceed? → `error!`
- Is this a normal lifecycle event the operator should see? → `info!`
- Is this only useful during development? → `debug!`

---

## Structured Logging

Always use structured key-value pairs, not string interpolation:

```rust
// ✅ Correct — structured fields, %error uses Display
warn!(%error, "dropping bad packet");
info!(%host, record_raw = args.record_raw, "starting live station-frame capture");
error!(%error, frames_written, "failed to persist station-frame cloud");
warn!(dropped_frames, "raw queue full, dropping raw packet");

// ❌ Wrong — string interpolation hides fields from subscribers
warn!("dropping bad packet: {error}");
info!("starting live station-frame capture on {host}");
```

### Field Naming

- Use `snake_case` identifiers: `dropped_frames`, `frames_written`, `record_raw`.
- Prefer `%variable` (Display) for error and string-like values.
- Use `variable = value` syntax for booleans and integers.
- Keep field names short but descriptive — they appear as labels in structured
  log output.

---

## What to Log

- Packet ingestion errors (corrupt headers, size mismatches)
- Calibration fetch failures and fallback decisions
- Frame emission counts at session boundaries
- Queue-full events with dropped-frame counts
- Storage failures that lose data
- Session start/end with relevant parameters (host, mode flags)

---

## What NOT to Log

- **Raw packet bytes** — too large, not human-readable. Save to `.qraw` if
  needed.
- **Per-point coordinates** — use `debug!` sparingly and only for small test
  fixtures.
- **Secrets, API keys, or credentials** — never log these at any level.
- **PII** — this is a sensor data pipeline; there is no PII. If user-provided
  strings (session notes, hostnames) are logged, keep them at `info!` and below.

---

## Subscriber Setup (Apps)

Each binary app is responsible for initializing a `tracing_subscriber`. The
typical pattern:

```rust
use tracing_subscriber::fmt;

fn main() {
    fmt::init();  // or fmt().with_env_filter(...).init()
    // ...
}
```

Library code (`crates/quanergy-client/`) must **not** call `tracing_subscriber::init()`.
It only imports and uses the macros.

---

## Common Mistakes

- **Do not** use `println!` or `eprintln!` for operational logging. Use the
  appropriate tracing macro.
- **Do not** call `tracing_subscriber::init()` in library code — only in binary
  `main()`.
- **Do not** log and return the same error from a library function. Logging is
  the caller's decision. The lenient-mode `warn!(%error, "dropping bad packet")`
  in `SensorPipeline::process_packet_bytes` is the one intentional exception.
- **Do not** use `format!()` inside a tracing macro. Use structured fields
  instead.
