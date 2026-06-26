# Reorganize quanergy client modules

## Goal

Reorganize `crates/quanergy-client/src` so the Rust library layout matches the
project's intended architecture boundaries while preserving behavior and keeping
the first visualizer milestone easy to maintain.

The user value is lower cognitive load when working on capture, protocol,
pipeline, calibration, replay, cloud, and future conversion/storage/measurement
work. A maintainer should be able to open the module tree and immediately find
the relevant code without reading a 900-line root module first.

## Confirmed Facts

- Project guidance says `ref/refactor_plan.md` is the current technical
  direction before planning protocol, capture, parsing, calibration,
  visualization, storage, or measurement changes.
- `ref/refactor_plan.md` recommends architecture boundaries named `net/`,
  `protocol/`, `calibration/`, `cloud/`, `convert/`, `storage/`, and `measure/`.
- Current `crates/quanergy-client/src` has directories only for `calibration/`
  and `protocol/`; root files include `cloud.rs`, `config.rs`, `error.rs`,
  `filters.rs`, `net.rs`, `pipeline.rs`, and `replay.rs`.
- Current file sizes show real maintainability pressure:
  `pipeline.rs` is 921 lines, `calibration/mod.rs` is 400 lines,
  `config.rs` is 373 lines, `protocol/mod.rs` is 230 lines, and
  `replay.rs` is 197 lines.
- `pipeline.rs` mixes public pipeline orchestration, packet parser dispatch,
  M-Series parser state, packet-type parsers for `0x00`, `0x01`, `0x04`,
  `0x06`, frame assembly helpers, and tests.
- `config.rs` mixes deviceInfo XML parsing, sensor model defaults, pipeline
  configuration, C++ XML settings parsing, and scalar parsing helpers.
- `replay.rs` mixes qraw sidecar metadata, qraw writer/reader logic, timestamp
  formatting, and tests.
- `apps/visualizer` imports `quanergy_client::{config, net, pipeline, replay}`
  and `quanergy_client::cloud::{Frame, PointXyzir}`. This means top-level public
  module paths are already consumed inside the workspace.
- There is one unrelated dirty-tree change in
  `crates/quanergy-client/Cargo.toml` changing the `toml` dependency version.
  The module reorganization must not overwrite or revert it.

## Requirements

- Keep the change focused on module/file organization. Do not change packet
  parsing behavior, calibration math, replay format, network behavior, visualizer
  behavior, or dependency versions as part of this task.
- Preserve existing public Rust API paths unless the user explicitly chooses a
  breaking cleanup. The user has chosen to preserve public paths. At minimum,
  existing workspace imports in `apps/visualizer` must continue compiling.
- Split large root modules into directory modules along existing domain
  boundaries, using small `mod.rs` files as re-export/assembly points where that
  preserves public paths.
- Keep tests close to the code they validate when splitting modules.
- Keep future out-of-scope domains (`convert/`, `storage/`, `measure/`) absent
  unless code actually moves there in this task. Do not create placeholder
  modules just to match a desired tree.
- Avoid unrelated formatting churn. File moves and import updates should account
  for nearly all changed lines.
- Preserve Windows-first milestone assumptions and the visualizer-first library
  contract.

## Acceptance Criteria

- [ ] The `quanergy-client` library compiles after the reorganization.
- [ ] Existing `apps/visualizer` imports continue to compile.
- [ ] `pipeline.rs` is no longer a 900-line root module; parser dispatch,
      M-Series parser implementation, and packet-specific parsing helpers live
      under a `pipeline/` directory or equivalent focused submodules.
- [ ] `config.rs` and/or `replay.rs` are split only where the split improves
      navigation without introducing speculative abstractions.
- [ ] `cloud`, `net`, `filters`, and `error` remain simple if evidence shows
      their current single-file shape is already appropriate.
- [ ] Existing tests for parser, replay, calibration, config, and visualizer
      behavior pass.
- [ ] No unrelated dirty-tree changes, especially the existing
      `crates/quanergy-client/Cargo.toml` edit, are reverted.

## Out of Scope

- New protocol support or parser behavior changes.
- New storage, measurement, station-frame transform, or visualization features.
- Dependency upgrades or Cargo metadata cleanup.
- Renaming the crate or changing executable behavior.
- Creating placeholder modules for planned but unimplemented domains.

## Decisions

- Preserve existing top-level public module API (`quanergy_client::pipeline`,
  `quanergy_client::config`, etc.) and reorganize implementation files behind
  those modules.
- Include `pipeline`, `config`, and `replay` in this refactor slice.
- Keep `cloud`, `net`, `filters`, and `error` as single-file modules unless a
  compile-time issue requires a minimal move.

## Open Questions

- None.

## Notes

- Keep `prd.md` focused on requirements, constraints, and acceptance criteria.
- Lightweight tasks can remain PRD-only.
- For complex tasks, add `design.md` for technical design and `implement.md` for execution planning before `task.py start`.
