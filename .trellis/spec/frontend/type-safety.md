# Type Safety

> Not applicable — this is a Rust project.

---

## Overview

This project is a Rust workspace with no TypeScript/JavaScript frontend.
TypeScript-specific patterns (Zod, io-ts, type guards, generics in JSX) are
irrelevant.

Rust's type system provides compile-time safety. The project uses:
- `thiserror` for error types
- `serde` derive for serialization
- Standard Rust enums and structs for domain modeling

See [Error Handling](../backend/error-handling.md) for Rust error type
conventions.

Do not attempt to match TypeScript type-safety conventions in this project —
they do not apply.
