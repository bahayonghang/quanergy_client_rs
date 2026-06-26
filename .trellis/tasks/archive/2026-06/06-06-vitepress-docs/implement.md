# VitePress Docs Implementation Plan

## Checklist

1. Refresh task context
   - Read `prd.md`, `design.md`, project `AGENTS.md`, and relevant Trellis spec
     indexes.
   - Verify current git status.

2. Add VitePress package metadata
   - Create `docs/package.json` with docs dev/build/preview scripts.
   - Install VitePress as a dev dependency to create a lockfile.
   - Verify Node/npm commands are available and compatible with VitePress.

3. Add VitePress source tree
   - Create English pages:
     `docs/index.md`, `docs/architecture.md`, `docs/implementation.md`,
     `docs/visualizer.md`, and `docs/capture-store.md`.
   - Create Chinese pages:
     `docs/zh/index.md`, `docs/zh/architecture.md`,
     `docs/zh/implementation.md`, `docs/zh/visualizer.md`, and
     `docs/zh/capture-store.md`.
   - Create `docs/.vitepress/config.mts` with VitePress locale routing:
     English at `/` and Chinese under `/zh/`.

4. Document current code accurately
   - Pull command examples from `apps/visualizer/src/lib.rs`.
   - Pull capture-store command examples from `apps/capture-store/src/main.rs`.
   - Pull SDK/module descriptions from `crates/quanergy-client/src/lib.rs` and
     nearby modules.
   - Mark later measurement features as future work, not implemented behavior.
   - Keep English and Chinese pages semantically equivalent, while allowing
     natural wording rather than literal line-by-line translation.

5. Add justfile integration
   - Add `docs`, `docs-build`, and `docs-preview` recipes.
   - Preserve existing Rust recipes unchanged.

6. Add generated-output ignores
   - Add `docs/node_modules/` and `docs/.vitepress/dist/` to `.gitignore`.
   - Do not ignore source files under `docs/`.

7. Verify
   - `rtk npm --prefix docs run docs:build`
   - `rtk just --list`
   - `rtk just docs-build`
   - `rtk just ci`

8. Review final diff
   - Confirm docs examples match current CLI flags.
   - Confirm English and Chinese page sets cover the same topics.
   - Confirm English pages build at `/` and Chinese pages build under `/zh/`.
   - Confirm no Rust behavior changed.
   - Confirm generated `docs/.vitepress/dist/` is not tracked.

## Risky Files

- `justfile`: changing existing recipes could break local Rust workflows.
- `.gitignore`: broad ignore patterns could hide source docs accidentally.
- `docs/package-lock.json`: generated dependency metadata should be reviewed
  for expected package-manager shape.

## Rollback Points

- If Node/npm is unavailable, keep planning artifacts and stop before adding
  partial package metadata.
- If VitePress build fails because of version/runtime mismatch, adjust the
  VitePress version or report the runtime requirement before touching Rust code.
- If docs examples become uncertain, rerun the relevant CLI help commands and
  update docs from observed output.

## Review Gate Before Start

Implementation should not start until the user approves the planning artifacts
or explicitly asks to proceed.
