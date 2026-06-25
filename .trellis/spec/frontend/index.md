# Frontend Development Guidelines

> Best practices for frontend development in this project.

---

## Overview

This is a Rust workspace. There is no web application frontend. The only
frontend artifact is the VitePress documentation site under `docs/`.

Guidelines for web frameworks (React, Vue, Svelte, etc.) are marked N/A
because they do not apply to this project. Sub-agents should not attempt to
match web frontend conventions here.

---

## Guidelines Index

| Guide | Description | Status |
|-------|-------------|--------|
| [Directory Structure](./directory-structure.md) | VitePress docs site layout | Filled |
| [Component Guidelines](./component-guidelines.md) | N/A — Rust project, no web components | Filled (N/A) |
| [Hook Guidelines](./hook-guidelines.md) | N/A — Rust project, no React hooks | Filled (N/A) |
| [State Management](./state-management.md) | N/A — Rust project, no JS state | Filled (N/A) |
| [Quality Guidelines](./quality-guidelines.md) | VitePress docs quality rules | Filled |
| [Type Safety](./type-safety.md) | N/A — Rust project, no TypeScript | Filled (N/A) |

---

## How to Fill These Guidelines

For each guideline file:

1. Document your project's **actual conventions** (not ideals)
2. Include **code examples** from your codebase
3. List **forbidden patterns** and why
4. Add **common mistakes** your team has made

The goal is to help AI assistants and new team members understand how YOUR project works.

---

**Language**: All documentation should be written in **English**.
