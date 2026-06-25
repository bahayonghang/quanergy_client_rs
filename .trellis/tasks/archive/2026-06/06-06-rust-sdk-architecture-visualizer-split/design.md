# Architecture Reorganization Design

## Current Architecture

The current repository is a single Cargo package with:

- `src/lib.rs` as the SDK library target.
- `src/main.rs` as the `quanergy-client` binary target.
- `src/visualizer.rs` as a public SDK module using Rerun.
- `tests/replay_visualizer.rs` as an integration smoke test that imports `quanergy_client::visualizer`.

This shape works for a first runnable rewrite, but it makes the SDK carry application-only dependencies and exposes visualizer concepts as if they were part of the reusable library contract.

The C++ reference uses a different target model:

- `quanergy_client` is a shared library target.
- `visualizer` is an executable target that links the library and owns visualization.
- `dynamic_connection` is an executable target that links the library and owns the interactive connection demo.
- There is no `quanergy_client.exe` target in the reference build.

## Desired Boundaries

Core SDK boundary:

- Owns wire protocol, raw packet representation, TCP packet source, deviceInfo fetch, C++ settings compatibility, calibration, filters, cloud data types, replay qraw format, and `SensorPipeline`.
- Has no dependency on Rerun.
- Has no public `visualizer` module.
- Remains usable by CLI, visualizer, storage, and future measurement tools.

Application boundary:

- Owns Clap command parsing, no-host user feedback, Rerun sink setup, live/replay visualizer loops, and record command orchestration.
- May depend on `rerun`, `clap`, and `tracing-subscriber`.
- Uses the selected `visualizer` executable contract instead of the old consolidated Rust binary contract.

## Split Options

### Option A: Single Package With Feature-Gated App Code

Keep one Cargo package. Move visualizer implementation out of the library module tree into application-owned modules such as `src/app/visualizer.rs` or `src/bin_support/visualizer.rs`. Make `rerun` optional and enable it only for the binary/test feature.

Pros:

- Smaller diff and lower migration risk.
- Keeps current package name and binary target simple.
- Good enough to remove `pub mod visualizer` from the SDK API.

Cons:

- Package-level dependencies and features remain entangled.
- A single package still mixes SDK and app ownership in one manifest.
- Future storage/measurement app crates may repeat this split work.

### Option B: Cargo Workspace With Separate SDK And App Packages

Convert the repo into a small workspace, for example:

```text
Cargo.toml
crates/
  quanergy-client/        # SDK library crate, package can keep or adjust current name
    src/
      lib.rs
      calibration/
      cloud/
      config/
      error.rs
      filters.rs
      net/
      pipeline/
      protocol/
      replay/
apps/
  visualizer/             # visualizer app package preserving original app name
    src/
      main.rs
      cli.rs
      rerun_sink.rs
```

Pros:

- Cleanest Rust ownership boundary between reusable SDK and applications.
- Rerun and Clap stay out of the SDK package dependency graph.
- Future storage/measurement tools can depend on the SDK crate without app dependencies.
- Better matches the project direction: reusable library API plus thin application wrappers.

Cons:

- Larger diff: file moves, manifest changes, test target moves, possible package-name compatibility decisions.
- More care needed to revise existing consolidated-binary tests.
- Cargo paths and docs need updates.

## Recommended Direction

Selected: Option B, Cargo workspace with separate SDK and app packages.

This most directly satisfies "split visualizer app from SDK" as an architectural boundary, not just a file move.

Naming correction from user: preserve the original-style split as one core `quanergy-client` side and one `visualizer` application side. Do not introduce a `quanergy-cli` package name. Do not keep a `quanergy-client.exe` compatibility shim for visualizer startup.

Recommended concrete layout:

```text
Cargo.toml                  # workspace root
crates/quanergy-client/     # package name: quanergy-client, lib crate: quanergy_client
apps/visualizer/            # package name: visualizer, bin name: visualizer
```

The SDK package keeps reusable Quanergy client functionality. The visualizer package owns Rerun and visualizer live/replay app behavior.

Current Rust-only consolidation points must be resolved explicitly:

- Current `visualizer live` and `visualizer replay` should move into `apps/visualizer`.
- Current `dynamic-connection` resembles original `dynamic_connection.exe`, but the user chose not to split or restore it in this visualizer-focused task.
- Current `record` is a Rust-specific raw qraw recorder, not a C++ reference app target. The user chose to keep it under the visualizer app as `visualizer record --host ... <OUTPUT>` because it prepares `.qraw` inputs for `visualizer replay`.

Option A remains documented only as the rejected lower-risk alternative.

## Module Reorganization Within The SDK

Regardless of split option, reorganize large modules only where it reduces real ownership mixing:

- `protocol/`
  - Keep packet header, constants, raw packet, return selection, LUT, and packet type constants.
  - Parser implementation can move under `protocol/parser/` or `pipeline/parser/` only if this makes `pipeline.rs` meaningfully smaller without changing behavior.
- `pipeline/`
  - Own `SensorPipeline`, counters, parser dispatch, and frame-level processing flow.
  - Packet-specific parsers may be split into private files such as `pipeline/parser_m_series.rs`, `pipeline/packet_01.rs`, or `protocol/packet_04.rs`.
- `cloud/`
  - Keep `Frame`, `PointHvdir`, `PointXyzir`, and conversion methods.
- `calibration/`
  - Keep encoder correction and automatic calibration.
- `net/`
  - Keep blocking TCP packet source and deviceInfo HTTP fetch.
- `replay/`
  - Keep qraw reader/writer and sidecar metadata.

Do not add empty `storage/`, `measure/`, or station-transform modules in this task.

## Compatibility Notes

- `quanergy_client::visualizer` is the only intended public API break.
- Existing core imports such as `quanergy_client::pipeline::SensorPipeline` and `quanergy_client::replay::QrawReader` should keep working.
- Existing C++ migration aliases `PointHVDIR` and `PointXYZIR` should remain.
- The backend quality spec currently assumes one Rust binary where bare `quanergy-client.exe` resolves to `visualizer live`. The user selected no compatibility shim, so that spec section and its tests must be revised to the new executable contract.

## Validation Strategy

- Run `cargo fmt --all -- --check`.
- Run `cargo clippy --all-targets --all-features -- -D warnings`.
- Run `cargo test --all-targets --all-features`.
- Run `just ci`.
- For dependency boundary verification, run a narrow core-library check without visualizer app features or against the SDK package only, depending on the chosen split.

## Open Design Decision

All required planning decisions are resolved:

- Workspace/package split.
- Core `quanergy-client` / `quanergy_client` SDK side.
- Separate `visualizer` app.
- No `quanergy-client.exe` visualizer compatibility shim.
- No `dynamic_connection` app in this task.
- Keep qraw recording as `visualizer record`.
