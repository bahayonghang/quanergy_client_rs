# VitePress Docs

## Goal

Create a repository-local VitePress documentation site under `docs/` that
documents the current Quanergy Rust rewrite architecture, implemented data-path
details, and real usage for the `visualizer` and `capture-store` applications.

The immediate user value is a browsable, maintainable project manual that helps
future SDK, visualization, capture, storage, and measurement work start from the
actual codebase instead of scattered task notes.

## Confirmed Facts

- The repository is a Rust workspace with three members:
  `crates/quanergy-client`, `apps/visualizer`, and `apps/capture-store`.
- There is currently no `docs/` directory, no VitePress config, and no
  `package.json`.
- The root `justfile` currently contains Rust-only commands:
  `ci`, `dev`, `build`, `release`, `test`, `fmt`, `fmt-check`, `clippy`,
  `check`, and `clean`.
- The root `justfile` uses PowerShell as its command shell.
- Project guidance requires local shell commands to be prefixed with `rtk`.
- The core SDK library exports modules for calibration, cloud, config, error,
  filters, net, pipeline, protocol, replay, storage, and transform.
- The core SDK public API re-exports `Frame`, `PointHvdir`, `PointXyzir`,
  `QuanergyError`, and `Result`, plus C++ migration aliases `PointHVDIR` and
  `PointXYZIR`.
- The core SDK implements packet-header parsing and raw packet handling,
  deviceInfo fetch/parsing, packet dispatch, parser coverage for `0x00`,
  `0x01`, `0x04`, and `0x06`, frame assembly, filtering, encoder correction,
  HVDIR-to-XYZIR conversion, qraw replay, qraw sidecars, qpcd storage, SQLite
  metadata, and station-frame transforms.
- `visualizer` is a separate application package that uses the core SDK and
  Rerun. It supports live visualization, qraw replay, qraw recording, optional
  Rerun connect/save output, calibration/filter flags, and top-level default
  live behavior.
- `capture-store` is a separate application package that uses the core SDK. It
  supports live and replay storage paths, station-frame pose parameters
  `x_m`, `y_m`, `z_m`, `yaw_deg`, `pitch_deg`, `roll_deg`, output directory and
  SQLite database selection, session metadata, bounded storage queue capacity,
  `.qpcd` frame output, and opt-in raw recording via `--record-raw`.
- The current project guidance says first-milestone scope focuses on original
  SDK functionality and visualizer parity, while tamping-station measurement is
  a later business extension.
- The archived storage planning task selected local SQLite plus one `.qpcd`
  file per completed frame, not per-point SQL rows.
- `.trellis/spec/frontend/index.md` says documentation should be written in
  English.
- Official VitePress getting-started guidance supports adding VitePress into an
  existing project with a nested `docs` source directory, `docs/.vitepress`
  config, and package scripts such as dev/build/preview. The current VitePress
  docs state Node.js 20+ is required.

## Requirements

- Create a `docs/` directory for all VitePress source documentation.
- Add VitePress project metadata and scripts without moving or restructuring
  the Rust workspace.
- Use `docs/package.json` for documentation scripts so Node metadata remains
  inside the documentation subsystem while the root `justfile` still exposes
  repo-level docs commands.
- Add `docs/.vitepress/config.mts` with navigation and sidebar entries for the
  required documentation areas.
- The documentation site must include at least:
  - an overview/home page;
  - project architecture;
  - implementation details;
  - visualizer usage;
  - capture-store usage.
- The first documentation version must be bilingual in English and Chinese.
- Architecture docs must describe the real workspace split between
  `quanergy-client`, `visualizer`, and `capture-store`.
- Architecture docs must distinguish current first-milestone SDK functionality
  from later tamping-station measurement work.
- Implementation docs must cover the actual data path from TCP/qraw packets
  through parsing, calibration, frame assembly, XYZIR conversion, transform,
  qpcd, and SQLite metadata where implemented.
- Visualizer usage docs must document real command shapes from the current
  `apps/visualizer` CLI, including live, replay, record, common pipeline flags,
  and Rerun output flags.
- Capture usage docs must document real command shapes from the current
  `apps/capture-store` CLI, including live, replay, transform fields, storage
  options, queue capacity, and `--record-raw`.
- Documentation must not present out-of-scope or future business features such
  as ROI segmentation or hammer-height calculation as implemented behavior.
- Add a `docs` recipe to the root `justfile` that starts the VitePress dev
  server.
- Add build and preview recipes unless implementation discovers a better local
  convention. Recommended names are `docs-build` and `docs-preview`.
- Keep generated VitePress output such as `docs/.vitepress/dist/` out of Git.
- Preserve existing Rust commands and their behavior.

## Acceptance Criteria

- [ ] `docs/` exists and contains VitePress source pages for overview,
      architecture, implementation details, visualizer usage, and capture-store
      usage.
- [ ] `docs/.vitepress/config.mts` defines site title, nav, and sidebars that
      link to all required pages.
- [ ] `docs/package.json` includes VitePress as a dev dependency and provides
      docs dev/build/preview scripts.
- [ ] `just docs` starts the VitePress development server through the docs
      package script.
- [ ] A VitePress build command succeeds locally and generates static output
      under `docs/.vitepress/dist/`.
- [ ] `.gitignore` excludes generated VitePress output and Node dependency
      folders.
- [ ] Existing `just --list` still shows the Rust commands and the new docs
      commands.
- [ ] Existing Rust quality gate still passes or, if skipped for time, the
      remaining risk is explicitly reported.
- [ ] Documentation examples match the current CLI flags and do not describe
      unimplemented measurement features as available.

## Out Of Scope

- Implementing new SDK, visualizer, capture, storage, ROI, or measurement
  behavior.
- Replacing Rerun or changing visualizer runtime behavior.
- Changing qraw, qpcd, or SQLite schemas.
- Creating a hosted deployment pipeline for the docs site.
- Adding screenshots or generated visual assets unless requested later.
- Adding languages beyond English and Chinese in the first version.

## Resolved Decisions

- The first documentation version is bilingual English/Chinese.
- Bilingual content uses VitePress locale routes: English at `/` and Chinese
  under `/zh/`.

## Open Questions

- None blocking for implementation planning. Artifact review is still required
  before `task.py start`.
