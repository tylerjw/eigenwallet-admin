# eigenwallet-admin: Design Plan

A web-based admin console / dashboard for the eigenwallet ASB (Atomic Swap Backend) BTC↔XMR market maker, deployed alongside the existing `eigenwallet` namespace in Tyler's homelab k8s cluster. Goal: replace `kubectl exec -it deploy/asb-controller -- ...` with a single Tailscale-only web UI that surfaces market position, system health, swap history, and operator actions including the dangerous "recycle" path.

---

## 1. Architecture overview

### High-level data flow

```
                          Tailscale (userspace ProxyClass)
                                       │
                                       ▼
                          ┌──────────────────────────────┐
                          │ eigenwallet-admin (web pod)  │
                          │  - axum HTTP + WebSocket     │
                          │  - Leptos SSR (one binary)   │
                          │  - background pollers        │
                          └─────────────┬────────────────┘
                                        │
        ┌───────────────────┬───────────┼────────────┬────────────────────┐
        ▼                   ▼           ▼            ▼                    ▼
  ┌───────────┐      ┌────────────┐  ┌──────┐  ┌──────────────┐    ┌──────────────┐
  │ asb       │      │ k8s API    │  │ CEX  │  │ swap-cli     │    │ Postgres     │
  │ JSON-RPC  │      │ (jobs,     │  │ APIs │  │ taker pods   │    │ (CNPG)       │
  │ :9944     │      │  pods,     │  │      │  │ (Jobs)       │    │              │
  │           │      │  configmap)│  │ ws + │  │              │    │ history,     │
  │           │      │            │  │ rest │  │ list-sellers,│    │ snapshots,   │
  │           │      │            │  │      │  │ buy-xmr      │    │ recycle log  │
  └───────────┘      └────────────┘  └──────┘  └──────────────┘    └──────────────┘
```

### What talks to what

- **Browser ↔ admin pod**: HTTPS over Tailscale (`tailscale.com` ingress class, `tailscale.com/proxy-class: userspace`). HTML rendered server-side by Leptos SSR; WebSocket upgrade for live tiles (balances, swap state, peer count).
- **admin ↔ asb JSON-RPC**: in-cluster HTTP to `http://asb.eigenwallet.svc:9944`. Source of truth for live state — no caching of swap state in our DB.
- **admin ↔ k8s API**: uses an in-cluster ServiceAccount with a Role limited to the `eigenwallet` namespace. Two purposes: (a) patch the `asb-config` ConfigMap + bump the `config-version` annotation on the asb Deployment to trigger restart; (b) create Jobs for `swap list-sellers` scans and recycle operations.
- **admin ↔ CEX**: `reqwest` polls Kraken & KuCoin REST tickers (no WS needed in v1). Same data sources as the existing `analyze-competitors.py` for continuity.
- **admin ↔ swap-cli Jobs**: admin creates a `batch/v1 Job` with the `ghcr.io/tylerjw/eigenwallet-swap-cli:4.5.0` image. Job writes JSON to a PVC (or stdout that the admin streams from k8s logs API).
- **admin ↔ postgres**: diesel-async to `eigenwallet-admin-db-rw:5432` (CNPG cluster in same namespace). Stores: snapshots over time for charts, recycle audit log, deposit/withdrawal events for cost basis, cached competitor scan results, ROI checkpoints.

### Pod composition

Single deployment, single container, single binary:

- **eigenwallet-admin** (Rust binary, Leptos SSR + axum, port 4000)

No separate frontend image. No nginx. The same binary serves the HTML shell, hydrates Leptos islands on the client, and exposes JSON endpoints + WS for both the SSR side and any direct AJAX. This is the same pattern Tyler already runs in `tarot-backend` minus the separate Flutter web image.

---

## 2. Tech stack decisions

### Backend: axum 0.8, diesel-async, bb8, jsonrpsee, reqwest, tower-sessions

Match Tyler's existing `tarot-backend` Cargo.toml verbatim where applicable. The dependency set is already vetted in production. Notable additions for this project:

- **`jsonrpsee` 0.24** (`http-client` feature) — typed JSON-RPC client to asb. Cleaner than hand-rolling reqwest calls and supports request batching when we need to fan out (e.g., status dashboard fires 6–8 RPCs in parallel).
- **`kube` 0.96 + `k8s-openapi` v1_32** — for ConfigMap patches, Job creation, pod listings. In-cluster auth via the mounted ServiceAccount token, no kubeconfig.
- **`argon2` 0.5** — already in Tyler's stack; reuse for the single-user password hash.
- **`tower-sessions` 0.13 with `tower-sessions-cookie-store`** — server-signed cookie sessions. Single-user means no DB session table needed; a signed cookie with `{ authed: true, exp }` is sufficient. *Alternative considered: JWT in cookie like tarot does. Rejected because we don't need stateless verification across instances and tower-sessions has built-in CSRF rotation.*
- **`tracing` 0.1 + `tracing-subscriber`** — same as existing stack. JSON logs so they flow into Loki.
- **`rust_decimal`** — exact arithmetic for BTC/XMR/USD math; never use `f64` for money.
- **`time` 0.3** with `serde` — Tyler uses `chrono` in tarot but for new code with diesel, `chrono` is fine; stay consistent and use `chrono` here too.

Diesel vs sqlx: Tyler asked for diesel. Use **diesel + diesel-async + diesel_migrations** exactly as tarot-backend does. `embed_migrations!("migrations")` + sync `PgConnection` for the migration run at startup + async pool for everything else. The diesel `schema.rs` macro pattern is established and Tyler already understands it.

### Frontend: Leptos 0.7 (recommended)

Three Rust frontend options were evaluated against the constraints: solo developer, mobile-friendly, prefer Rust, SSR desired (snappy first paint matters for a dashboard).

**Leptos 0.7 — recommended.**
- Real SSR with selective hydration ("islands") shipped in 0.7. The `axum` integration is first-party; you mount Leptos as an axum `Router` and share state.
- Fine-grained reactivity via signals; no VDOM. This matches the dashboard workload well — many independent tiles that update on their own intervals.
- `leptos_meta` + `leptos_router` cover head tags + client-side routing.
- Active development, the maintainer (Greg Johnston) is responsive, the book is decent. Ecosystem is the largest of the Rust frontend frameworks today.
- Mobile-friendly is a CSS problem, not a framework problem; Leptos doesn't constrain it. Pair with **Tailwind CSS** via `tailwindcss-cli` in the Dockerfile (Tyler hasn't used Tailwind in tarot but it's the lowest-overhead mobile-first option for a solo dev).
- Compile times: with `cargo-leptos`, watch mode is workable but cold builds are slow. Mitigated by `cargo-chef` in the Dockerfile (already used in `backend.Dockerfile`).

**Dioxus 0.6 — close second.**
- DX is genuinely excellent — hot reload is smoother than Leptos's, and the `rsx!` macro is more familiar to React refugees.
- SSR exists but is less mature than Leptos's; the "fullstack" story (Dioxus's term) was rewritten in 0.5 and is still settling.
- Routing is fine, but the ecosystem around forms/tables is thinner.
- Reject only because SSR is the deciding factor here. If this were a SPA-only console, Dioxus would win.

