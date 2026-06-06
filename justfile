set shell := ["powershell.exe", "-NoProfile", "-Command"]

default:
    @just --list

ci:
    cargo fmt --all -- --check
    cargo clippy --all-targets --all-features -- -D warnings
    cargo test --all-targets --all-features

dev:
    cargo check --all-targets --all-features
    cargo test --all-targets --all-features

docs:
    npm --prefix docs run docs:dev

docs-build:
    npm --prefix docs run docs:build

docs-preview:
    npm --prefix docs run docs:preview

build:
    cargo build

release:
    cargo build --release

test:
    cargo test --all-targets --all-features

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

clippy:
    cargo clippy --all-targets --all-features -- -D warnings

check:
    cargo check --all-targets --all-features

clean:
    cargo clean
