# Quanergy Rust Rewrite Implementation Plan

## Milestones

1. Project skeleton and public API
   - Create Cargo library + CLI binary.
   - Add modules for errors, cloud types, config, protocol, replay, net,
     calibration, filters, pipeline, and visualizer.
   - Validate with `cargo check`.

2. Protocol and fixtures
   - Implement packet header read/write and `.qraw` v1 reader/writer.
   - Implement sidecar metadata load/save.
   - Add generated deterministic fixtures and small replay smoke data.
   - Validate header, endian, invalid signature, and replay timing tests.

3. Parser and pipeline
   - Implement parser dispatch for `0x00`, `0x01`, `0x04`, and `0x06`.
   - Implement M-Series LUT, vertical angle handling, return selection, frame
     assembly, and cloud size limits.
   - Implement filters and HVDIR-to-XYZIR conversion.
   - Validate point counts, units, rings, NaNs, frame completion, filters, and
     conversion tests.

4. Calibration and settings compatibility
   - Implement deviceInfo fetch/parse.
   - Implement C++ XML settings compatibility and CLI config merge.
   - Implement manual/deviceInfo/disabled correction and automatic calibration
     core loop.
   - Validate XML sample loading, sidecar precedence, and calibration synthetic
     signal tests.

5. CLI, live/replay, and Rerun visualizer
   - Implement `visualizer live`, `visualizer replay`, `record`, and
     `dynamic-connection` commands.
   - Add Rerun spawn/connect/save modes and display-only downsampling.
   - Validate replay smoke via `.rrd` save and nonempty frames.

6. Hardening
   - Add `tracing` diagnostics, counters, `--strict`, and `--verbose`.
   - Add docs for Rust API and C++ concept mapping.
   - Run full quality gate.

## Validation Commands

```powershell
rtk cargo fmt --check
rtk cargo clippy --all-targets -- -D warnings
rtk cargo test
rtk cargo test replay
```

Replay visualizer smoke should save an `.rrd` under `target/` and must not
require opening a GUI.

## Risk And Rollback Points

- Rerun dependency may be heavy. Keep visualization behind a `VisualizerSink`
  boundary so parser/pipeline tests can run without a GUI.
- Parser coverage is broad. Keep each parser independently testable with small
  generated fixtures.
- Automatic calibration is complex. Land calculation/apply tests before wiring
  live calibration behavior.
- `.qraw` format is new. Keep versioned magic and tests so later PCAP import can
  be added without breaking v1.

## Pre-Start Review Gate

- `prd.md`, `design.md`, and `implement.md` exist.
- User approved the plan and requested implementation.
- Task may be started with `task.py start`.
