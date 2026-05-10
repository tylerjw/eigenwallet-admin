# eigenwallet-admin developer recipes.
# Run with: `just <recipe>`. Use `just` (no args) to list.

set shell := ["bash", "-uc"]
set dotenv-load := true

default:
    @just --list --unsorted

# ---- toolchain & dep hygiene ----------------------------------------------

# Update rustup & install latest stable plus the wasm target.
toolchain:
    rustup self update || true
    rustup update stable
    rustup target add wasm32-unknown-unknown
    rustup component add rustfmt clippy

# Show outdated / available upgrades without writing.
upgrade-check:
    cargo upgrade --dry-run

# Bump deps to the latest compatible (within semver) and refresh the lockfile.
upgrade:
    cargo upgrade
    cargo update

# Bump deps including incompatible (major) bumps. Review the diff after.
upgrade-major:
    cargo upgrade --incompatible
    cargo update

# Security audit of the lockfile.
audit:
    cargo audit

# ---- format / lint --------------------------------------------------------

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

# Deny-on-warnings clippy across all targets and both feature sets.
lint:
    cargo clippy --all-targets --features ssr -- -D warnings
    cargo clippy --target wasm32-unknown-unknown --features hydrate --no-default-features --lib -- -D warnings

# Format-check + lint + audit. CI's gate.
check: fmt-check lint audit

# ---- build / test ---------------------------------------------------------

# Fast type-check across both feature sets.
typecheck:
    cargo check --features ssr
    cargo check --target wasm32-unknown-unknown --features hydrate --no-default-features --lib

# Full release build via cargo-leptos (server + wasm bundle).
build:
    cargo leptos build --release

# Run all unit + integration tests.
test:
    cargo test --features ssr

# Live-reload dev server. Requires cargo-leptos + a running DATABASE_URL.
dev:
    cargo leptos watch

# ---- container / k8s -----------------------------------------------------

# Build the production container image locally.
docker-build:
    docker build -t ghcr.io/tylerjw/eigenwallet-admin:dev .

# Kustomize-validate the homelab manifests against the API server (dry-run, no apply).
kube-validate path:
    kubectl kustomize {{path}} | kubectl apply --dry-run=server -f -

# ---- one-shot bootstrap for fresh machines -------------------------------

bootstrap: toolchain
    cargo install cargo-leptos cargo-edit cargo-audit just || true
