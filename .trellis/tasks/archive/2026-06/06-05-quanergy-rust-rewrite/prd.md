# Plan Quanergy Rust Rewrite

## Goal

Plan the Rust rewrite of the Quanergy C++ SDK workflow as a full functional
replacement for this repository's local official SDK reference, with the first
implementation priority being the `visualizer` real-time point-cloud viewing
path.

The plan should remove the C++/PCL/Boost/VTK dependency chain while preserving
the useful SDK behavior in Rust: sensor capture, packet parsing, calibration,
filtering, HVDIR-to-XYZIR conversion, live visualization, and dynamic
connection control.

## User Value

- Avoid the deployment and maintenance cost of reviving the legacy C++ SDK with
  PCL, Boost, VTK, and old Visual Studio/vcpkg constraints.
- Build a Rust-native data path that can directly capture raw Quanergy packets,
  parse supported SDK packet formats, convert HVDIR to XYZIR, visualize live
  point clouds, and provide the original SDK example-app behavior without
  PCL/Boost/VTK.
- Keep the early milestones independently testable, especially before the field
  device packet type/version is known.

## Confirmed Facts

- The repository currently contains Trellis scaffolding, `AGENTS.md`,
  `.gitignore`, `ref/refactor_plan.md`, and a local copy of the official C++
  SDK under `ref/quanergy_client`; no Rust source has been created yet.
- The official SDK README describes the library as consuming raw Quanergy sensor
  data and producing PCL point clouds. Example apps are `visualizer` and
  `dynamic_connection`.
- The official SDK CMake project is C++11 and depends on PCL and Boost. PCL
  visualization is optional for `NoViz`/cross-compiling, but PCL common/io and
  Boost program_options/system remain part of the SDK build.
- The common packet header is a packed 20-byte structure with signature
  `0x75bd7e97`, packet `size`, seconds/nanoseconds timestamps, semantic version
  bytes, and `packet_type`. Multi-byte fields are deserialized from network byte
  order.
- The `visualizer` app defaults the sensor TCP port to `4141`, constructs a
  `SensorClient`, connects raw packets into `SensorPipeline`, then consumes
  `PointXYZIR` clouds for visualization.
- The C++ `VisualizerModule` uses PCLVisualizer with a black background, a
  coordinate system, camera position `(0, 0, 30)`, clip distances `0..50`, and
  point coloring by `intensity`. Incoming clouds update or add one point cloud
  keyed by `frame_id`. The visualizer avoids blocking the data path by trying to
  lock the cloud mutex for only 10 ms in the point-cloud slot.
- The `dynamic_connection` app shares the same sensor pipeline and CLI settings
  as `visualizer`, but provides `run`, `stop`, and `exit` commands to connect,
  disconnect, reset calibration when needed, and count received clouds.
- `SensorPipeline` wires a variadic parser over packet parsers `0x00`, `0x01`,
  `0x04`, and `0x06`, then applies encoder calibration, distance/ring-intensity
  filtering, polar-to-cartesian conversion, and async point-cloud delivery.
- `SensorPipelineSettings` supports host, frame name, return selection
  (`0`, `1`, `2`, or `all`), encoder calibration/override amplitude+phase,
  calibration frame rate, min/max distance filter, min/max cloud size, and
  M-Series ring filter thresholds from the XML settings file.
- The reference settings file is `ref/quanergy_client/settings/client.xml`.
  It contains `Settings.host`, `Settings.frame`, `Settings.return`,
  `Settings.EncoderCorrection.{calibrate,frameRate,override,amplitude,phase}`,
  `Settings.DistanceFilter.{min,max}`, `Settings.minCloudSize`,
  `Settings.maxCloudSize`, and ring filter entries. The C++ loader code looks
  up ring filter keys as `range0/intensity0` through `range7/intensity7`, while
  the sample XML uses `Range0/Intensity0` through `Range7/Intensity7`.
- M-Series packets use 50 firings per TCP packet, 8 lasers, and up to 3 returns.
  API version 5 uses 10 micrometer distance units; older versions use 10 mm.
- Packet type `0x04` is reduced-bandwidth M-Series data. The C++ parser accepts
  version `0.1.0`, requires vertical angles, uses 50 firings, maps firing
  position through the M-Series horizontal-angle LUT, emits one return_id, and
  scales radius by `0.00001` meters.
- Packet type `0x00` is M-Series data. The parser accepts version `0.1.0`,
  supports single-return or all-returns mode, removes duplicate all-return
  distances, and switches distance scaling based on API version.
