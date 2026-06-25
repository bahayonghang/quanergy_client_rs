# Directory Structure

> How backend code is organized in this project.

---

## Overview

The reusable SDK library lives under `crates/quanergy-client/src`. Keep its
module tree aligned with the rewrite architecture in `ref/refactor_plan.md`:
capture/networking, protocol parsing, calibration, point clouds, pipeline
processing, replay, and later conversion/storage/measurement work should have
separate module boundaries.

Use directory modules when a domain contains multiple responsibilities or grows
large enough that navigation suffers. Keep small, focused domains as single
files.

---

## Directory Layout

```text
src/
├── calibration/
│   └── mod.rs
├── config/
│   ├── device_info.rs
│   ├── mod.rs
│   ├── settings.rs
│   └── xml.rs
├── pipeline/
│   ├── dispatch.rs
│   ├── helpers.rs
│   ├── m_series.rs
│   ├── mod.rs
│   └── packet_01.rs
├── protocol/
│   └── mod.rs
├── replay/
│   ├── mod.rs
│   ├── qraw.rs
│   └── sidecar.rs
├── cloud.rs
├── error.rs
├── filters.rs
├── lib.rs
└── net.rs
```

---

## Module Organization

- Preserve public module paths when reorganizing internals. For example,
  `quanergy_client::pipeline::SensorPipeline` should remain available even if
  the implementation moves behind `pipeline/mod.rs`.
- Prefer `mod.rs` as a small assembly and re-export point for directory modules.
  Keep the domain implementation in focused sibling files.
- Split large multi-responsibility modules by existing domain boundaries rather
  than introducing abstract layers. Example: `pipeline/dispatch.rs` handles
  packet type dispatch, `pipeline/m_series.rs` handles stateful M-Series
  parsing, and `pipeline/packet_01.rs` handles the stateless HVDIR-list parser.
- Keep tests close to the module they validate using `tests.rs` inside the
  directory module when the module has been split.
- Do not create placeholder modules for planned domains such as `convert/`,
  `storage/`, or `measure/`. Add those directories only when code actually
  lands there.
- Do not split small focused modules just to make every domain a directory.
  `cloud.rs`, `net.rs`, `filters.rs`, and `error.rs` are acceptable as
  single-file modules while they remain narrow.

---

## Naming Conventions

- Use snake_case file and directory names.
- Name directory modules after the public domain path they preserve:
  `config/`, `pipeline/`, `replay/`.
- Name implementation files after concrete responsibilities:
  `device_info.rs`, `settings.rs`, `qraw.rs`, `sidecar.rs`, `m_series.rs`,
  `packet_01.rs`.

---

## Examples

- `crates/quanergy-client/src/pipeline/` keeps public pipeline orchestration in
  `mod.rs`, parser dispatch in `dispatch.rs`, and packet-specific parsing in
  focused implementation files.
- `crates/quanergy-client/src/config/` preserves the public `config` API while
  separating deviceInfo parsing, settings application, and XML helpers.
- `crates/quanergy-client/src/replay/` preserves the public `replay` API while
  separating qraw binary IO from sidecar metadata.
