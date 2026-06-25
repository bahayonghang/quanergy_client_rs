# Implementation Plan

## Checklist

1. Decide split granularity.
   - Status: done. Selected Cargo workspace/package split.
   - Status: done. Selected original-style naming: core `quanergy-client`, app `visualizer`.
   - Status: done. Selected no `quanergy-client.exe` visualizer compatibility shim.
   - Status: done. Selected no `dynamic_connection` app split in this task.
   - Status: done. Selected `visualizer record --host ... <OUTPUT>` for qraw recording.
   - Verify: no open planning decisions remain.

2. Prepare Cargo layout.
   - Status: done. Created SDK and app package manifests.
   - Status: done. Kept SDK package/lib side as `quanergy-client` / `quanergy_client`.
   - Status: done. Created separate `visualizer` package and binary for visualizer functionality.
   - Status: done. Moved package-level dependencies to the owning package.
   - Verify: `rtk cargo metadata --no-deps --format-version 1` shows intended targets.

3. Move visualizer implementation out of the SDK library surface.
   - Status: done. Removed `pub mod visualizer` from the SDK `lib.rs`.
   - Status: done. Moved Rerun sink and visualizer config into the app-owned module/package.
   - Verify: `rtk rg -n "pub mod visualizer|quanergy_client::visualizer|QuanergyError::Visualizer|rerun|clap" Cargo.toml crates apps .trellis/spec/backend/quality-guidelines.md` shows no SDK-core visualizer/Rerun/Clap exposure.

4. Move or adapt app orchestration.
   - Status: done. Moved visualizer live/replay CLI parsing and Rerun orchestration under the `visualizer` app boundary.
   - Status: done. Moved qraw recording orchestration under `visualizer record`.
   - Status: done. Removed the current default-to-visualizer `quanergy-client.exe` behavior and revised tests/specs accordingly.
   - Status: done. Did not create a `dynamic_connection` app in this task.
   - Status: done. Shared SDK calls go through public library APIs.
   - Verify: command parsing tests cover the selected `visualizer` executable contract.

5. Split oversized SDK modules only as needed.
   - Status: done. No SDK parser/pipeline module split was needed for this visualizer boundary move.
   - Status: done. Avoided behavior changes in parser scaling, frame assembly, filtering, and calibration.
   - Verify: existing parser and calibration unit tests pass without assertion changes except import paths.

6. Update integration tests.
   - Status: done. Moved replay visualizer smoke test to the app package.
   - Status: done. Kept qraw replay through `SensorPipeline`, `.rrd` save, `flush_blocking`, and nonempty assertion.
   - Verify: test target runs under `rtk cargo test --all-targets --all-features`.

7. Run quality gates.
   - Status: done. `rtk cargo fmt --all -- --check`
   - Status: done. `rtk cargo check --all-targets`
   - Status: done. `rtk cargo clippy --all-targets --all-features -- -D warnings`
   - Status: done. `rtk cargo test --all-targets --all-features`
   - Status: done. `rtk just ci`

## Verification Results

- `rtk cargo metadata --no-deps --format-version 1`: workspace has SDK package `quanergy-client` with lib target only, plus app package `visualizer` with lib/bin/test targets.
- `rtk cargo tree -p visualizer -i rerun`: `rerun` is owned by `visualizer`.
- `rtk cargo tree -p quanergy-client -i rerun`: `rerun` is absent from the SDK dependency graph.
- `rtk cargo tree -p quanergy-client -i clap`: `clap` is absent from the SDK dependency graph.
- `rtk cargo run -p visualizer -- --help`: root metadata works.
- `rtk cargo run -p visualizer -- dynamic-connection`: rejected as an unrecognized subcommand.
- `rtk cargo run -p visualizer -- visualizer live`: rejected as an unrecognized subcommand.

## Risky Files

- `Cargo.toml`
- `Cargo.lock`
- `src/lib.rs`
- `src/main.rs`
- `src/visualizer.rs`
- `src/pipeline.rs`
- `tests/replay_visualizer.rs`

## Rollback Points

- After Cargo layout changes but before source moves.
- After removing `pub mod visualizer` but before module splits.
- After test migration but before optional deeper `pipeline.rs` file splitting.

## Not Planned

- No new storage, measurement, or station transform modules.
- No changes to qraw format.
- No changes to packet parser algorithms unless tests expose a move-related regression.
- No CLI behavior changes unless explicitly approved.