- Packet type `0x01` is an HVDIR-like point-list packet. It validates version
  `0.1.0`, deserializes point count and sequence, converts integer horizontal
  and vertical angle fields to standard HVDIR, scales range from micrometers to
  meters, and assigns ring ids by grouping close vertical angles.
- Packet type `0x06` is M1 data. It validates version `0.1.0` and dispatches
  parsing based on whether `return_id` indicates one return or all three
  returns.
- M-Series horizontal angles use `M_SERIES_NUM_ROT_ANGLES = 10400` with an LUT
  spanning `[-pi, pi]` after a half-rotation index shift.
- M-Series frame assembly is sweep-based. The parser tracks direction and
  azimuth progress, and completes a cloud on configured sweep angle or full
  360-degree wrap. A TCP packet is not the same thing as a complete scan frame.
- Device calibration is fetched through HTTP from
  `/PSIA/System/deviceInfo`. The C++ SDK reads model, optional encoder
  amplitude/phase, and optional per-laser vertical angles from the XML payload.
- The C++ HTTP client defaults to port `7780`.
- Encoder correction applies a sinusoidal offset:
  `offset(angle) = amplitude * sin(angle + phase)`, with zero offset computed at
  angle `0`. Applying correction adjusts `h` by `zero_offset - offset(h)` and
  wraps back into `[-pi, pi]`.
- The C++ SDK also contains automatic encoder calibration, but the refactor plan
  recommends not implementing it in the first MVP unless field validation shows
  it is needed.
- HVDIR to XYZIR conversion is:
  `xy = d * cos(v)`, `x = xy * cos(h)`, `y = xy * sin(h)`,
  `z = d * sin(v)`, preserving intensity and ring. NaN distance yields NaN
  coordinates.
- `AGENTS.md` now records this repository as a focused Rust rewrite, not a full
  SDK clone, and says the first implementation should start with protocol
  confirmation and raw packet recording.
- Visualizer stack research:
  - Rerun's Rust SDK supports logging `Points3D` and updating point clouds over
    time. Its SDK operating modes include spawning an external viewer,
    connecting to a viewer over gRPC, serving, saving, and stdout output.
  - egui/eframe is an official Rust immediate-mode GUI stack for native and web
    apps. `egui-wgpu` supports custom wgpu rendering inside an egui app.
  - kiss3d provides a simple Rust 3D window with camera controls and point
    rendering support, but it is a narrower 3D scene library rather than a full
    application shell.

## Requirements

- Define the staged full-rewrite scope before implementation starts.
- Use local evidence from `ref/refactor_plan.md` and `ref/quanergy_client` as
  the current source of protocol and SDK behavior.
- Prefer independently verifiable milestones:
  1. Rust project skeleton, shared data types, and protocol capture foundation
  2. real-time visualizer path equivalent to the C++ `visualizer`
  3. packet parser coverage for SDK packet types `0x00`, `0x01`, `0x04`, and
     `0x06`
  4. deviceInfo calibration, encoder correction, distance/ring filters,
     HVDIR-to-XYZIR conversion, and frame assembly
  5. dynamic connect/disconnect behavior equivalent to `dynamic_connection`
  6. original SDK parity review and compatibility hardening
- Prioritize getting live visualization working first, but design parser and
  pipeline modules so the remaining SDK packet formats can be added without
  rewriting the visualizer.
- Do not include tamping-station storage, station-frame transforms, ROI
  processing, or height measurement in the first complete rewrite milestone.
  Those are follow-up business extensions after original SDK functionality is
  working in Rust.
- Treat official SDK output as preferred reference data when available; otherwise
  rely on raw captures, deterministic fixtures, and known-geometry checks.
- When real sensor hardware or C++ SDK reference output is unavailable, first
  milestone acceptance may rely on fixed packet fixtures, algorithm unit tests,
  and replay visualizer smoke tests.
- For this complex task, create `design.md` and `implement.md` before
  `task.py start`.

## Acceptance Criteria

- [ ] `prd.md` identifies MVP scope, out-of-scope items, and testable acceptance
      criteria.
- [ ] `design.md` defines architecture boundaries, data flow, protocol contracts,
      calibration behavior, storage format direction, and key trade-offs.
- [ ] `implement.md` defines ordered implementation milestones, validation
      commands/checks, risky files or rollback points, and pre-start review
      gates.
- [ ] Remaining open questions are only user intent/scope/risk decisions, not
      facts discoverable from the repository.
