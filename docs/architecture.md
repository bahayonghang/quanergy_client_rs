# Architecture

The repository keeps SDK logic in a reusable library and keeps application
behavior in workspace apps. The CLI apps should stay thin wrappers around
library modules; protocol, calibration, pipeline, transform, and storage logic
belong in `crates/quanergy-client`.

## Workspace Boundaries

```text
Quanergy sensor or qraw file
  -> quanergy-client SDK library
  -> visualizer app or capture-store app
```

| Package | Responsibility | Application-only dependencies |
| --- | --- | --- |
| `quanergy-client` | TCP capture, packet protocol, deviceInfo, calibration, filters, frame assembly, replay, transforms, qpcd, SQLite metadata. | None from the visualizer path. |
| `visualizer` | CLI parsing, live/replay orchestration, qraw recording, Rerun sink output. | `rerun`, `clap`, tracing setup. |
| `capture-store` | CLI parsing, station transform configuration, live/replay storage orchestration, bounded storage queue. | `clap`, tracing setup. |

## SDK Modules

| Module | Purpose |
| --- | --- |
| `net` | Connects to the Quanergy TCP stream on port `4141` and fetches deviceInfo XML from HTTP port `7780`. |
| `protocol` | Parses the 20-byte packet header, validates signature `0x75bd7e97`, owns `RawPacket`, return selection, angle lookup, and default vertical angles. |
| `config` | Reads SDK settings and deviceInfo values into `PipelineConfig`. |
| `calibration` | Applies manual or automatic encoder correction. |
| `filters` | Applies distance and ring/intensity filtering before XYZIR output. |
| `pipeline` | Dispatches packet parsers, assembles frames, applies calibration and filters, and emits `Frame<PointXyzir>`. |
| `cloud` | Defines `Frame`, `PointHvdir`, and `PointXyzir`, including HVDIR-to-XYZIR conversion. |
| `replay` | Reads and writes `.qraw` packets plus `.qraw.toml` calibration sidecars. |
| `transform` | Applies station-frame coordinate transforms through a reusable transform interface. |
| `storage` | Writes and reads `.qpcd` files and SQLite metadata. |

## Current Scope

Implemented first-milestone and near-term storage behavior includes:

- TCP packet capture and qraw replay.
- Packet header validation and parser dispatch for `0x00`, `0x01`, `0x04`, and `0x06`.
- DeviceInfo parsing for model, vertical angles, encoder amplitude, and encoder phase.
- M-Series frame assembly by sweep/wrap, not one fake frame per TCP packet.
- Manual and automatic encoder calibration.
- HVDIR-to-XYZIR conversion and reusable frame types.
- Rerun visualization through the `visualizer` app.
- Station-frame transforms, `.qpcd` binary frame files, and SQLite metadata through the SDK and `capture-store` app.

## Future Business Work

Tamping-station ROI segmentation, 32-hammer grouping, height statistics, and
measurement-result tables are future business extensions. They should consume
station-frame point clouds produced by the current storage path instead of
changing the capture, parser, or visualizer architecture.
