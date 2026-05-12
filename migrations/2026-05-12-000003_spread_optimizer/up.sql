-- Auto-spread optimizer state. One row of operator-tunable parameters
-- (gamma, bounds, target margin, auto-apply flag); many rows of
-- recommendation history with the inputs that produced each one so the
-- operator can audit what the algorithm "saw."

CREATE TABLE spread_optimizer_config (
    -- enforced singleton: every row must have id=1
    id              INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    -- Avellaneda-Stoikov risk aversion. Higher = wider spreads.
    gamma           NUMERIC NOT NULL DEFAULT 1.0,
    -- Hard floor / ceiling that the recommendation will be clamped to.
    -- 0.015 = 1.5%, 0.08 = 8%.
    min_spread      NUMERIC NOT NULL DEFAULT 0.015,
    max_spread      NUMERIC NOT NULL DEFAULT 0.08,
    -- Operator's target USD profit per swap. Drives the margin_term.
    target_swap_profit_usd  NUMERIC NOT NULL DEFAULT 5.0,
    -- Estimated round-trip recycle cost per swap (USD). Updated as we
    -- get a better number from the recycling research.
    amortized_recycle_cost_usd  NUMERIC NOT NULL DEFAULT 8.0,
    -- Estimated on-chain BTC+XMR fees per swap (USD).
    chain_fees_per_swap_usd     NUMERIC NOT NULL DEFAULT 5.0,
    -- Max change per cycle (e.g. 0.0025 = ±0.25%).
    step_size_max               NUMERIC NOT NULL DEFAULT 0.0025,
    -- Cooldown between auto-applies, seconds.
    cooldown_seconds            INTEGER NOT NULL DEFAULT 1800,
    -- If true, the poller actually writes to asb ConfigMap. If false,
    -- it only records recommendations and the operator applies manually.
    auto_apply      BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO spread_optimizer_config DEFAULT VALUES;

CREATE TABLE spread_recommendations (
    id                  BIGSERIAL PRIMARY KEY,
    recommended_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    current_spread      NUMERIC NOT NULL,
    recommended_spread  NUMERIC NOT NULL,
    -- JSONB breakdown of the components that produced the recommendation:
    -- floor, vol_term, inventory_term, competitor_term, margin_term,
    -- raw_vol_30min, inventory_skew, our_rank, top_quartile_cutoff, etc.
    components          JSONB NOT NULL,
    -- One-line operator-readable rationale ("widened 0.25%: σ_30min up 40%").
    rationale           TEXT NOT NULL,
    applied             BOOLEAN NOT NULL DEFAULT FALSE,
    applied_at          TIMESTAMPTZ
);

CREATE INDEX idx_spread_recommendations_at
    ON spread_recommendations (recommended_at DESC);