**Yew 0.21 — not recommended.**
- VDOM-based, no SSR worth shipping (their SSR is experimental and not commonly used in prod).
- Slower iteration; the project's release cadence has fallen behind Leptos/Dioxus.
- Skip.

**TypeScript + React + Vite — fallback we are not taking.**
- Faster initial dev velocity for someone fluent in React. The library ecosystem (TanStack Query, Recharts, shadcn/ui) is unmatched.
- Cost: a second container, a second image build pipeline, CORS configuration, duplicated TypeScript types of all Rust DTOs (or `ts-rs`/`specta` to generate them), and a JS toolchain in the homelab CI.
- For a single-user homelab tool where the operator is Rust-fluent, the single-binary Leptos path is cleaner long-term even though week-1 velocity is slower.

**Charts.** Leptos doesn't have a first-class chart library. Three options:
1. **uPlot via wasm-bindgen wrapper** — tiny (~40 KB), fast, ugly default but tweakable. Best for time-series.
2. **Plotly.js via leptos_plotly or a thin wrapper** — heavy (~3 MB), but Plotly is what `market-observer/` already uses. Reusing chart styling is plausible.
3. **Hand-rolled SVG in Leptos** — fine for 1-2 charts, becomes a chore beyond that.

Recommend uPlot for the account-value-over-time and volume charts, and hand-rolled SVG sparklines inline in tiles. Skip Plotly.

### Crate summary

| Concern | Crate |
|---|---|
| HTTP server | axum 0.8 |
| Frontend | leptos 0.7 + leptos_axum 0.7 + leptos_meta + leptos_router |
| Styling | tailwindcss (via cargo-leptos asset pipeline) |
| DB pool | diesel-async 0.8 + bb8 |
| Migrations | diesel_migrations 2.3 |
| Sessions | tower-sessions 0.13 |
| Password hashing | argon2 0.5 |
| JSON-RPC client | jsonrpsee 0.24 (http-client) |
| HTTP client (CEX) | reqwest 0.13 |
| k8s client | kube 0.96 + k8s-openapi |
| Money math | rust_decimal 1 |
| Logging | tracing + tracing-subscriber (JSON) |
| Rate limiting | tower_governor (already vetted) |
| Charts (client) | uPlot via wasm bindings |

---

## 3. Feature-by-feature breakdown

### 3.1 Market position

**Data source.** Three inputs joined in memory: (1) our current quote from asb `get-current-quote`; (2) competitor quotes from the last `swap list-sellers` scan (cached in DB, refreshed every 10 minutes); (3) CEX mid from cached Kraken+KuCoin tickers (15 s refresh).

**Wire API.** `GET /api/market/position` returns:
```json
{
  "our_quote": { "price_btc": "0.00524", "spread_pct": 3.91, "min": "0.001", "max": "0.05" },
  "cex_mid": { "btc_xmr": "0.00504", "source": "kraken+kucoin", "as_of": "..." },
  "rank": { "by_spread": 5, "by_price": 5, "out_of": 9, "cheapest_competitor_pct": 1.60 },
  "trend_30m": [ /* spread % samples */ ]
}
```

**UI sketch.** Single tile at top of dashboard. Big number = our spread vs CEX mid. Subtext = rank ("5th of 9 active makers"). Sparkline of last 30 minutes of our spread. Color: green if top 3 by price, yellow if 4-6, red if 7+.

**MVP vs polish.** MVP: tile + sparkline. Polish: per-competitor breakdown drawer; toggle "show only those who serve my size range".

**Gotchas.** `list-sellers` is expensive (~30-60 s rendezvous round trips, several MB of P2P chatter). Cache scan results aggressively. Don't re-scan from the position tile — it just reads cached data.

### 3.2 Spread control

**Data source.** ConfigMap `asb-config` in the `eigenwallet` namespace; `[maker]` section is read by asb at startup only.

**Wire API.**
- `GET /api/maker/config` — read current ConfigMap, parse `[maker]` section.
- `PUT /api/maker/config` — body is the new `[maker]` block. Server validates ranges, writes the new TOML to the ConfigMap, then patches the asb Deployment `spec.template.metadata.annotations["config-version"]` to a new value (timestamp) to force a rolling restart. Returns a job ID for the operator to poll.
- `GET /api/maker/config/restart-status?id=...` — polls Deployment `status.observedGeneration` + ready replicas to confirm the new pod is up. WebSocket variant for live updates.

**UI sketch.** Form with 5 fields: `min_buy_btc`, `max_buy_btc`, `ask_spread`, `developer_tip`, `anti_spam_deposit_ratio`. Each shows the current value with a slider/number input. Bottom of form: "Save and restart asb" button → confirmation dialog showing the diff in TOML form. After submit, a progress bar tracks "ConfigMap updated → Old pod terminating → New pod Ready → asb RPC reachable".

