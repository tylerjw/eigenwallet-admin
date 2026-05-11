# Multi-stage build for the eigenwallet-admin server.
#
# Stage 1 (chef): cargo-chef base, lets stage 2 cache the dep graph independent
# of source changes.
# Stage 2 (builder): cargo-leptos build --release produces target/server/release
# (the SSR server binary) and target/site/ (the wasm bundle + tailwind CSS).
# Stage 3 (runtime): debian-slim, runs the unprivileged binary on port 4000.

FROM lukemathwalker/cargo-chef:0.1.77-rust-1.95-trixie AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder

# System deps for diesel (libpq), reqwest (libssl, ca-certs), and wasm tooling.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        libpq-dev pkg-config libssl-dev curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add wasm32-unknown-unknown

# cargo-leptos pulls cargo-chef-style dep caching for *both* server and wasm.
# cargo-leptos 0.3+ downloads a matching `wasm-bindgen-cli` at runtime rather
# than bundling it, which avoids the schema-version mismatch we hit on 0.2.47
# (bundled wasm-bindgen 0.2.105 vs project 0.2.121).
RUN cargo install --locked cargo-leptos --version ^0.3

COPY --from=planner /app/recipe.json recipe.json
# Cook the SSR deps. The wasm deps cache is built by cargo-leptos below.
RUN cargo chef cook --release --features ssr --recipe-path recipe.json

COPY . .
RUN cargo leptos build --release
# cargo-leptos only builds the bin-target (eigenwallet-admin). seed-admin needs
# a separate `cargo build` step; otherwise the runtime image ships a 320 KB
# cargo-chef empty-main stub.
RUN cargo build --release --features ssr --bin seed-admin

FROM debian:trixie-slim AS runtime

LABEL org.opencontainers.image.source="https://github.com/tylerjw/eigenwallet-admin"
LABEL org.opencontainers.image.title="eigenwallet-admin"
LABEL org.opencontainers.image.description="Admin console for the eigenwallet ASB market maker"

RUN apt-get update \
    && apt-get install -y --no-install-recommends libpq5 libssl3 ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --uid 1000 appuser

# Server binary
COPY --from=builder /app/target/release/eigenwallet-admin /usr/local/bin/
COPY --from=builder /app/target/release/seed-admin /usr/local/bin/
# Static + wasm bundle. The server reads LEPTOS_SITE_ROOT to find this.
COPY --from=builder /app/target/site /var/www/eigenwallet-admin

ENV LEPTOS_SITE_ROOT=/var/www/eigenwallet-admin \
    LEPTOS_OUTPUT_NAME=eigenwallet-admin \
    LEPTOS_SITE_ADDR=0.0.0.0:4000 \
    LEPTOS_SITE_PKG_DIR=pkg \
    RUST_LOG=info \
    PORT=4000

USER appuser
EXPOSE 4000

CMD ["eigenwallet-admin"]
