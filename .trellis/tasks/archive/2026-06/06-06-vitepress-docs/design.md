# VitePress Docs Design

## Architecture

Add a small documentation subsystem at the repository root:

```text
docs/
  package.json
  package-lock.json
  index.md
  architecture.md
  implementation.md
  visualizer.md
  capture-store.md
  zh/
    index.md
    architecture.md
    implementation.md
    visualizer.md
    capture-store.md
  .vitepress/
    config.mts
```

The Rust workspace remains unchanged. Documentation build tooling lives inside
`docs/` so Node metadata stays scoped to the documentation subsystem. The root
`justfile` remains the repo-level entry point for contributors.

Generated files are excluded:

```text
docs/node_modules/
docs/.vitepress/dist/
```

## VitePress Contract

Use VitePress as a documentation-only dependency:

```json
{
  "private": true,
  "type": "module",
  "scripts": {
    "docs:dev": "vitepress dev .",
    "docs:build": "vitepress build .",
    "docs:preview": "vitepress preview ."
  },
  "devDependencies": {
    "vitepress": "<current compatible version>"
  }
}
```

During implementation, choose the installed version through `npm install -D
vitepress` or the local package manager output instead of hand-writing a stale
version. Current official guidance requires Node.js 20+.

## Justfile Contract

Keep existing Rust recipes untouched and add docs recipes near other workflow
commands:

```just
docs:
    npm --prefix docs run docs:dev

docs-build:
    npm --prefix docs run docs:build

docs-preview:
    npm --prefix docs run docs:preview
```

The `docs` recipe intentionally starts the dev server because the user asked for
a docs command used to start documentation. `docs-build` is the non-interactive
verification command.

## Content Structure

Recommended pages:

```text
docs/index.md
  Purpose, current milestone, quick command table.

docs/architecture.md
  Workspace split, SDK/app boundaries, module map, current vs future scope.

docs/implementation.md
  TCP/qraw -> PacketHeader -> RawPacket -> SensorPipeline -> Frame<PointXyzir>
  -> optional station transform -> qpcd/SQLite data path.

docs/visualizer.md
  visualizer live/replay/record usage and Rerun output flags.

docs/capture-store.md
  capture-store live/replay usage, transform flags, storage output layout,
  SQLite metadata, bounded queue, and --record-raw.

docs/zh/*.md
  Chinese equivalents of the English pages.
```

Do not describe future ROI segmentation, 32-hammer grouping, or height
statistics as implemented. Those can be mentioned only as later work.

Use VitePress locale routing for bilingual docs:

```text
/      English docs
/zh/   Chinese docs
```

This keeps each page readable, lets VitePress expose language-specific nav and
sidebar labels, and avoids long mixed-language pages.

## Source Of Truth

Use repository evidence for examples:

- `Cargo.toml` for workspace members.
- `crates/quanergy-client/src/lib.rs` for SDK module exports.
- `apps/visualizer/src/lib.rs` for visualizer commands and flags.
- `apps/capture-store/src/main.rs` for capture/store commands and flags.
- `ref/refactor_plan.md`, project `AGENTS.md`, and archived Trellis tasks for
  milestone boundaries and business-extension wording.

Do not copy long source snippets into docs. Prefer short command examples and
high-level data-flow diagrams in Markdown.

## Validation

Expected verification commands:

```powershell
rtk npm --prefix docs install
rtk npm --prefix docs run docs:build
rtk just --list
rtk just docs-build
rtk just ci
```

If `just docs` is started during verification, it is a long-running dev server.
Use it only when manual browser inspection is needed, then stop the process
before ending the turn.

## Trade-offs

Root package vs nested docs package:

- A nested `docs/package.json` was selected by the user. It isolates Node
  metadata from the Rust workspace root.
- Root commands stay simple through `just docs`, `just docs-build`, and
  `just docs-preview`, which call `npm --prefix docs`.

English-only vs bilingual:

- English-only matches `.trellis/spec/frontend/index.md` and is faster to keep
  synchronized with code.
- Bilingual docs were selected by the user. This helps local operators but
  doubles review and maintenance effort.

Locale routes vs same-page bilingual sections:

- Locale routes were selected by the user. They keep pages shorter and match
  VitePress' built-in i18n model.
- Same-page bilingual sections make direct comparison easy, but create long
  pages and less usable navigation.

Docs-build in CI:

- Add `docs-build` now as a local gate.
- Do not wire it into `just ci` unless the user explicitly wants docs build to
  become part of the Rust CI gate. This avoids making Node installation a
  requirement for every Rust-only check.
