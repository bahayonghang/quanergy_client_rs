<!-- TRELLIS:START -->
# Trellis Instructions

These instructions are for AI assistants working in this project.

This project is managed by Trellis. The working knowledge you need lives under `.trellis/`:

- `.trellis/workflow.md` — development phases, when to create tasks, skill routing
- `.trellis/spec/` — package- and layer-scoped coding guidelines (read before writing code in a given layer)
- `.trellis/workspace/` — per-developer journals and session traces
- `.trellis/tasks/` — active and archived tasks (PRDs, research, jsonl context)

If a Trellis command is available on your platform (e.g. `/trellis:finish-work`, `/trellis:continue`), prefer it over manual steps. Not every platform exposes every command.

If you're using Codex or another agent-capable tool, additional project-scoped helpers may live in:
- `.agents/skills/` — reusable Trellis skills
- `.codex/agents/` — optional custom subagents

Managed by Trellis. Edits outside this block are preserved; edits inside may be overwritten by a future `trellis update`.

<!-- TRELLIS:END -->

# Project Context

This repository is a Rust functional rewrite of the local Quanergy C++ SDK
reference under `ref/quanergy_client`. Tamping-station measurement is a later
business extension, not part of the first original-SDK rewrite milestone. The
current source of technical direction is `ref/refactor_plan.md`; read it before
planning or implementing protocol, capture, parsing, calibration,
visualization, point-cloud storage, or measurement work.

The intended scope is to replace the useful SDK behavior in Rust without
retaining the C++/PCL/Boost/VTK dependency chain. C++ ABI compatibility is not a
goal. The first implementation priority is the `visualizer` path: real-time
sensor capture, parsing, XYZIR conversion, intensity-colored point-cloud
rendering, and non-blocking UI updates.

The first complete rewrite milestone only needs to support
`Windows x86_64-pc-windows-msvc`. Do not let Linux/macOS compatibility drive
first-milestone design decisions unless the task explicitly changes scope.

First-milestone scope is original SDK functionality only: capture, replay,
packet parsing, pipeline processing, encoder calibration, filters,
HVDIR-to-XYZIR conversion, real-time visualization, dynamic connection behavior,
and C++ CLI/XML settings compatibility. Station-frame transforms, storage
formats, ROI processing, and tamping-hammer height measurement are follow-up
business work.

Expose a reusable Rust library API for first-milestone SDK functionality. The
CLI should be a thin wrapper over library modules, not the place where protocol,
pipeline, calibration, or visualization logic lives.

# Implementation Priorities

- Prioritize real-time visualizer parity with the C++ `visualizer` app while
  keeping the capture/parser/pipeline modules reusable by CLI, storage, and
  measurement tools.
- The protocol foundation still starts with connecting to the Quanergy TCP
  stream on port `4141`, reading the 20-byte packet header, verifying signature
  `0x75bd7e97`, logging packet size/version/type/timestamps, and optionally
  saving raw `.qraw` packets for replay.
- Any live recording path that writes `.qraw` must also write a `.qraw.toml`
  sidecar with deviceInfo/calibration metadata when available, or an explicit
  calibration-incomplete marker and error reason when deviceInfo fails.
- Implement SDK packet parser coverage for `0x00`, `0x01`, `0x04`, and `0x06`.
  Prioritize the packet types needed for the first visualizer run, but design
  parser dispatch so the full reference parser set fits without visualizer
  rewrites.
- Use the official SDK behavior as a reference for wire format, scaling, angle
  lookup, encoder correction, and HVDIR-to-XYZIR conversion. Keep the Rust
  implementation small and testable rather than binding to PCL/Boost/VTK.
- Fetch calibration from the sensor deviceInfo endpoint when needed:
  HTTP port `7780`, path `/PSIA/System/deviceInfo`. Support model, vertical
  angles, and encoder amplitude/phase. Allow config overrides for field use.
- Implement M-Series automatic encoder calibration for first visualizer parity,
  including calculate/apply behavior equivalent to the C++ SDK.
- Frame assembly matters. Do not treat each TCP packet as a complete scan unless
  the current task explicitly chooses a packet-level debug mode.
- For storage, prefer one binary point-cloud file per frame plus database
  metadata/results. Avoid one database row per point in production paths.

# Rust Architecture Guidance

Keep module boundaries aligned with the rewrite plan:

```text
net/          TCP capture, reconnect, raw packet recording
protocol/     packet header and packet-type parsers
calibration/  deviceInfo, vertical angles, encoder correction
cloud/        HVDIR, XYZIR, frames
convert/      polar-to-cartesian and station transforms
storage/      qpcd/bin, optional PCD debug export, metadata database
measure/      ROI processing and tamping-hammer height extraction
```

Prefer the simplest runtime that satisfies the task. For capture and parser
milestones, bounded channels are required so visualization, storage, or
measurement work cannot stall packet ingestion. Use Tokio only when the task has
a clear need for it.

# Validation Expectations

Every implementation slice should have a concrete check:

- raw recorder: proves stable connection, valid signatures, sane packet sizes,
  packet type/version discovery, and raw packet persistence
- parser: proves point counts, distance units, ring ordering, scan direction,
  and frame completion behavior using captured raw packets or fixtures
- calibration/conversion: proves vertical angle handling, encoder correction,
  coordinate sign conventions, and HVDIR-to-XYZIR output against known samples
- storage/measurement: proves frame metadata, file readability, ROI grouping,
  and robust height statistics such as percentile or top-N based results

When official SDK output is unavailable, validate against raw captures and known
geometry checks such as measured planes, walls, calibration boards, or known
height targets.

# Local Command Rule

The local RTK command wrapper applies in this repository. Prefix shell commands
with `rtk` unless deliberately using an approved raw shell escape.
