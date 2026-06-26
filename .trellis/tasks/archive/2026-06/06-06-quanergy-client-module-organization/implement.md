# Implementation Plan

## Assumptions

- The user chose to preserve existing public module paths.
- This is a refactor-only task: behavior changes are defects unless required to
  fix a move-related compile/test failure.
- The existing dirty change in `crates/quanergy-client/Cargo.toml` belongs to
  another work item and must be preserved.

## Checklist

1. Establish baseline.
   - Inspect `git status --short`.
   - Run `rtk cargo test -p quanergy-client --lib` if quick enough to confirm
     the SDK unit-test baseline before moving files.
   - Status: done. Initial dirty tree only showed
     `crates/quanergy-client/Cargo.toml`.
   - Status: done. `rtk cargo test -p quanergy-client --lib` passed with 16
     tests before file moves.

2. Split `pipeline.rs`.
   - Move public orchestration into `pipeline/mod.rs`.
   - Move private dispatch into `pipeline/dispatch.rs`.
   - Move M-Series parser state and `0x00`/`0x04`/`0x06` parsing into
     `pipeline/m_series.rs`.
   - Move `0x01` parsing into `pipeline/packet_01.rs`.
   - Move shared helper functions into `pipeline/helpers.rs`.
   - Move existing pipeline tests into `pipeline/tests.rs`.
   - Status: done.
   - Verify: `rtk cargo test -p quanergy-client --lib pipeline` passed with 6
     tests.

3. Split `config.rs`.
   - Move `SensorModel` and `DeviceInfo` into `config/device_info.rs`.
   - Move `PipelineConfig` and `EncoderMode` into `config/settings.rs`.
   - Move XML flattening and parse helpers into `config/xml.rs`.
   - Re-export the existing public API from `config/mod.rs`.
   - Move config tests into `config/tests.rs`.
   - Status: done.
   - Verify: `rtk cargo test -p quanergy-client --lib config` passed with 2
     tests.

4. Split `replay.rs`.
   - Move `SidecarMetadata` into `replay/sidecar.rs`.
   - Move `QrawReader`, `QrawWriter`, and qraw constants into `replay/qraw.rs`.
   - Keep `current_time_string` public through `replay::current_time_string`.
   - Move replay tests into `replay/tests.rs`.
   - Status: done.
   - Verify: `rtk cargo test -p quanergy-client --lib replay` passed with 1
     test.

5. Review small root modules.
   - Confirm `cloud.rs`, `net.rs`, `filters.rs`, and `error.rs` remain simple
     enough to stay as single files.
   - Do not split them unless a compile-time dependency cycle from previous
     steps forces a minimal adjustment.
   - Status: done. `cloud.rs`, `net.rs`, `filters.rs`, and `error.rs` remain
     single-file modules.
   - Verify: no placeholder `convert/`, `storage/`, or `measure/` modules were
     added.

6. Run full quality gate.
   - `rtk cargo fmt --all -- --check`
   - `rtk cargo clippy --all-targets --all-features -- -D warnings`
   - `rtk cargo test --all-targets --all-features`
   - `rtk just ci`
   - Status: done. All four commands passed after the module split.
   - Note: `just ci` emitted a Cargo warning for the pre-existing
     `toml = "1.1.2+spec-1.1.0"` dependency version. Cargo generated a
     `Cargo.lock` update during verification; it was restored because
     dependency/lockfile changes are out of this task's scope.
   - Note: after restoring `Cargo.lock`, `rtk cargo test -p quanergy-client
     --lib --locked` fails because the pre-existing `Cargo.toml` edit requires
     a lockfile update. This is not caused by the module reorganization.

7. Review diff.
   - Confirm file moves and import/module edits dominate the diff.
   - Confirm no changes to parser logic, replay format, visualizer behavior, or
     dependency versions.
   - Status: done. The pre-existing `Cargo.toml` change remains preserved and
     unrelated; `Cargo.lock` is not modified by this task.

## Risky Files

- `crates/quanergy-client/src/pipeline.rs`
- `crates/quanergy-client/src/config.rs`
- `crates/quanergy-client/src/replay.rs`
- `crates/quanergy-client/src/lib.rs`
- `apps/visualizer/src/lib.rs`
- `apps/visualizer/tests/replay_visualizer.rs`

## Rollback Points

- After `pipeline` split and focused tests pass.
- After `config` split and focused tests pass.
- After `replay` split and focused tests pass.
- Before running `just ci`, inspect the diff for accidental behavior edits.

## Completion Criteria

- PRD acceptance criteria are satisfied.
- Full quality gate passes or any failure is clearly unrelated and documented
  with evidence.
- Task artifacts reflect the final implemented shape if implementation deviates
  from the planned split for a concrete Rust module/privacy reason.