- [ ] The user reviews and approves the planning artifacts before implementation
      starts.

## Scope Boundaries

- PCL/Boost/VTK bindings or a C++ compatibility layer.
- C++ ABI compatibility is not required; this is a Rust functional rewrite, not
  a drop-in binary/library replacement.
- Visualizer functionality is in scope and has first priority.
- The first visualizer implementation will use Rerun Viewer as the rendering
  frontend. The Rust rewrite should expose a visualizer sink that streams
  `XYZIR` frames as Rerun `Points3D` with intensity-based coloring, while
  keeping the sink boundary replaceable for a future embedded/custom UI.
- The first visualizer milestone must support both live sensor input and offline
  replay. `live` reads Quanergy packets from TCP `4141`; `replay` reads saved
  raw packet streams from disk. Both inputs must feed the same parser, pipeline,
  and Rerun visualizer sink.
- Offline replay in the first visualizer milestone supports the rewrite's
  native `.qraw` stream format only. A `.qraw` file is an append-only stream of
  complete Quanergy packets, each preserving the original 20-byte packet header
  and full packet body. PCAP/PCAPNG import is out of scope for the first
  visualizer milestone.
- Offline replay must work without a live sensor deviceInfo endpoint when
  calibration/model data is provided out of band. Replay calibration priority:
  1. explicit CLI/XML settings overrides for model, vertical angles, and encoder
     amplitude/phase
  2. `.qraw` sidecar metadata such as `capture.qraw.toml`, containing model,
     vertical angles, encoder amplitude/phase, and capture source
  3. M8 default vertical angles if no explicit vertical angles are available
  4. error for MQ replay without vertical angles
  5. M1 replay does not require M-Series vertical angles
- Any live recording path, including `record` and `visualizer live --record`,
  must generate a `.qraw.toml` sidecar next to the `.qraw` stream. When
  deviceInfo is available, sidecar metadata must include at least sensor host,
  capture start time, rewrite version, model, vertical angles, and encoder
  amplitude/phase if present. When deviceInfo fails, a sidecar must still be
  written with calibration marked incomplete and the failure reason recorded.
- `visualizer live` must not record by default. It should only write `.qraw`
  when the user explicitly passes a recording path such as `--record <path>`.
  A separate `record` subcommand must exist for dedicated raw packet capture.
- The Rerun visualizer may downsample points for display to protect real-time
  rendering and transport performance, but this must be configurable and
  disableable. Display downsampling must not affect parser, pipeline, storage,
  replay, or measurement outputs, which retain full-resolution frames.
- Rerun output defaults to spawning Rerun Viewer for interactive use. The CLI
  must also support connecting to an already running viewer, for example
  `--rerun-connect <addr>`, and saving a Rerun recording file, for example
  `--rerun-save <file.rrd>`, for CI and non-GUI debugging.
- Rerun visualizer defaults: color points by intensity and cap display output to
  300000 points per frame; `--visualizer-max-points 0` disables display-only
  downsampling.
- The Rust rewrite should expose one primary CLI with subcommands, for example
  `visualizer live`, `visualizer replay`, `record`, and `dynamic-connection`.
  Thin compatibility wrapper binaries or aliases for C++ example names such as
  `visualizer` and `dynamic_connection` may be added later if needed, but core
  logic should live behind the shared CLI/library modules.
- The first milestone must expose a reusable Rust library API in addition to the
  CLI. CLI commands should be thin entry points over library modules such as
  capture/replay, protocol parsing, pipeline processing, calibration, filters,
  point types, and visualizer sinks.
- The public Rust library API should prioritize idiomatic Rust module and type
  names. Documentation and a small number of type aliases may preserve C++ SDK
  concept mappings for migration familiarity.
- The first complete rewrite milestone only needs to support and validate
  `Windows x86_64-pc-windows-msvc`. Linux and macOS compatibility are not
  acceptance requirements for this milestone.
- Windows packaging/distribution artifacts are not required for the first
  milestone. Acceptance is based on local Cargo build/test and runnable CLI
  behavior, not MSI installers, portable zips, signing, or release automation.
- SDK packet parser coverage for `0x00`, `0x01`, `0x04`, and `0x06` is in scope.
- The first visualizer milestone must support all four SDK parser families:
  `0x00`, `0x01`, `0x04`, and `0x06`. A narrower parser subset is not
  acceptable for the first visualizer milestone.
- Automatic M-Series encoder calibration is required in the first visualizer
  milestone, in addition to deviceInfo-provided amplitude/phase, manual
  amplitude/phase override, and disabled correction modes.
