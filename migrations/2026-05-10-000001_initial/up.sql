CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Single-user auth. One row.
CREATE TABLE admin_credentials (
    id uuid PRIMARY KEY DEFAULT uuid_generate_v4(),
    password_hash text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    rotated_at timestamptz
);

-- Balance & price snapshots for charts. Written every 5 min.
CREATE TABLE balance_snapshots (
    taken_at timestamptz PRIMARY KEY,
    btc_sat bigint NOT NULL,
    xmr_atomic numeric(40,0) NOT NULL,
    btc_usd numeric(20,8) NOT NULL,
    xmr_usd numeric(20,8) NOT NULL,
    total_usd numeric(20,8) NOT NULL,
    total_btc numeric(20,10) NOT NULL
);
CREATE INDEX idx_balance_snapshots_taken_at ON balance_snapshots (taken_at DESC);

-- Mirror of asb's swap history.
CREATE TABLE swaps (
    swap_id text PRIMARY KEY,
    peer_id text NOT NULL,
    state text NOT NULL,
    btc_sat bigint NOT NULL,
    xmr_atomic numeric(40,0) NOT NULL,
    started_at timestamptz NOT NULL,
    completed_at timestamptz,
    btc_usd_at_completion numeric(20,8),
    xmr_usd_at_completion numeric(20,8),
    profit_usd numeric(20,8),
    raw_log_excerpt jsonb
);
CREATE INDEX idx_swaps_started_at ON swaps (started_at DESC);
CREATE INDEX idx_swaps_state ON swaps (state);

-- Operator-recorded capital deposits/withdrawals for ROI math.
CREATE TABLE capital_events (
    id uuid PRIMARY KEY DEFAULT uuid_generate_v4(),
    occurred_at timestamptz NOT NULL,
    direction text NOT NULL CHECK (direction IN ('deposit','withdraw')),
    asset text NOT NULL CHECK (asset IN ('BTC','XMR')),
    amount_atomic numeric(40,0) NOT NULL,
    usd_value_at_event numeric(20,8),
    notes text,
    created_at timestamptz NOT NULL DEFAULT now()
);

-- Cached CEX prices.
CREATE TABLE cex_prices (
    sampled_at timestamptz PRIMARY KEY,
    btc_usd numeric(20,8),
    xmr_usd numeric(20,8),
    btc_xmr numeric(20,10),
    sources text[]
);

-- Competitor scans.
CREATE TABLE competitor_scans (
    scan_id uuid PRIMARY KEY DEFAULT uuid_generate_v4(),
    started_at timestamptz NOT NULL,
    completed_at timestamptz,
    trigger text NOT NULL CHECK (trigger IN ('cron','manual','recycle_prep')),
    raw_output text
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

-- Audit trail of [maker] config changes.
CREATE TABLE maker_config_history (
    id bigserial PRIMARY KEY,
    changed_at timestamptz NOT NULL DEFAULT now(),
    previous_toml text NOT NULL,
    new_toml text NOT NULL,
    restart_observed_at timestamptz,
    notes text
);
