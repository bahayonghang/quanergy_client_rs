# Quanergy Client RS

`quanergy_client_rs` is a Rust functional rewrite of the useful Quanergy C++
client SDK data path. The current repository focuses on the original SDK
workflow: capture packets, parse supported packet types, apply calibration and
filters, convert HVDIR points to XYZIR frames, visualize live or replayed data,
and persist station-frame point clouds for later measurement work.

The first milestone targets Windows `x86_64-pc-windows-msvc`. C++ ABI
compatibility, PCL, Boost, VTK, and Linux/macOS-first design are not goals for
this milestone.

## Workspace

| Path | Role |
| --- | --- |
| `crates/quanergy-client` | Reusable SDK library for capture, protocol, calibration, frames, replay, transforms, and storage. |
| `apps/visualizer` | Rerun-based live/replay visualizer and qraw recorder. |
| `apps/capture-store` | Station-frame capture and replay storage app writing PCD 0.7 frames (`.pcd`) plus SQLite metadata. |
| `ref/quanergy_client` | Local C++ SDK reference used for protocol and behavior parity. |

## Common Commands

```powershell
rtk just ci
rtk cargo run -p visualizer -- live --host 192.0.2.10
rtk cargo run -p visualizer -- replay sample.qraw --rerun-save sample.rrd
rtk cargo run -p capture-store -- live --host 192.0.2.10 --output-dir capture-output
rtk cargo run -p capture-store -- replay sample.qraw --output-dir replay-output
```

## Documentation Map

- [Architecture](./architecture.md): workspace boundaries and current-vs-future scope.
- [Implementation](./implementation.md): packet, calibration, frame, transform, and storage data flow.
- [Visualizer](./visualizer.md): live visualization, replay, recording, and Rerun output.
- [Capture Store](./capture-store.md): station-frame capture, replay storage, PCD 0.7, and SQLite metadata.
