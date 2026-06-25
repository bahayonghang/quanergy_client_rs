# Quality Guidelines

> Code quality standards relevant to the documentation frontend only.

---

## Overview

The only frontend code in this project is the VitePress documentation site
under `docs/`. Rust code quality rules live in
[Backend Quality Guidelines](../backend/quality-guidelines.md).

---

## Forbidden Patterns

- Do not add root `package.json` or root `package-lock.json` for documentation
  tooling unless the project deliberately introduces a root JavaScript workspace.
- Do not mix Rust source and Node tooling outside the `docs/` directory.

---

## Required Patterns

- Keep VitePress and Node metadata scoped to `docs/`.
- Run VitePress commands through the root `justfile` (`just docs`,
  `just docs-build`, `just docs-preview`).
- Keep bilingual (en/zh) pages semantically equivalent.

---

## Code Review Checklist

- [ ] No root-level Node artifacts introduced.
- [ ] English and Chinese pages are semantically equivalent.
- [ ] `just docs-build` succeeds.
