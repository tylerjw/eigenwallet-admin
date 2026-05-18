-- Track Kraken-side balances in each snapshot so the chart's total value
-- doesn't dip during a recycle (when BTC has left the maker wallet but
-- hasn't yet come back as XMR — the value isn't gone, it's at Kraken).
--
-- Default 0 for backfill: existing rows stay correct (asb-only total),
-- new rows will be populated by the poller when the Kraken read-only
-- API is reachable.
--
-- kraken_usd aggregates ZUSD (fiat) + USDT (stablecoin) — both ~$1 each,
-- not worth separate columns at this point.
ALTER TABLE balance_snapshots
  ADD COLUMN kraken_btc_sat     bigint        NOT NULL DEFAULT 0,
  ADD COLUMN kraken_xmr_atomic  numeric(40,0) NOT NULL DEFAULT 0,
  ADD COLUMN kraken_usd         numeric(20,8) NOT NULL DEFAULT 0;
