# eigenwallet-admin — agent notes

Web admin console for the eigenwallet ASB market maker. Single-binary Rust app:
axum + Leptos 0.8 SSR with hydration; diesel + diesel-async to a CNPG Postgres;
jsonrpsee to asb's RPC; kube 3.x for in-cluster control. See `PLAN.md` for the
full architecture.

## Use the justfile, not raw cargo

All routine tasks have a recipe — prefer `just <recipe>` over ad-hoc cargo
invocations so the cross-target flags (`--features ssr` vs the wasm hydrate
build) stay consistent.

| Task                                     | Command            |
| ---------------------------------------- | ------------------ |
| List recipes                             | `just`             |
| Format                                   | `just fmt`         |
| Lint (deny warnings, both feature sets)  | `just lint`        |
| Full pre-commit gate                     | `just check`       |
| Type-check (fast iteration)              | `just typecheck`   |
| Tests                                    | `just test`        |
| Full release build (server + wasm)       | `just build`       |
| Dev server with hot reload               | `just dev`         |
| Bump deps within semver                  | `just upgrade`     |
| Bump across major versions               | `just upgrade-major` |
| `cargo audit`                            | `just audit`       |
| Update toolchain                         | `just toolchain`   |
| Build container image                    | `just docker-build` |
| Kustomize-validate manifests             | `just kube-validate apps/eigenwallet-admin` |

After **any** dep change or before commit, run `just check`. CI runs the same.

## Compile model

Two compile targets share `src/lib.rs`:

- **`ssr`** (default) — server binary, runs migrations, owns the DB pool,
  hosts Leptos SSR and all REST endpoints, drives background pollers.
- **`hydrate`** — `wasm32-unknown-unknown` cdylib that runs in the browser
  and hydrates interactive islands.

Cargo features gate the imports cleanly: anything under `src/server/` and
the `crate::server` module is `#[cfg(feature = "ssr")]`-only.

## REST API: server functions, not axum routes

All `/api/*` endpoints are Leptos server functions (`#[server]` blocks)
embedded in the page module where they're consumed. **Do not** add parallel
axum routes for the same data — server functions are mounted automatically
under `/api/<name>` and work on both SSR (direct call) and hydrate (HTTP).

The thin axum layer in `src/server/mod.rs` exists only to:
1. Host the `LeptosRoutes` integration.
2. Apply the session middleware.
3. Apply the auth-gate (`auth_gate`) that redirects unauth'd page requests
   to `/login` and returns 401 for unauth'd `/api/*` calls.

## Database

- diesel + diesel-async with bb8. Sync `PgConnection` is used **only** at
  startup to apply embedded migrations; everything else is async.
- `src/server/schema.rs` is hand-edited to match `migrations/`; regenerate
  with `diesel print-schema` if a real DB is available.
- The CNPG cluster name is `admin-db`; rw service is `admin-db-rw`.
- Always use `rust_decimal::Decimal` for money columns. Never `f64`.

## Versions

- Rust 1.95 (pinned via `rust-toolchain.toml`).
- Edition 2024.
- Latest stable for every dep — `just upgrade` keeps it that way.

## Don't

- Don't add an `/api/*` axum route for something a server function already
  covers.
- Don't import `chrono` types into kube-touching code unless necessary —
  kube 3.x uses `jiff` for metadata timestamps; convert at boundaries.
- Don't use `f64` for BTC/XMR/USD math.
- Don't introduce a JWT path; sessions are the single auth source.
- Don't query the asb sqlite directly; the swap-mirror reads tracing logs.