- First-milestone automatic encoder calibration must implement the C++ SDK's
  core calibration loop, but not `calibrateOnly` CSV/debug-output mode. Required
  behavior includes `--calibrate`, `--frame-rate`, complete 360-degree frame
  detection, timeout handling, phase convergence, amplitude threshold handling,
  applying completed calibration to subsequent HVDIR points, and unit tests for
  `calculate()` using synthetic signals plus exceptional inputs.
- First visualizer CLI and config must preserve the C++ SDK entry points:
  `--settings-file`, `--host`, `--frame`, `--return`, `--calibrate`,
  `--frame-rate`, `--manual-correct`, `--min-distance`, `--max-distance`,
  `--min-cloud-size`, and `--max-cloud-size`. The Rust implementation may map
  these inputs into an internal Rust-native config struct.
- XML settings-file compatibility with the C++ SDK is required for first
  visualizer parity.
- XML ring filter parsing must accept both lowercase keys used by the C++ loader
  code (`range0/intensity0` ... `range7/intensity7`) and uppercase keys used by
  the sample XML (`Range0/Intensity0` ... `Range7/Intensity7`).
- Dynamic connection behavior is in scope after the first live visualizer path.
- Automatic encoder calibration is in scope for first visualizer parity.
- Per-point relational database storage for production point clouds.
- Tamping-station storage, station-frame transforms, ROI processing, and height
  measurement are out of scope for the first complete rewrite milestone.

## Decisions

- Use Rerun Viewer for the first real-time point-cloud visualizer.
- Require offline raw replay support in the first visualizer milestone.
- Require parser coverage for `0x00`, `0x01`, `0x04`, and `0x06` in the first
  visualizer milestone.
- Require automatic M-Series encoder calibration in the first visualizer
  milestone.
- Implement the C++ SDK's core automatic encoder calibration loop in first
  milestone; omit `calibrateOnly` CSV/debug-output mode.
- Preserve C++ SDK CLI flag names and XML settings-file compatibility while
  using Rust-native config internally.
- Accept both lowercase and uppercase XML ring filter keys for compatibility
  with the C++ loader code and bundled sample XML.
- Use native `.qraw` as the first replay format. Defer PCAP/PCAPNG import to a
  later tooling stage.
- `.qraw` v1 stores packet timing: file magic/version plus repeated records
  containing packet arrival delta, packet length, and complete Quanergy packet
  bytes.
- Use explicit replay config and `.qraw.toml` sidecar metadata to supply
  calibration/model data when replay cannot contact live deviceInfo.
- Generate `.qraw.toml` sidecar metadata for every live recording path.
- Do not record `.qraw` by default in `visualizer live`; require explicit
  `--record <path>` or use of the dedicated `record` subcommand.
- Default to spawning Rerun Viewer, while supporting explicit Rerun connect and
  Rerun recording save modes.
- Allow configurable display-only downsampling for the Rerun visualizer, with a
  way to disable it.
- Use `tracing`; default log level is `info`, `--verbose` enables debug logging,
  and diagnostics should include counters for dropped and bad packets.
- Parser and stream errors are lenient by default for recoverable packet errors,
  with warnings/counters; `--strict` fails immediately.
- Prefer generated deterministic fixtures in tests; allow only small checked-in
  `.qraw` examples.
- Use one primary Rust CLI with subcommands; keep compatibility wrapper binaries
  optional rather than making them the main architecture.
- Support and validate only `Windows x86_64-pc-windows-msvc` for the first
  complete rewrite milestone.
- First milestone covers original SDK functionality only. Tamping-station
  business features are follow-up work.
- Expose a reusable Rust library API; do not make CLI behavior the only public
  surface.
- Use idiomatic Rust names for modules/types while documenting C++ SDK concept
  mappings and adding only limited migration-friendly aliases.
- Accept fixed fixtures, algorithm unit tests, and replay visualizer smoke tests
  as the baseline evidence package when real hardware or C++ reference output is
  unavailable.
- Do not require Windows packaging/distribution artifacts for first-milestone
  acceptance.

## Open Questions

- None. User approved the displayed implementation plan on 2026-06-05 and asked
  to implement it.

## Notes

- Keep `prd.md` focused on requirements, constraints, and acceptance criteria.
- Lightweight tasks can remain PRD-only.
- For complex tasks, add `design.md` for technical design and `implement.md` for execution planning before `task.py start`.
