# Tamping Station Point Cloud Storage Planning

## Goal

Plan the first tamping-station data layer after the visualizer-first SDK
rewrite: read Quanergy point-cloud frames from the existing Rust SDK pipeline,
transform them from the sensor coordinate frame into the tamping-station
coordinate frame using configured scanner installation extrinsics, and persist
full-resolution frame data plus searchable metadata for a second program to
consume.

The immediate user value is a reliable capture/storage foundation for later
red-box tamping-hammer-space segmentation and per-group top-height calculation.
Segmentation and height calculation are intentionally not required for this
task unless the user expands the scope.

## Confirmed Facts

- The current repository is a Rust rewrite of the Quanergy C++ SDK reference,
  with first-milestone priority on the `visualizer` path.
- Project guidance says station-frame transforms, storage formats, ROI
  processing, and tamping-hammer height measurement are follow-up business work
  after original SDK parity.
- `ref/refactor_plan.md` already identifies this route as feasible:
  `Quanergy TCP stream -> Rust parser -> HVDIR -> encoder correction -> XYZIR
  in sensor frame -> T_station_sensor -> XYZIR in tamping-station frame ->
  qpcd/pcd/database`.
- The current Rust SDK already has:
  - `TcpPacketSource` for TCP port 4141 capture.
  - 20-byte packet-header parsing with signature `0x75bd7e97`.
  - packet type dispatch for `0x00`, `0x01`, `0x04`, and `0x06`.
  - M-Series frame assembly on sweep/wrap, not per-packet fake frames.
  - deviceInfo fetch from HTTP port 7780 path `/PSIA/System/deviceInfo`.
  - deviceInfo parsing for model, vertical angles, encoder amplitude, and
    encoder phase.
  - encoder correction and automatic calibration support.
  - HVDIR-to-XYZIR conversion into `Frame<PointXyzir>`.
  - `.qraw` raw recording plus `.qraw.toml` calibration sidecar for replay.
- The current Rust SDK does not yet have:
  - a station/extrinsic transform module.
  - a production point-cloud binary storage format.
  - a database schema or writer for frame metadata.
  - a storage-oriented app/CLI path separate from visualizer rendering.
- The C++ reference does not provide tamping-station storage or database design.
  This is business-specific work, not original SDK parity.
- Per-point relational database rows are explicitly discouraged by the project
  plan because high-rate point clouds can produce hundreds of thousands of
  points per frame.
- The user-provided site concept has one scanner looking downward over a
  tamping station, 32 tamping hammers split into four groups of eight. The
  red-box hammer installation space and later group-height calculation depend
  on a stable station coordinate system.
- The user decided the first implementation should persist `capture_session`
  and `scan_frame` only. Work cycles or passes should remain optional
  `session_id` / `notes` / `status` metadata for now, or be added as a later
  extension table.
- The scanner is fixed above the station and scans downward, but may be mounted
  with a non-zero tilt angle. Field staff can adjust the scanner pose at any
  time, so the sensor-to-station coordinate transform must be exposed as a
  reusable, configurable method rather than hard-coded.
- The user selected yaw/pitch/roll Euler angles in degrees for the default
  pose-based transform config, with field names `yaw_deg`, `pitch_deg`, and
  `roll_deg`.
- The yaw/pitch/roll transform is only the default example. The design must
  allow users to define and swap in a different coordinate conversion algorithm
  later without rewriting capture or storage code.
- The user confirmed local SQLite for the first-version metadata database.
- The station is fixed during scanning; there is no vehicle-motion coordinate
  frame in the first version. The station coordinate system should be a
  three-dimensional Cartesian frame with its origin at a fixed lower-left point
  on the station/site.
- The user agreed that from the lower-left origin, station `X` points right,
  station `Y` points upward / deeper into the station drawing, and station `Z`
  points vertically upward. Units are meters.
- The user decided raw `.qraw` should not be continuously saved by default in
  the first version. Production storage defaults to transformed `.qpcd` frame
  files plus SQLite metadata. A `--record-raw` switch should enable synchronized
  `.qraw` recording for debugging or calibration.

## Requirements

- Reuse the existing Rust SDK pipeline as the source of full-resolution
  `Frame<PointXyzir>` data.
