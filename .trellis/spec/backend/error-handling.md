# Error Handling

> How errors are defined, propagated, and handled in this project.

---

## Overview

The crate defines a single `QuanergyError` enum in `crates/quanergy-client/src/error.rs`
using `thiserror`. Every fallible public API returns `crate::error::Result<T>`,
which is an alias for `std::result::Result<T, QuanergyError>`. Library code
**must not** call `unwrap()` or `expect()` — propagate errors with `?` instead.

---

## Error Types

### Central Error Enum

All errors flow through `QuanergyError` (see `crates/quanergy-client/src/error.rs`).
The enum uses `thiserror::Error` derive with `#[error("...")]` format strings:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum QuanergyError {
    #[error("invalid packet signature 0x{0:08x}")]
    InvalidSignature(u32),

    #[error("packet is too short: got {actual} bytes, need at least {minimum}")]
    PacketTooShort { actual: usize, minimum: usize },

    #[error("unsupported packet type 0x{0:02x}")]
    UnsupportedPacketType(u8),

    #[error("calibration failed: {0}")]
    Calibration(String),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("XML error: {0}")]
    Xml(#[from] quick_xml::Error),

    // ... additional variants for JSON, TOML, SQLite, HTTP, etc.
}
```

### Error Categories

| Category | Variant | When used |
|---|---|---|
| Protocol | `InvalidSignature`, `PacketTooShort`, `PacketSizeMismatch`, `UnsupportedPacketType`, `UnsupportedPacketVersion` | Packet header validation, type/version dispatch |
| Calibration | `Calibration` | Encoder calibration failures, invalid angle data |
| Configuration | `Config`, `InvalidReturnSelection`, `InvalidVerticalAngles`, `InvalidSensorStatus` | CLI/config validation, sensor status checks |
| Pipeline | `ReturnIdMismatch` | Return-selection filtering |
| Storage/Replay | `ReplayFormat`, `StorageFormat` | `.qraw` / `.qpcd` format errors, SQLite schema violations |
| Transform | `Transform` | Coordinate transform errors |
| External | `Io`, `Xml`, `TomlDe`, `TomlSer`, `Json`, `Sqlite`, `Http` | Transparent wrappers over dependency errors |

---

## Error Propagation

### Transparent Wrapping with `#[from]`

Dependency errors that the caller shouldn't need to distinguish are wrapped
with `#[from]` so `?` works directly:

```rust
// In net.rs — io::Error auto-converts to QuanergyError::Io
let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5))?;
```

Only add `#[from]` for errors where the *source type* is the meaningful
distinction. If a dependency can fail in multiple semantically different ways,
use an explicit `.map_err()` instead:

```rust
// In net.rs — ureq errors are mapped explicitly because the Http variant
// carries a String, not the ureq::Error type
let response = ureq::get(&url)
    .call()
    .map_err(|error| QuanergyError::Http(error.to_string()))?;
```

### Result Type Alias

```rust
pub type Result<T> = std::result::Result<T, QuanergyError>;
```

Every public function that can fail returns `crate::error::Result<T>`. Do not
import or use `std::result::Result` directly in public signatures.

---

## Strict vs Lenient Error Handling

The pipeline (`crates/quanergy-client/src/pipeline/mod.rs`) supports two modes
controlled by `PipelineConfig::strict`:

- **Strict mode** (`strict = true`): A single bad packet returns an error
  immediately. Used in tests and offline validation.
- **Lenient mode** (`strict = false`, default for live capture): Bad packets are
  counted in `PipelineCounters::bad_packets`, logged with `warn!`, and dropped.
  The pipeline continues processing subsequent packets.

```rust
pub fn process_packet_bytes(&mut self, packet: &[u8]) -> Result<Vec<Frame<PointXyzir>>> {
    self.counters.packets_seen += 1;
    let frames = match self.parser.parse(packet) {
        Ok(frames) => frames,
        Err(error) if self.config.strict => return Err(error),
        Err(error) => {
            self.counters.bad_packets += 1;
            warn!(%error, "dropping bad packet");
            return Ok(Vec::new());
        }
    };
    // ...
}
```

Apps that process streams should use lenient mode. CLI tools and tests that
operate on single inputs should use strict mode.

---

## Naming Conventions

- Error variant names are PascalCase nouns: `InvalidSignature`, `PacketTooShort`.
- Use field names that distinguish the error context: `{ expected, actual }`,
  `{ requested, actual }`, `{ major, minor, patch }`.
- Format strings include the most diagnostic value first. Use hex for binary
  fields (`0x{0:02x}`) and display for human-readable fields (`{0}`).

---

## Common Mistakes

- **Do not** call `unwrap()` or `expect()` in library code. Propagate with `?`
  or handle with `match` / `if let`.
- **Do not** define per-module error enums. All errors go through
  `QuanergyError`. Add a new variant instead.
- **Do not** log *and* return the same error from library code — the caller
  decides whether to log. The sole exception is the lenient-mode warn-and-drop
  pattern in `SensorPipeline`.
- **Do not** use `Box<dyn Error>` or `.into()` to erase error types. The
  `QuanergyError` enum preserves type information for the caller.
