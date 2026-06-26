# State Management

> Not applicable — this is a Rust project.

---

## Overview

This project is a Rust workspace with no web frontend. State management
patterns (Redux, Zustand, React Query, etc.) are irrelevant.

Rust state management in this project follows standard Rust patterns:
owned types, `&mut self` methods on structs, and bounded channels for
pipeline stages. See [Quality Guidelines](../backend/quality-guidelines.md)
for Rust-specific patterns.

Do not attempt to match JavaScript state management conventions in this
project — they do not apply.