**MVP vs polish.** MVP: edit the 5 fields, restart, wait for ready. Polish: history of past `[maker]` configs (audit table), one-click "revert to previous", warnings if `max_buy_btc` is being raised (operator policy: don't suggest raising it, but still allow it explicitly).

**Gotchas.**
- The asb Deployment uses `strategy: Recreate` with a single replica — restart will cause ~30-60 s downtime during which the maker is offline.
- A failed restart (asb crash-looping on bad config) needs a recovery path. The "Save" button should snapshot the previous ConfigMap content into a Postgres row before patching; revert UI must work even if asb is down (it talks to k8s, not asb).
- The whole TOML file is owned by us — preserve everything else (non-`[maker]` sections) verbatim. Use a TOML round-tripping crate (`toml_edit`) not the lossy `toml` serializer.

### 3.3 Charts (account value, volume, swaps/day)

**Data source.** Postgres `balance_snapshots` table populated by a background task every 5 minutes. Each row: `(taken_at, btc_wallet_sat, xmr_wallet_atomic, btc_usd_at_snapshot, xmr_usd_at_snapshot, total_usd_at_snapshot, total_btc_equivalent)`. The poller pulls balances via `bitcoin-balance` and `monero-balance` JSON-RPC calls, then multiplies by cached CEX prices.

For volume + swaps/day: derived from the `swaps` table (see §4) by aggregating completed swaps in a day's bucket.

**Wire API.**
- `GET /api/charts/account-value?period=24h|7d|30d|90d|all&denom=usd|btc` — returns a time series.
- `GET /api/charts/volume?period=...` — daily buckets.
- `GET /api/charts/swap-count?period=...` — daily count + per-state breakdown (completed / refunded / punished / pending).

**UI sketch.** Three stacked time-series charts on the Overview page. Toggle: "Denominate in USD / BTC". Hover shows exact value + timestamp. Default period 7d.

**MVP vs polish.** MVP: account value chart only, USD-denominated, 7d/30d/90d periods. Polish: BTC denomination toggle, volume + swap-count charts, log scale toggle, downloadable CSV.

**Gotchas.**
- Cold-start problem: there's no history before the admin pod starts. First days will be sparse. Acceptable.
- USD prices need a source. Reuse the CEX poller (Kraken `XBTUSD` and `XMRUSD` if available; else cross-rate via XBTXMR + XBTUSD). Snapshot the exact USD price used in each row so historical charts don't shift when CEX prices later change.

### 3.4 Swaps table

**Data source.** asb's `get-swaps` returns currently-active swaps only (operator confirmed in the migration memo). For full history, two complementary sources:

1. **JSON tracing logs** at `/asb-data/logs/tracing-*.log`. Already tailed by the tracing sidecar.
2. **Backfill from the asb SQLite DB** at `/asb-data/sqlite` if asb exposes it via RPC (it doesn't directly; the data is internal). 

Best path: parse the tracing logs at startup to build the history table, then maintain it going forward by polling `get-swaps` for active swaps and watching log lines for state transitions.

For v1, simpler approach: **mount the asb-data PVC read-only into the admin pod** and parse the tracing log directly into a `swaps` table at startup + incrementally tail. Lower-risk than scraping the SQLite directly.

**Wire API.**
- `GET /api/swaps?state=all|active|completed|refunded|punished&limit=50&offset=0&sort=...`
- `GET /api/swaps/:swap_id` — full detail including computed profit.

**UI sketch.** Table with columns: state pill, counterparty peer ID (truncated), BTC amount, XMR amount, BTC/XMR price, computed profit (BTC + USD), start time, duration. Pagination + state filter dropdown. Click row → detail drawer.

Computed profit per swap: `(xmr_received * spot_xmr_usd_at_completion) - (btc_paid * spot_btc_usd_at_completion)` for a buy, sign flipped for a sale. Plus fee components: BTC tx fee, XMR tx fee, our spread captured. The asb side does *sell BTC for XMR* (we receive BTC from taker, send them XMR), so a successful swap = we gained BTC, lost XMR; profit is in the spread.

**MVP vs polish.** MVP: read-only table, single-line rows, the basic filters. Polish: detail drawer with full state-transition timeline; export CSV; search by peer ID; profit aggregation footer.

**Gotchas.** Profit computation assumes you know cost basis. That's hard mid-stream; document the methodology clearly (mark-to-market at swap completion vs FIFO inventory accounting). See open question §10.

### 3.5 Ongoing activity

**Data source.** asb `get-swaps` (live RPC, polled every 5 s when at least one user has the dashboard open; every 60 s otherwise). For timelock proximity: each in-flight swap has `cancel_timelock_at_block` and we know current Bitcoin block height (asb exposes it indirectly; alternatively, query electrs `blockchain.headers.subscribe`).

**Wire API.** WebSocket `/ws/activity` pushes incremental updates. `GET /api/activity` for initial state.

**UI sketch.** Top-of-dashboard banner that only appears when at least one swap is in a non-terminal state. Shows per-swap: state, counterparty, age, blocks-until-timelock. Red blinking border if any swap is within 6 blocks of cancel timelock.

**MVP vs polish.** MVP: banner + simple list. Polish: notification sound or browser push on near-timelock events, click-through to swap detail drawer.

**Gotchas.** Computing "blocks until timelock" requires knowing the current block height. asb may report this; if not, electrs query needed. Don't underestimate timelock = swap gets refunded (we lose the lock fee or worse).

### 3.6 Rate of return

**Data source.** Two new Postgres tables:
- `capital_events` — manual ledger entries: deposits and withdrawals. Operator marks "I deposited 0.5 BTC on 2026-04-01" and "I withdrew 0.1 BTC on 2026-04-15 to cold storage". Each entry has `direction (deposit|withdraw)`, asset, amount, usd_value_at_event (optional override), notes.
- `balance_snapshots` — same as §3.3.

ROI is computed at request time from these two tables. The principle: **time-weighted return on capital actually committed**. Tyler operator wants "started with X, are now at Y, that's Z% over N days" — this needs a baseline date and a current-mark.

Two methodologies to support; pick one as default:
- **Mark-to-market (simple)**: total USD value now / total USD value at "start date" - 1.
- **TWR (time-weighted return)**: chained returns across sub-periods between capital events. Standard for portfolios with deposits/withdrawals.

**Wire API.**
- `GET /api/roi?since=YYYY-MM-DD&method=mtm|twr&denom=usd|btc`
- `POST /api/capital-events` — operator records a deposit/withdrawal.
- `GET /api/capital-events`

**UI sketch.** Card on Overview: big % number, subtitle "since 2026-04-15 (N days)". Settings page to record capital events.

**MVP vs polish.** MVP: mark-to-market only, USD denomination. Polish: TWR, BTC denomination, ROI per recycle (annotate the chart with each recycle event).

**Gotchas.** Cost-basis tracking is genuinely hard if you want it right (FIFO/LIFO/specID per coin). v1 punts: operator manually records "I added X of cost basis Y on date Z" and the system marks-to-market. See open question §10.

### 3.7 System health dashboard

**Data source.** Mix of k8s API and direct probes.
- **monerod sync**: `monerod` exposes `get_info` JSON-RPC on port 18081. Returns `height`, `target_height`, `synchronized`. Direct call from admin pod.
- **bitcoind sync**: `bitcoin-cli getblockchaininfo` — but we don't have bitcoin-cli in the admin pod. Instead, `bitcoind` accepts JSON-RPC; call `getblockchaininfo` directly with reqwest (cookie auth or rpcuser/rpcpass — check the bitcoind manifest for what's configured).
- **electrs index height**: electrs serves Electrum protocol on tcp/50001. Use the `blockchain.headers.subscribe` Electrum RPC. There's no production-grade Rust electrum client; either hand-roll a tiny one (the protocol is line-delimited JSON) or use `electrum-client` crate (synchronous, but fine in a tokio `spawn_blocking`).
- **Tor onion reachability**: asb-controller's `onion-service-status` and `multiaddresses` already report this. Use asb RPC.
- **Peer count**: asb-controller's `active-connections`.
- **Rendezvous registration**: asb-controller's `registration-status`. Operator noted "4/8 currently" — that's the visible metric.
- **Per-service Ready states**: k8s API `GET /api/v1/namespaces/eigenwallet/pods` filtered by labels.

**Wire API.** `GET /api/health` returns a single object with each subsystem's state. WebSocket `/ws/health` pushes updates.

**UI sketch.** Grid of small status cards: monerod (height N / N), bitcoind (height N / N), electrs (height N), tor (onion reachable: yes/no), peers (N connected), rendezvous (4/8 registered), asb pod (Running), bitcoind pod (Running), etc. Red border on any service that's degraded.

**MVP vs polish.** MVP: 8 tiles, all the above. Polish: drill-in pages per service with logs from k8s API.

**Gotchas.** bitcoind RPC needs credentials. Read them from a Secret mounted into the admin pod. Don't hard-code.

### 3.8 Taker market state

**Data source.** `swap list-sellers` from the `swap-cli` image (the fork-patched binary). Triggered on a schedule (every 10 min) by a CronJob or by a background task in the admin pod that creates a one-shot k8s Job.

The scan output is parsed into a `competitor_scans` table:
```sql
competitor_scans (
  scan_id uuid pk,
  scan_started_at timestamptz,
  scan_completed_at timestamptz,
  trigger text  -- 'cron'|'manual'|'recycle_prep'
);
competitor_quotes (
  scan_id uuid fk,
  peer_id text,
  price_btc_per_xmr numeric,
  min_btc numeric,
  max_btc numeric,
  status text  -- 'reachable'|'unreachable'|'spam'
);
```

CEX reference prices are cached separately in `cex_prices`.

**Wire API.**
- `GET /api/competitors/latest` — latest completed scan.
- `GET /api/competitors/history?period=...`
- `POST /api/competitors/scan` — kick off a manual scan now (returns scan_id, then poll/WS).

**UI sketch.** Page showing a sortable table of competitors (peer id, quote, min/max, distance from CEX mid in %). Highlight our row. Distribution histogram below: shows where each competitor falls on the spread axis, our position marked with a vertical line.

**MVP vs polish.** MVP: latest scan only, sortable table, our row highlighted. Polish: historical view (replay how the field moved over the day), competitor stickiness (which peers we see consistently), correlation between our spread and capture rate.

**Gotchas.**
- The fork's `list-sellers` may need rendezvous addresses passed as args; bake them into the Job manifest from the same source-of-truth list that's in `asb.yaml`'s `rendezvous_point` array. Make this a shared ConfigMap if possible to avoid drift.
- A scan can take 60+ seconds and can fail partially. The Job should write partial results progressively if possible; the admin pod tails the Job's pod logs and parses incrementally.
- The MY_PEER filter (currently hardcoded in an external `analyze-competitors.py` to a specific peer ID) needs to be derived from `asb-controller peer-id` at admin pod startup, not hard-coded. The fork notes "if rekeyed, update there" — automate this away.

### 3.9 Spread recommendation indicator

**Data source.** Latest `competitor_scans` + our current `[maker].ask_spread`.

Logic (v1, simple):
1. From the latest scan, build a sorted list of competitor spreads against CEX mid.
2. Compute the "tier 1" cutoff: the spread of the Nth cheapest competitor where N is configurable (default 3 — being top-3 is the goal).
3. If our spread > tier-1 cutoff: recommendation = "tighten to <tier-1 cutoff - 0.1%> to enter tier 1".
4. If our spread <= tier-1 cutoff but > tier-1 cutoff - 0.5%: recommendation = "stay where you are".
5. If our spread is much tighter than needed (e.g., we're cheapest by >1% margin): recommendation = "widen to <tier-1 cutoff - 0.1%> to capture more margin without losing position".

Crucially, **show the recommendation but do not apply it automatically.** Dynamic pricing is explicitly deferred per the migration memo.

**Wire API.** `GET /api/spread/recommendation` returns `{ current_spread, recommended_spread, reasoning, tier_1_cutoff, our_rank }`.

**UI sketch.** Small advisory card on the dashboard. Single sentence: "You're at +3.91%, cheapest competitor at +1.60%. Consider widening to +2.00% to be #1 in your tier." Action button "Open spread control" jumps to §3.2.

**MVP vs polish.** MVP: heuristic above. Polish: backtest the heuristic against historical scans to validate it; surface confidence interval; consider min/max coverage (a competitor with `max_buy_btc = 0.001` shouldn't count as "in our tier" for someone with 0.05 max).

**Gotchas.** Per operator policy in the migration memo, the recommendation must never suggest raising `max_buy_btc`. Spread, yes; size knobs, no.

### 3.10 Recycle trigger (sensitive — real money)

**Data source / mechanism.** A recycle is: take some of our BTC inventory, run `swap buy-xmr --seller <peer-id-or-multiaddr> --change-address <our-asb-bitcoin-deposit>` against a chosen competitor to refill XMR inventory.

The execution path is a k8s Job using the `eigenwallet-swap-cli` image:
1. Operator picks a target competitor from the latest scan + an amount (constrained to that competitor's min/max).
2. Server computes an estimated outcome (XMR received, spread captured vs CEX mid, our resulting balance).
3. Confirmation dialog with full details.
4. On confirm, server creates a Job with the taker config mounted. Job runs `swap buy-xmr ...`. Output is parsed and progress is streamed back via WS to the admin UI.
5. After the Job completes, the swap is also visible in the swaps table (because the swap-cli's own state log + the eventual XMR receipt into our wallet) — record into a `recycle_events` table with the planned vs actual outcome.

**Wire API.**
- `POST /api/recycle/quote` — body `{ peer_id, btc_amount }`. Returns estimated XMR out, fees, projected balances. Stateless preview.
- `POST /api/recycle/execute` — body `{ peer_id, btc_amount, confirmation_token, dry_run: bool }`. The confirmation_token is the hash returned from `/quote`; refusing to execute if the token doesn't match the current quote prevents stale-confirmation accidents.
- `GET /api/recycle/active` — currently-running recycle Job if any (only one allowed at a time).
- `GET /api/recycle/history`

**UI sketch.** Dedicated "Recycle" page (not a button on the dashboard — too easy to misclick).
1. Step 1: select counterparty from a list of competitors (only those reachable in the latest scan).
2. Step 2: amount slider, constrained to that counterparty's range and our BTC balance.
3. Step 3: review pane — shows estimated XMR out, effective price, profit vs CEX mid, fees breakdown, your resulting balances, **a fresh quote refresh check** (re-fetches the seller's quote and re-renders).
4. Step 4: typed confirmation: the operator types the BTC amount exactly to enable the "Execute recycle" button.
5. Step 5: progress page with Job logs streaming, abort button (creates a `Job` deletion).

**Safety details — see §8 for the full set.**

**Gotchas.**
- Taker requires its own Bitcoin wallet to fund the spend. Use a dedicated taker wallet (separate PVC, separate hidden-service-less identity). Funding the taker wallet from asb's BTC inventory is its own ops step (out of scope for v1; recycle assumes the taker wallet is already funded).
- The change address parameter is critical — the change must go back to **our asb's Bitcoin deposit address** (`asb-controller bitcoin-seed`-derived). Hardcoding the wrong address sends change to a wallet we don't control.
- Concurrent recycles: refuse if another recycle is in flight. Single-row mutex via a Postgres advisory lock keyed on a const.

---

## 4. Data model

Postgres schema. Two principles:

- **asb is the source of truth for swap state and live balances.** Do not duplicate; only cache for chart/history purposes.
- **Anything historical that we generate ourselves (snapshots, scans, capital events, recycle outcomes) lives in our DB.**

```sql
-- Single-user auth. One row. Replace by `UPDATE` to rotate.
CREATE TABLE admin_credentials (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    password_hash text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    rotated_at timestamptz
);

-- Balance & price snapshots for charts. Written every 5 min.
CREATE TABLE balance_snapshots (
    taken_at timestamptz PRIMARY KEY,
    btc_sat bigint NOT NULL,
    xmr_atomic numeric(40,0) NOT NULL,
    btc_usd numeric(20,8) NOT NULL,    -- USD per 1 BTC at this time
    xmr_usd numeric(20,8) NOT NULL,
    total_usd numeric(20,8) NOT NULL,
    total_btc numeric(20,10) NOT NULL
);
CREATE INDEX idx_balance_snapshots_taken_at ON balance_snapshots (taken_at DESC);

-- Mirror of asb's swap history. Populated from tracing logs + ongoing get-swaps.
CREATE TABLE swaps (
    swap_id text PRIMARY KEY,             -- asb's swap UUID (string form)
    peer_id text NOT NULL,
    state text NOT NULL,                  -- 'completed'|'refunded'|'punished'|'in-progress'|...
    btc_sat bigint NOT NULL,
    xmr_atomic numeric(40,0) NOT NULL,
    started_at timestamptz NOT NULL,
    completed_at timestamptz,
    -- Spot prices at completion for profit math (NULL until completed).
    btc_usd_at_completion numeric(20,8),
    xmr_usd_at_completion numeric(20,8),
    profit_usd numeric(20,8),
    raw_log_excerpt jsonb                 -- last few state-transition entries for debugging
);
CREATE INDEX idx_swaps_started_at ON swaps (started_at DESC);
CREATE INDEX idx_swaps_state ON swaps (state);

-- Operator-recorded capital deposits/withdrawals for ROI math.
CREATE TABLE capital_events (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    occurred_at timestamptz NOT NULL,
    direction text NOT NULL CHECK (direction IN ('deposit','withdraw')),
    asset text NOT NULL CHECK (asset IN ('BTC','XMR')),
    amount_atomic numeric(40,0) NOT NULL,
    usd_value_at_event numeric(20,8),    -- optional manual override; else mark-to-snapshot
    notes text,
    created_at timestamptz NOT NULL DEFAULT now()
);

-- Cached CEX prices, used to backfill snapshots and for live tiles.
CREATE TABLE cex_prices (
    sampled_at timestamptz PRIMARY KEY,
    btc_usd numeric(20,8),
    xmr_usd numeric(20,8),
    btc_xmr numeric(20,10),
    sources text[]                       -- e.g. {'kraken','kucoin'}
);

-- Competitor scans (list-sellers output).
CREATE TABLE competitor_scans (
    scan_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    started_at timestamptz NOT NULL,
    completed_at timestamptz,
    trigger text NOT NULL CHECK (trigger IN ('cron','manual','recycle_prep')),
    raw_output text                     -- full stdout for debugging
);

CREATE TABLE competitor_quotes (
    id bigserial PRIMARY KEY,
    scan_id uuid NOT NULL REFERENCES competitor_scans(scan_id) ON DELETE CASCADE,
    peer_id text NOT NULL,
    multiaddr text,
    price_btc_per_xmr numeric(20,10),
    min_btc numeric(20,10),
    max_btc numeric(20,10),
    reachable boolean NOT NULL,
    reason_if_unreachable text
);
CREATE INDEX idx_competitor_quotes_scan ON competitor_quotes (scan_id);
CREATE INDEX idx_competitor_quotes_peer ON competitor_quotes (peer_id);

-- Recycle audit log.
CREATE TABLE recycle_events (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    initiated_at timestamptz NOT NULL DEFAULT now(),
    counterparty_peer_id text NOT NULL,
    btc_planned_sat bigint NOT NULL,
    estimated_xmr_atomic numeric(40,0),
    estimated_effective_price numeric(20,10),
    confirmation_token text NOT NULL,
    job_name text,                        -- k8s Job name
    status text NOT NULL CHECK (status IN ('pending','running','completed','failed','aborted')),
    completed_at timestamptz,
    actual_xmr_atomic numeric(40,0),
    actual_btc_sat bigint,
    failure_reason text,
    notes text
);
CREATE INDEX idx_recycle_initiated_at ON recycle_events (initiated_at DESC);

-- Audit trail of [maker] config changes.
CREATE TABLE maker_config_history (
    id bigserial PRIMARY KEY,
    changed_at timestamptz NOT NULL DEFAULT now(),
    previous_toml text NOT NULL,
    new_toml text NOT NULL,
    restart_observed_at timestamptz,      -- when the new asb pod became Ready
    notes text
);
```

What's derived on-the-fly vs cached:
- **Live** (asb RPC, never persisted): current balances, active swaps, current quote, peer count, registration status. The Overview tiles all read live.
- **Cached for history** (our DB): balance snapshots, completed swap rows, competitor scans, recycle outcomes, capital events.

---

## 5. External integrations

### 5.1 asb JSON-RPC client

`jsonrpsee::http_client::HttpClient` pointed at `http://asb.eigenwallet.svc:9944`. One client per `AppState`. Typed methods modeled on the asb-controller subcommand surface:

```rust
#[rpc(client)]
trait AsbRpc {
    #[method(name = "check_connection")]
    async fn check_connection(&self) -> RpcResult<...>;
    #[method(name = "bitcoin_balance")]
    async fn bitcoin_balance(&self) -> RpcResult<BalanceBtc>;
    #[method(name = "monero_balance")]
    async fn monero_balance(&self) -> RpcResult<BalanceXmr>;
    #[method(name = "get_swaps")]
    async fn get_swaps(&self) -> RpcResult<Vec<ActiveSwap>>;
    #[method(name = "registration_status")]
    async fn registration_status(&self) -> RpcResult<RegistrationStatus>;
    #[method(name = "get_current_quote")]
    async fn get_current_quote(&self) -> RpcResult<Quote>;
    #[method(name = "set_withhold_deposit")]
    async fn set_withhold_deposit(&self, on: bool) -> RpcResult<()>;
    // ... etc
}
```

**Important caveat:** the exact RPC method names need to be verified against asb 4.5.0's actual RPC spec — the asb-controller CLI names may not 1:1 match the underlying RPC method names. First implementation task is to dump the RPC schema (most jsonrpsee servers expose `rpc.discover` or similar) and pin the right names.

### 5.2 Polling vs subscribing

asb's RPC is request-response; no subscription protocol. Background pollers in the admin pod:

| Loop | Interval | What it does |
|---|---|---|
| balance_snapshot | 5 min | Pull btc+xmr balances, multiply by latest cex_prices, insert row |
| active_swaps | 5 s (when WS clients present), 60 s (idle) | Pull get-swaps, diff against DB, push WS updates |
| health | 10 s | Aggregate health endpoint, push to /ws/health |
| cex_prices | 15 s | Hit Kraken + KuCoin REST tickers, write row |
| competitor_scan | 10 min | Trigger list-sellers job (or skip if one ran recently) |
| swap_log_tail | continuous | Tail `/asb-data/logs/tracing*.log` (PVC mounted RO), update swaps rows |

Each loop is a `tokio::spawn` with its own interval. Failures log and continue; no global retry — they wake up on the next tick.

### 5.3 CEX reference prices

Match `analyze-competitors.py`'s approach. Two sources for redundancy:

- **Kraken**: `GET https://api.kraken.com/0/public/Ticker?pair=XBTUSD,XMRUSD,XBTXMR`
- **KuCoin**: `GET https://api.kucoin.com/api/v1/market/orderbook/level1?symbol=BTC-USDT` and `XMR-USDT`

Use the median of available sources for the mid. If only one source responds, use it but flag `sources: ['kraken']` for downstream visibility. Cache for 15 s.

### 5.4 list-sellers scans

Two implementation options:

**Option A: long-lived helper pod.** Deploy a `swap-cli` pod alongside asb-controller with `command: ["sleep", "infinity"]`, and `kubectl exec` into it from the admin pod via the k8s API (`pods/<name>/exec` subresource). Faster (no Job startup cost), simpler to log-tail. Downside: longer-running pod, idle resource usage.

**Option B: one-shot Jobs.** Create a `batch/v1 Job` per scan. Cleaner isolation, easier to surface failures. Downside: ~5-10 s of cold-start overhead per scan.

Recommend **Option B** for `list-sellers` (every 10 min is fine with the overhead) and **for recycles** (clear audit trail per Job). Option A could be added later as an optimization if scan latency matters.

The Job template:
```yaml
apiVersion: batch/v1
kind: Job
metadata:
  generateName: scan-
  namespace: eigenwallet
spec:
  ttlSecondsAfterFinished: 600
  backoffLimit: 0
  template:
    spec:
      restartPolicy: Never
      containers:
        - name: scan
          image: ghcr.io/tylerjw/eigenwallet-swap-cli:4.5.0
          args:
            - list-sellers
            - --rendezvous-point=<addr1>
            - --rendezvous-point=<addr2>
            # ... etc
            - --json
```

Admin pod creates the Job, polls its status until `succeeded`, then reads the pod logs and parses. Use `kube::Api::<Job>::create` and `Api::<Pod>::logs`.

### 5.5 Recycle execution

Same pattern, but the Job mounts the taker BTC wallet PVC and runs `swap buy-xmr ...`. The taker config is a separate ConfigMap (`taker-config`). The recycle Job:

```yaml
apiVersion: batch/v1
kind: Job
metadata:
  generateName: recycle-
spec:
  ttlSecondsAfterFinished: 86400          # keep for a day for log review
  backoffLimit: 0                         # never auto-retry a money move
  activeDeadlineSeconds: 7200             # hard kill after 2h
  template:
    spec:
      restartPolicy: Never
      containers:
        - name: recycle
          image: ghcr.io/tylerjw/eigenwallet-swap-cli:4.5.0
          args:
            - buy-xmr
            - --seller=<multiaddr>
            - --change-address=<our-asb-deposit-addr>
            - --json
          volumeMounts:
            - { name: taker-data, mountPath: /taker-data }
            - { name: taker-config, mountPath: /etc/swap, readOnly: true }
      volumes:
        - { name: taker-data, persistentVolumeClaim: { claimName: taker-data } }
        - { name: taker-config, configMap: { name: taker-config } }
```

The taker-data PVC needs to exist (new NFS PV alongside `asb-data`). Initial wallet funding is an operator step done out-of-band (send BTC to the taker's deposit address); v1 does not include a UI for that.

---

## 6. Deployment

### Directory layout

Following the `apps/tarot/` pattern, create `homelab/apps/eigenwallet-admin/`:

```
homelab/apps/eigenwallet-admin/
├── kustomization.yaml
├── namespace.yaml             # (or reuse eigenwallet/ namespace — see below)
├── postgres.yaml              # CNPG Cluster
├── backend.yaml               # Deployment + Service
├── rbac.yaml                  # ServiceAccount + Role + RoleBinding
├── ingress.yaml               # Tailscale Ingress
├── recycle-config.yaml        # taker ConfigMap + taker-data PV/PVC
└── backup.yaml                # daily pg_dump CronJob (optional)
```

Namespace decision: **reuse the existing `eigenwallet` namespace**. The admin console is part of the same operational unit; cross-namespace RBAC adds complexity for no benefit. Adjust kustomization at `homelab/apps/eigenwallet/kustomization.yaml` to include the new resources, or list them as a sibling kustomization included by the cluster Kustomization.

### Deployment manifest

Mirrors `tarot/backend.yaml`:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: admin
  namespace: eigenwallet
spec:
  replicas: 1
  strategy: { type: Recreate }
  selector: { matchLabels: { app: eigenwallet-admin } }
  template:
    metadata: { labels: { app: eigenwallet-admin } }
    spec:
      serviceAccountName: eigenwallet-admin
      containers:
        - name: admin
          image: ghcr.io/tylerjw/eigenwallet-admin:latest
          imagePullPolicy: Always
          env:
            - { name: PORT, value: "4000" }
            - { name: ASB_RPC_URL, value: "http://asb:9944" }
            - { name: BITCOIND_RPC_URL, value: "http://bitcoind:8332" }
            - { name: MONEROD_RPC_URL, value: "http://monerod:18081" }
            - { name: ELECTRS_URL, value: "tcp://electrs:50001" }
            - { name: SWAP_CLI_IMAGE, value: "ghcr.io/tylerjw/eigenwallet-swap-cli:4.5.0" }
            - { name: RUST_LOG, value: "info" }
            - name: DATABASE_URL
              value: "postgres://admin:$(POSTGRES_PASSWORD)@admin-db-rw:5432/admin"
            - name: POSTGRES_PASSWORD
              valueFrom: { secretKeyRef: { name: admin-db-credentials, key: password } }
            - name: SESSION_SECRET
              valueFrom: { secretKeyRef: { name: admin-session, key: session_secret } }
            - name: BITCOIND_RPC_AUTH
              valueFrom: { secretKeyRef: { name: bitcoind-rpc, key: rpcauth } }
          volumeMounts:
            - name: asb-data
              mountPath: /asb-data
              readOnly: true
          ports: [ { containerPort: 4000 } ]
          resources:
            requests: { memory: 128Mi, cpu: 50m }
            limits:   { memory: 512Mi }
      volumes:
        - name: asb-data
          persistentVolumeClaim:
            claimName: asb-data       # RWM — read-only mount alongside asb's rw mount
```

(RWM allows the existing asb-data PVC, already declared RWM in `storage.yaml`, to be mounted RO here for log tailing.)

### Postgres

Direct copy of `apps/tarot/postgres.yaml`, renamed:

```yaml
apiVersion: postgresql.cnpg.io/v1
kind: Cluster
metadata: { name: admin-db, namespace: eigenwallet }
spec:
  instances: 1
  bootstrap:
    initdb:
      database: admin
      owner: admin
      secret: { name: admin-db-credentials }
  storage:
    size: 2Gi
    storageClass: nfs-truenas
  resources:
    requests: { memory: 128Mi, cpu: 50m }
    limits: { memory: 256Mi }
```

### RBAC

```yaml
apiVersion: v1
kind: ServiceAccount
metadata: { name: eigenwallet-admin, namespace: eigenwallet }
---
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata: { name: eigenwallet-admin, namespace: eigenwallet }
rules:
  - apiGroups: [""]
    resources: ["configmaps"]
    verbs: ["get","list","watch","update","patch"]
  - apiGroups: [""]
    resources: ["pods","pods/log"]
    verbs: ["get","list","watch"]
  - apiGroups: ["apps"]
    resources: ["deployments"]
    verbs: ["get","list","watch","patch"]
  - apiGroups: ["batch"]
    resources: ["jobs"]
    verbs: ["get","list","watch","create","delete"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata: { name: eigenwallet-admin, namespace: eigenwallet }
roleRef: { apiGroup: rbac.authorization.k8s.io, kind: Role, name: eigenwallet-admin }
subjects: [ { kind: ServiceAccount, name: eigenwallet-admin, namespace: eigenwallet } ]
```

Note: no `secrets` or `nodes` access — the admin pod cannot read the operator-managed secrets through k8s API; it only consumes the ones explicitly mounted into its own pod spec.

### Tailscale ingress

Following `headlamp-ingress.yaml`:

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: eigenwallet-admin
  namespace: eigenwallet
  annotations:
    tailscale.com/funnel: "false"
    tailscale.com/proxy-class: userspace
spec:
  ingressClassName: tailscale
  defaultBackend:
    service: { name: admin, port: { number: 4000 } }
  tls:
    - hosts: [eigenwallet-admin]
```

Reachable at `https://eigenwallet-admin.tailnet-xxx.ts.net` on Tyler's tailnet only.

### Image pull

ghcr.io/tylerjw/* images are public (the migration memo confirms this for swap-cli; the eigenwallet-images workflow doesn't restrict). No imagePullSecret needed for the admin image as long as it's published public on ghcr.

### GHA build

New repo `tylerjw/eigenwallet-admin` with:

```
.github/workflows/build.yml      # cargo-chef multi-stage build, push to ghcr
Dockerfile                       # cargo-chef + tailwindcss + Leptos SSR
src/                             # Rust + Leptos
migrations/                      # diesel migrations
Cargo.toml
```

The Dockerfile is essentially `mvp0/deployment/backend.Dockerfile` with cargo-leptos and a Node.js stage for Tailwind CSS compilation. Workflow triggers on `main` push and `release/*` branches.

### Flux wiring

Add `apps/eigenwallet-admin/` to the cluster's Flux Kustomization (`clusters/homelab/...`). It picks up on next reconcile (`flux reconcile source git flux-system`).

---

## 7. Auth

### Approach: single-user password + signed-cookie session

- One row in `admin_credentials` (argon2id-hashed password). Tyler creates it by running a `seed_admin` binary (mirroring `mvp0/backend/src/bin/seed_reviewer.rs`) once at deploy time: `kubectl exec -it ... -- seed-admin set-password`. This avoids writing the password into a Secret manifest; it lives only as a hash in Postgres.
- Login form posts username+password (or just password — there's only one user). On match, the server issues a tower-sessions cookie with `{ authed: true, exp: now + 30d }`.
- All `/api/*` and `/admin/*` routes require a valid session via an axum middleware extractor. The `/login` and `/api/auth/login` routes are exempt.
- Rate limit login attempts: `tower_governor` with a per-IP key, 5 attempts per minute. Failed attempts also log to `tracing` so Loki shows them.

### Crate specifics

- `argon2 = "0.5"` — same default params as tarot uses.
- `tower-sessions = "0.13"` with `tower-sessions-memory-store` for v1 (single replica, session loss on restart is acceptable — operator just logs in again). Upgrade path: `tower-sessions-postgres-store` if we ever want session persistence across restarts.
- `cookie::SameSite::Strict`, `Secure: true`, `HttpOnly: true`, path `/`.
- CSRF: tower-sessions provides token rotation. For state-changing endpoints (POST/PUT/DELETE), require an `X-CSRF-Token` header matching the session-issued value. Leptos server functions handle this automatically; direct AJAX clients need to fetch the token from a `/api/csrf` endpoint first.

### Lost password recovery

There's no email. If Tyler loses the password:
1. `kubectl exec -it admin-pod -- seed-admin set-password` — overwrites the existing row.

Document this in a `README.md` in the repo. No "forgot password" flow in the app itself.

### What NOT to do

- No OAuth, no SSO, no Google sign-in. Single-user homelab.
- No JWT — sessions are kept server-side via tower-sessions; the cookie carries only the session ID. (Tarot uses JWT because it has mobile clients; this admin console is browser-only.)
- No password reset by email.

---

## 8. Recycle execution safety

This is real money on a personal homelab; the safeguards stack matters more than perfect UX.

### Pre-execution gates

1. **Two-page flow**, not a single button. Recycle is a dedicated page; it does not appear on the Overview.
2. **Live re-quote on confirm**: before showing the confirmation, the server re-fetches the counterparty quote via a fresh `list-sellers`-scoped scan (or a targeted `swap` quote subcommand if one exists). If the quote drifted beyond a configurable threshold (default 0.5%), block submission and force re-review.
3. **Typed confirmation**: operator must type the BTC amount exactly into a confirmation field to enable the Execute button. (`0.005` enters `0.005` — typo and the button stays disabled.)
4. **Magnitude gate**: if BTC amount > 50% of our current BTC inventory, OR > $5,000 USD equivalent (configurable), require a second confirmation step (second typed-confirm field with the dollar amount).
5. **Rate limit**: at most one recycle per hour, regardless of UI prompts. Enforced server-side via a `recycle_events` query, not a UI state. Override flag in URL `?override=YES_REALLY` for operator emergencies.
6. **Dry-run toggle**: a checkbox "Dry run (compute only, do not execute)" creates a recycle_event row with `status='dry_run'` and runs the Job in a mode where it stops before broadcasting the BTC tx — *if* the swap-cli supports this. If not, dry-run is replaced with a "quote-only" mode that only runs the seller-discovery + quote phase. Document the actual behavior in the UI.
7. **Confirmation token binding**: `/api/recycle/quote` returns a token = `hash(peer_id, btc_amount, server_quote_at_time, expires_in_120s)`. `/api/recycle/execute` rejects if token doesn't match a fresh recompute. Eliminates stale-confirmation races.

### During execution

- Single in-flight recycle at a time, enforced by Postgres advisory lock.
- Status streamed via WS to the UI. Stop button creates `kubectl delete job/<name>` — but only valid during the "discovery" phase; once the BTC tx is broadcast on-chain, abort is impossible (a deletion of the Job would orphan an in-flight swap with the counterparty still holding our tx). UI should clearly distinguish abortable vs unabortable phases.
- All output written to `recycle_events.notes` for audit.

### On failure

- Job exit code non-zero → `status='failed'`, `failure_reason` populated from last 50 lines of pod log.
- Operator notified in UI banner. Manual recovery only — admin console does not retry money operations.
- If the BTC tx already broadcast, the swap goes into asb's normal swap flow on the asb side too (because we're the taker), and shows up in §3.4 swaps table.

### What's deliberately not in scope

- Multi-step approval (Tyler is the only user).
- Email/SMS confirmation (no notifications infra).
- Withdrawal to cold storage from the console (`withdraw-btc` exists on asb-controller — leave it as a kubectl-exec ops step for v1).

---

## 9. Phased rollout

### v1 — MVP (target: 3 weeks)

**Week 1** — backend skeleton, auth, dashboard read-only

- New repo `tylerjw/eigenwallet-admin` initialized with the Rust template (cargo-leptos starter).
- axum + Leptos SSR + tower-sessions + argon2 wired together.
- Diesel migrations: `admin_credentials`, `balance_snapshots`, `swaps`, `cex_prices`.
- `seed-admin set-password` binary.
- asb JSON-RPC client (jsonrpsee). Verify exact RPC names against running asb.
- Overview page: shows current BTC balance, XMR balance, peer count, registration status, active swap count. Read-only. No charts yet, no edit.
- Health dashboard: simple status grid for asb / bitcoind / monerod / electrs.
- GHA build workflow for `ghcr.io/tylerjw/eigenwallet-admin:latest`.
- k8s manifests in `homelab/apps/eigenwallet-admin/` with Postgres + Tailscale ingress. Reachable at `https://eigenwallet-admin.<ts>.ts.net`.

**Week 2** — history & charts

- Tail `asb-data/logs/tracing*.log` into `swaps` table. Backfill from existing file on startup.
- Background poller for `balance_snapshots` (every 5 min).
- CEX price poller (Kraken + KuCoin, every 15 s).
- Charts: account value over time, swaps per day. uPlot integration.
- Swaps table page with state filter + pagination + computed profit per swap (mark-to-market).
- `capital_events` table + simple UI to record deposits/withdrawals.
- Mark-to-market ROI tile on Overview.

**Week 3** — spread control & competitor scans

- ConfigMap edit flow: read `asb-config`, edit `[maker]` section, write back, patch deployment annotation, poll for restart completion.
- `maker_config_history` audit table.
- Job-based `list-sellers` scans: every 10 min CronJob-equivalent driven by a background task in admin pod.
- Competitor scans page: latest scan table, our position highlighted.
- Spread recommendation tile (heuristic).
- Market position tile.
- Manual "Scan now" button.

v1 ships without recycle execution.

### v2 — polish (target: 2 weeks)

- **Recycle flow.** Full UX per §8. Requires taker wallet PVC + taker ConfigMap + funding (operator side). All safety gates implemented.
- Per-swap detail drawer.
- Ongoing-activity banner with timelock proximity warnings.
- BTC-denominated chart toggle.
- ROI: time-weighted return method.
- Mobile layout pass: verify all pages usable on a phone (the recycle page especially — typed confirmations need usable keyboard handling).
- CSV export for swaps and recycle history.

### v3 — advanced (target: open-ended)

- Dynamic pricing integration: surface `market-observer/DYNAMIC_PRICING_DESIGN.md` proposals in the console, with one-click "apply recommended spread" gated by safety checks.
- Withdrawal flow (`withdraw-btc`) — same safety gates as recycle.
- Notification integration: tailscale push or Discord webhook on near-timelock events or recycle completion.
- Per-counterparty stickiness analytics ("we've completed N swaps with peer X").
- Backtest the spread heuristic against historical scans.
- Logs page that surfaces k8s pod logs from Loki for any pod in the namespace.

---

## 10. Open questions

These are decisions where the right answer depends on operator preference and shouldn't be made unilaterally. Each should be resolved before or during the relevant implementation week.

1. **ROI methodology.** Mark-to-market (simple but misleading during deposits/withdrawals) vs TWR (correct but more complex math) vs FIFO cost-basis (most accurate, requires per-trade cost basis tracking). Recommend MTM for v1, TWR for v2. Does Tyler want anything more sophisticated, or is this overkill for a personal market maker where the cost basis is essentially "what I put in"?

2. **Swap history source of truth.** Two options for reconstructing the 130-row swap history: (a) parse the tracing JSON logs from the PVC, (b) directly read the asb sqlite DB file from the PVC. Logs are forward-only and have richer state transitions; sqlite has authoritative final states. Which does Tyler want as primary, with the other as fallback?

3. **Dynamic pricing**: stay in `market-observer/` as a script, or eventually move into the admin console with operator approval gates? The migration memo says deferred; reconfirming: is "deferred" = "doesn't go in v2 either" or = "go in v3 if it shaped up"?

4. **Patched-fork taker CLI surface.** The `swap-cli` from the fork has `list-sellers`, `deposit-address`, `buy-xmr`. Recycle uses `buy-xmr`. Does Tyler want `deposit-address` (taker wallet receive address) surfaced in the UI too — necessary if the operator funds the taker from the admin console rather than out-of-band? Or keep funding strictly as a kubectl ops step?

5. **Recycle dry-run semantics.** Does `swap buy-xmr` have a `--dry-run` or equivalent mode that stops short of broadcasting the BTC funding tx? If not, accept that "dry run" in the admin UI means "discover seller + show quote, do not execute" rather than "run the full swap minus broadcast"?

6. **Taker wallet identity.** A new wallet (new BTC seed) or reuse asb's BDK wallet for taker too? Separate is cleaner (no fund mingling, separate failure domain); shared simplifies inventory accounting. Recommend separate; need confirmation before designing the taker-data PVC.

7. **Asb restart UX during config save.** Restart is ~30-60 s during which the maker is offline (no listeners, no swap acceptance). Is that acceptable for any config change, or do we want an "off-hours only" guard? (Probably not worth it for a personal hobby maker, but worth asking.)

8. **Magnitude gate thresholds.** Default recycle magnitude gate proposed: >50% of inventory OR >$5,000. The right numbers depend on the operator's actual size and risk tolerance.

9. **Frontend hot reload during dev.** cargo-leptos with `cargo leptos watch` works but is slow on cold builds. Acceptable, or do we want to invest in a faster inner-loop dev setup (e.g., Trunk for client-only iterations, accepting that SSR diverges)? Probably "acceptable" for a homelab tool but flagging.

10. **Logs ingestion.** Tracing logs from the asb pod already flow into Loki via Promtail (per `homelab/CLAUDE.md`). Should the admin console parse them from Loki via LogQL instead of from the PVC file? Loki is the more durable source; PVC tail is simpler. Recommend PVC tail for v1, Loki integration as a v3 nice-to-have.

11. **Backup of the admin DB.** Same nightly pg_dump CronJob pattern as tarot, or skip (everything's reproducible from asb logs anyway)? Recommend nightly backup of `admin_credentials`, `capital_events`, `recycle_events` only — the snapshot/scan tables are noise that can be rebuilt.

---

### Implementation references

- An existing axum/diesel-async/argon2 backend in another homelab app — used as the dependency template for Cargo.toml, AppState shape, and env-driven config.
- The eigenwallet stack's `asb.yaml` — defines the `asb` Service and Deployment we patch (the ConfigMap + the `config-version` annotation).
- An existing app Deployment/Service pair used as the template for this app's manifest layout.
- An existing reverse-proxy/ingress pattern in the cluster (Caddy + Cloudflare DNS-challenge TLS) reused for `eigen.home.weaver-labs.xyz`.