- Define a configured rigid transform from sensor frame to station frame.
- The station coordinate frame must use the fixed lower-left site point as
  origin.
- From that origin, station coordinates use `X` to the right, `Y` upward /
  deeper into the station drawing, and `Z` vertically upward, in meters.
- The transform configuration must accept the scanner origin relative to the
  station origin plus adjustable tilt/orientation angles, then produce and
  apply the corresponding rigid transform.
- The default transform configuration must expose `yaw_deg`, `pitch_deg`, and
  `roll_deg` in degrees.
- The transform method must be reusable by live capture, replay conversion, and
  later measurement tools.
- Capture and storage orchestration must depend on a transform interface or
  strategy boundary, not directly on the yaw/pitch/roll algorithm, so future
  conversion algorithms can replace the default.
- Apply that transform before production persistence so downstream consumers do
  not need to know scanner mounting pose.
- Persist one binary point-cloud file per completed frame, not one database row
  per point.
- Persist database metadata that lets another Rust program enumerate frames,
  locate the binary point-cloud file, know which coordinate frame it is in, and
  verify calibration/extrinsic provenance.
- Use local embedded SQLite for the first-version metadata database.
- Use a first-version database model centered on `capture_session` and
  `scan_frame`; do not add first-class work-cycle/pass tables in this task.
- Retain `.qraw` as a raw replay/debug artifact where needed, but do not make
  `.qraw` the production transformed-frame storage format.
- Do not continuously save `.qraw` by default. Add a `--record-raw` option for
  debug/calibration sessions that need raw replay.
- Keep the storage path bounded and non-blocking relative to sensor ingestion:
  capture/parsing must not stall on slow database or disk writes.
- Make the design compatible with Windows x86_64-pc-windows-msvc first.
- Keep this task scoped to data acquisition, transform, and storage design.

## Acceptance Criteria

- [ ] Planning artifacts explain whether the current SDK can support the
      proposed data acquisition and storage path.
- [ ] Planning artifacts identify the current implementation gaps between the
      visualizer pipeline and a production storage pipeline.
- [ ] Planning artifacts choose or recommend a database/storage shape for the
      first version and explain trade-offs.
- [ ] Planning artifacts define the minimum metadata needed by a downstream
      program to read station-frame point clouds safely.
- [ ] Planning artifacts define validation checks for capture, transform, and
      storage without requiring later ROI segmentation.
- [ ] Planning artifacts list the open product decisions that cannot be answered
      from repository evidence.

## Out Of Scope

- Red-box ROI segmentation.
- 32-hammer or 4-group height calculation.
- Tamping-hammer measurement statistics such as p99.5/top-N.
- Multi-sensor fusion.
- C++ SDK ABI compatibility.
- Linux/macOS-first design.
- Per-point SQL table storage for production frames.
- Replacing or redesigning the current visualizer.

## Resolved Decisions

- First-version metadata schema records continuous complete frames under
  `capture_session + scan_frame`. Station work cycles/passes are not
  first-class entities in this task.
- The station transform is a configurable method built from scanner pose:
  translation relative to station origin plus orientation/tilt angles. The
  active pose can be changed in the field and should be snapshotted with frame
  metadata.
- The default field-facing angle convention is yaw/pitch/roll Euler angles in
  degrees, named `yaw_deg`, `pitch_deg`, and `roll_deg`.
- The yaw/pitch/roll transform is not a permanent algorithm lock-in. The SDK
  must expose a replaceable coordinate transform strategy so downstream users
  can define a new algorithm later.
- The first-version metadata database is local embedded SQLite.
- The station coordinate origin is a fixed lower-left point in the physical
  station/site. The first version does not model vehicle motion.
- Station axes are fixed as `X` to the right, `Y` upward / deeper into the
  station drawing, and `Z` vertically upward, with meter units.
- First-version production storage writes transformed `.qpcd` files and local
  SQLite metadata by default. Raw `.qraw` recording is opt-in via
  `--record-raw`.

## Open Questions

- None blocking for first implementation planning. Artifact review is still
  required before `task.py start`.

## Notes

- The highest-risk unknown is not whether the SDK can produce XYZIR frames; the
  current code already does that. The highest-risk product decision is how the
  physical station coordinate frame should be defined for downstream
  measurement.
