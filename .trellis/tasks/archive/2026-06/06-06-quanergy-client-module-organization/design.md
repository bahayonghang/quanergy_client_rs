# Design

## Scope

This task is a source organization refactor for `crates/quanergy-client/src`.
It must preserve the existing public module API:

- `quanergy_client::pipeline::{SensorPipeline, PipelineCounters}`
- `quanergy_client::config::{PipelineConfig, DeviceInfo, EncoderMode, SensorModel, flatten_xml}`
- `quanergy_client::replay::{QrawReader, QrawWriter, SidecarMetadata, current_time_string}`
- existing top-level modules declared by `src/lib.rs`

No parser behavior, calibration math, replay format, network behavior, or
visualizer behavior should change.

## Current Shape

The root `src` directory currently contains several modules as single files:

```text
src/
  cloud.rs
  config.rs
  error.rs
  filters.rs
  net.rs
  pipeline.rs
  replay.rs
  calibration/mod.rs
  protocol/mod.rs
```

The main maintainability issue is not every root file. It is that several root
files mix multiple responsibilities:

- `pipeline.rs` is 921 lines and contains public pipeline orchestration,
  parser dispatch, M-Series parser state, packet type parsers, frame assembly
  helpers, and tests.
- `config.rs` is 373 lines and contains sensor model parsing, deviceInfo XML
  parsing, pipeline settings, XML flattening, scalar parsers, and tests.
- `replay.rs` is 197 lines and contains sidecar metadata plus qraw binary
  reader/writer logic.

The small modules `cloud.rs`, `net.rs`, `filters.rs`, and `error.rs` are not
large enough to justify speculative splitting in this task.

## Target Shape

Use directory modules only where they reduce real navigation cost while keeping
public paths stable.

```text
src/
  lib.rs
  cloud.rs
  error.rs
  filters.rs
  net.rs
  calibration/
    mod.rs
  protocol/
    mod.rs
  pipeline/
    mod.rs
    dispatch.rs
    m_series.rs
    packet_01.rs
    helpers.rs
    tests.rs
  config/
    mod.rs
    device_info.rs
    settings.rs
    xml.rs
    tests.rs
  replay/
    mod.rs
    sidecar.rs
    qraw.rs
    tests.rs
```

This is the preferred split for implementation. If Rust privacy or borrow
boundaries make a submodule split awkward, prefer a slightly coarser split over
adding public surface or complex abstraction.

## Module Boundaries

### pipeline

`pipeline/mod.rs` owns the public API and orchestration:

- `PipelineCounters`
- `SensorPipeline`
- imports and `pub`/private module assembly

`pipeline/dispatch.rs` owns private parser dispatch:

- `ParserDispatch`
- packet type routing

`pipeline/m_series.rs` owns stateful M-Series frame assembly and packet parsers
for `0x00`, `0x04`, and `0x06`:

- `MSeriesParser`
- `MSeriesFiring`
- `register_packet`, `check_complete`, `add_firing`

`pipeline/packet_01.rs` owns the stateless HVDIR-list parser:

- `parse_01`
- `ring_for_vertical_angle`

`pipeline/helpers.rs` owns parser helper functions shared by M-Series and
packet `0x01`:

- packet length/status validation
- distance-to-point conversion
- all-return expansion
- cloud organization

`pipeline/tests.rs` keeps the existing pipeline parser tests close to the
pipeline module after the split.

### config

`config/mod.rs` preserves the public API by re-exporting:

- `SensorModel`
- `DeviceInfo`
- `EncoderMode`
- `PipelineConfig`
- `flatten_xml`

`config/device_info.rs` owns `SensorModel` and `DeviceInfo`.

`config/settings.rs` owns `EncoderMode`, `PipelineConfig`, and C++ settings XML
application logic.

`config/xml.rs` owns `flatten_xml` and scalar XML parsing helpers. Helpers can
remain `pub(super)` unless already public.

### replay

`replay/mod.rs` preserves the public API by re-exporting:

- `SidecarMetadata`
- `QrawWriter`
- `QrawReader`
- `current_time_string`

`replay/sidecar.rs` owns sidecar metadata construction, loading, and saving.

`replay/qraw.rs` owns qraw binary reader/writer types and record format
constants.

`replay/tests.rs` keeps replay format tests close to the replay module.

## Compatibility

Top-level module declarations in `lib.rs` should remain:

```rust
pub mod calibration;
pub mod cloud;
pub mod config;
pub mod error;
pub mod filters;
pub mod net;
pub mod pipeline;
pub mod protocol;
pub mod replay;
```

Existing imports in `apps/visualizer` must not need semantic changes. Import
formatting changes from `cargo fmt` are acceptable.

## Trade-offs

Preserving public module paths means this is not a public API redesign. The
benefit is a low-risk internal reorganization that can be verified by compile
and tests.

Splitting `protocol/mod.rs` and `calibration/mod.rs` is tempting, but they are
already inside domain directories and are less urgent than `pipeline.rs`. Leave
them for a later targeted task unless implementation shows a small helper move
is necessary.

Creating empty `convert/`, `storage/`, or `measure/` modules would make the tree
look closer to the long-term plan, but it would add placeholder surface without
current code. Do not create them in this task.

## Rollback

This task should consist mostly of file moves and import/module edits. If a
regression appears, rollback can be done by restoring the original single-file
modules for the affected domain. The existing unrelated `Cargo.toml` dependency
edit must not be reverted.
