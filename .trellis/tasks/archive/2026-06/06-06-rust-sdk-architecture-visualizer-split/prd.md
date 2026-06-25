# Reorganize Rust SDK Architecture And Split Visualizer App

## Goal

Reorganize the current Rust Quanergy rewrite so the SDK library exposes reusable capture, protocol, calibration, cloud, replay, filtering, and pipeline APIs, while the visualizer remains an application concern rather than a public SDK module or mandatory SDK dependency.

The outcome should make the first milestone easier to extend toward CLI, storage, and measurement tools without keeping Rerun visualization code inside the core library surface.

## Confirmed Facts

- The repository is a Rust functional rewrite of the local C++ SDK reference under `ref/quanergy_client`.
- First milestone scope is original SDK functionality: capture, replay, packet parsing, pipeline processing, encoder calibration, filters, HVDIR-to-XYZIR conversion, real-time visualization, dynamic connection behavior, and C++ CLI/XML settings compatibility.
- Follow-up business work, including station transforms, storage formats, ROI processing, and tamping-hammer height measurement, is not part of the first original-SDK rewrite milestone.
- The project guidance requires a reusable Rust library API; the CLI should be a thin wrapper over library modules rather than the owner of protocol, pipeline, calibration, or visualization logic.
- Current Cargo metadata shows one package, one library target `quanergy_client` at `src/lib.rs`, one binary target `quanergy-client` at `src/main.rs`, and one integration test target `tests/replay_visualizer.rs`.
- Current `src/lib.rs` publicly exports `pub mod visualizer`, so Rerun visualization is part of the SDK public module tree.
- Current `Cargo.toml` makes both `clap` and `rerun` non-optional package dependencies. `cargo tree -i rerun` confirms `rerun v0.33.0` is pulled directly by `quanergy-client`.
- `src/visualizer.rs` is the only implementation module using Rerun types. `src/main.rs` and `tests/replay_visualizer.rs` use that public visualizer module.
- `src/main.rs` currently combines CLI parsing, default `visualizer live` launch behavior, config construction, deviceInfo enrichment, record/replay/dynamic-connection orchestration, sidecar writing, and Rerun sink setup.
- `src/pipeline.rs` currently combines parser dispatch, packet type `0x00`, `0x01`, `0x04`, and `0x06` parsing, M-Series frame assembly, return selection, distance/ring filtering, encoder correction dispatch, and HVDIR-to-XYZIR output.
- `src/config.rs`, `src/net.rs`, `src/replay.rs`, `src/calibration/mod.rs`, `src/cloud.rs`, `src/protocol/mod.rs`, `src/filters.rs`, and `src/error.rs` are SDK-core concerns and do not require Rerun UI code.
- The backend quality spec requires preserving the CLI default visualizer contract: bare `quanergy-client.exe` resolves to `visualizer live`, root metadata flags remain root-level, no default sensor host is invented, and the existing command tests cover these cases.
- The backend quality spec requires replay visualizer smoke coverage when packet parsing, replay, pipeline, or Rerun visualization cross library/CLI boundaries change.
- The C++ reference CMake builds `quanergy_client` as a shared library target with Windows outputs such as `quanergy_client.dll`, `.lib`, and `.exp`.
- The C++ reference CMake builds `visualizer` as a separate executable that links `quanergy_client` and owns PCL visualization.
- The C++ reference CMake builds `dynamic_connection` as a separate executable that links `quanergy_client` and demonstrates start/stop/reconnect behavior.
- The C++ reference build does not define a `quanergy_client.exe` / `quanergy-client.exe` executable. The current Rust single binary is a Rust-specific consolidation, not original SDK naming.
- The user chose not to keep a `quanergy-client.exe` compatibility shim for default visualizer behavior. Visualizer behavior should move to `visualizer`.
- The user chose not to split or restore the `dynamic_connection` application in this task. This task should keep only the visualizer application split.
- The user chose to keep the Rust-only qraw recording workflow inside the visualizer app as `visualizer record --host ... <OUTPUT>`, because recording qraw captures supports `visualizer replay` offline verification.

## Requirements

- Use a Cargo workspace/package split, not only a single-package feature split.
- Preserve original-style naming: keep the core SDK side as `quanergy-client` / `quanergy_client`, and split the visualizer functionality into a separately named `visualizer` application.
- Remove visualizer implementation from the SDK public API surface.
- Ensure the core SDK can build and test without pulling Rerun as a required dependency.
- Preserve the existing public SDK concepts that are part of the first milestone: `Frame`, `PointHvdir`, `PointXyzir`, `QuanergyError`, `Result`, protocol parsing, packet source, replay, calibration, filters, config, and `SensorPipeline`.
- Preserve the core SDK side as `quanergy-client` / `quanergy_client`; move visualizer live/replay behavior to the separate `visualizer` application.
- Keep qraw recording as part of the visualizer workflow: `visualizer record --host ... <OUTPUT>`.
- Do not create a `dynamic_connection` application in this task.
- Replace the existing Rust CLI default-visualizer compatibility tests with tests that match the selected original-style executable split.
- Keep architectural changes surgical: move ownership boundaries and module layout without changing packet parser behavior, calibration math, frame assembly semantics, or raw replay format.
- Reorganize large mixed-responsibility modules only where the move directly supports the requested architecture split.
- Keep first-milestone scope focused on original SDK parity. Do not implement station transforms, storage formats, ROI processing, or tamping-hammer measurement as part of this task.
- Prefer standard Rust layout where application entry points live outside the library core and feature-gated or separate package dependencies keep application-only crates out of SDK builds.

## Acceptance Criteria

- [x] `quanergy_client` core library no longer exposes `pub mod visualizer`.
- [x] The core library can be built/tested without Rerun enabled or required.
- [x] Visualizer-specific code lives under a separately named `visualizer` application path and can run live/replay behavior equivalent to the current visualizer CLI path.
- [x] `visualizer record --host ... <OUTPUT>` preserves current `.qraw` recording and `.qraw.toml` sidecar behavior.
- [x] There is no `quanergy-client.exe` default-to-visualizer compatibility shim.
- [x] Command parsing tests are updated to cover the selected executable contract for `visualizer` and any retained non-visualizer executable.
- [x] Replay visualizer smoke coverage still writes `.qraw`, replays through `SensorPipeline`, saves `.rrd`, flushes, and asserts nonempty output.
- [x] Existing parser tests for packet types `0x00`, `0x01`, `0x04`, and `0x06` still pass.
- [x] Strict/lenient bad packet behavior still passes.
- [x] `just ci` passes after the reorganization.
- [x] The final diff does not add storage, measurement, station transform, or speculative placeholder modules outside the requested split.

## Out Of Scope

- Adding new packet formats beyond current `0x00`, `0x01`, `0x04`, and `0x06` coverage.
- Changing the qraw file format or qraw sidecar schema.
- Implementing station-frame transforms, `.qpcd`/PCD storage, database metadata, ROI processing, or tamping-hammer height measurement.
- Replacing Rerun with another visualizer.
- Reworking runtime concurrency unless the chosen split exposes a concrete need.
- Changing non-visualizer SDK behavior beyond what is required to split out the `visualizer` application.
- Restoring or splitting the original C++ `dynamic_connection` application.

## Open Questions

- None.
