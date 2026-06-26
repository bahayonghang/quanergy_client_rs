# Directory Structure

> How frontend-style documentation code is organized in this project.

---

## Overview

This project currently has a documentation frontend, not an application
frontend. Keep VitePress source and Node package metadata scoped to `docs/` so
the Rust workspace root stays Rust-only.

## Directory Layout

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

Generated files stay ignored:

```text
docs/node_modules/
docs/.vitepress/cache/
docs/.vitepress/dist/
```

## Module Organization

VitePress commands are exposed from the root `justfile`, but they run through
the docs package:

```just
docs:
    npm --prefix docs run docs:dev

docs-build:
    npm --prefix docs run docs:build

docs-preview:
    npm --prefix docs run docs:preview
```

Do not add root `package.json` or root `package-lock.json` for documentation
tooling unless the project deliberately introduces a root JavaScript workspace.

## Naming Conventions

Use VitePress locale routing for bilingual documentation:

- English pages live directly under `docs/` and build at `/`.
- Chinese pages live under `docs/zh/` and build at `/zh/`.
- Keep paired English and Chinese pages semantically equivalent, while allowing
  natural wording instead of line-by-line translation.

## Examples

The current documentation source under `docs/` is the canonical example for
this convention.
