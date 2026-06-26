# Implementation Plan

Do not start implementation until the user approves this plan and the task is
started with `task.py start`.

## Checklist

1. Define station-coordinate contract.
   - Use the fixed lower-left site point as station origin.
   - Write the selected axis direction convention and transform config shape:
     `X` right, `Y` upward / deeper into station drawing, `Z` vertically upward,
     units in meters.
   - Verify with a unit test for identity and known translation/rotation.
   - Include pose-to-transform tests for scanner origin plus tilt/orientation
     angle inputs.
   - Use `yaw_deg`, `pitch_deg`, and `roll_deg` as the default pose config
     fields.

2. Add transform module in the SDK core.
   - Define a replaceable `CoordinateTransform` strategy boundary.
   - Implement a small rigid-transform type or use a minimal matrix helper.
   - Expose a method that builds the transform from scanner position plus
     adjustable `yaw_deg` / `pitch_deg` / `roll_deg` parameters.
   - Implement the yaw/pitch/roll pose transform as the default strategy only.
   - Apply it to `Frame<PointXyzir>` without changing intensity or ring.
   - Verify with targeted transform tests.

3. Add `.qpcd` binary frame format.
   - Define stable magic/version/header/point stride.
   - Implement writer and reader.
   - Verify round-trip with small frames and invalid-magic rejection.

4. Add metadata persistence.
   - Add `rusqlite` for local SQLite metadata persistence.
   - Create schema migration or schema initialization for capture session and
     scan frame metadata.
   - Do not add first-class work-cycle/pass tables in the first version.
   - Verify insert/read metadata and cloud-path readability.

5. Add storage orchestration app.
   - Create a thin app such as `apps/capture-store`.
   - Support live capture and replay-from-qraw modes.
   - Default live storage to transformed `.qpcd` plus SQLite metadata only.
   - Add `--record-raw` to enable synchronized `.qraw` recording and sidecar
     writing for debugging/calibration.
   - Reuse existing config/deviceInfo/pipeline construction patterns from the
     visualizer app.
   - Ensure app code depends on the transform strategy interface, not directly
     on the yaw/pitch/roll transform.
   - Verify against synthetic qraw or synthetic packet integration tests.

6. Add write safety and backpressure counters.
   - Write `.qpcd.tmp` before final rename.
   - Track frames written, bad packets, dropped/skipped frames, and storage
     errors.
   - Verify stale temp files are not recorded as complete metadata.

7. Run quality gates.
   - `rtk cargo fmt --all --check`
   - `rtk cargo test --workspace`
   - `rtk just ci`

## Risky Files

- `Cargo.toml`
- `Cargo.lock`
- `crates/quanergy-client/Cargo.toml`
- `crates/quanergy-client/src/lib.rs`
- `crates/quanergy-client/src/cloud.rs`
- New `crates/quanergy-client/src/transform/*`
- New `crates/quanergy-client/src/storage/*`
- New `apps/capture-store/*`

## Rollback Points

- If matrix/extrinsic scope becomes unclear, stop after PRD/design updates and
  do not add transform code.
- If field adjustment requires live hot-reload instead of per-run config reload,
  revise the app orchestration design before implementation.
- If transform customization requires dynamic plugins or scripting rather than
  a Rust trait/config strategy, revise `design.md` before implementation.
- If SQLite choice changes to PostgreSQL/network storage later, revise
  `design.md` before adding network database dependencies.
- If `.qpcd` format requirements expand to cross-language tooling immediately,
  reconsider whether PCD/PLY debug export is required in the same slice.

## Follow-up Checks Before Start

- Review `prd.md`, `design.md`, and `implement.md`, then start the task only
  after explicit implementation approval.
