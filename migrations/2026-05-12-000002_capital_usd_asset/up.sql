-- Allow USD as a capital-event asset. USD events represent fiat that was
-- wired into / withdrawn from an exchange (Kraken etc.) — i.e. the operator's
-- true cost basis, distinct from the BTC/XMR rows which track the moment
-- crypto arrived at the maker.
ALTER TABLE capital_events DROP CONSTRAINT capital_events_asset_check;
ALTER TABLE capital_events ADD CONSTRAINT capital_events_asset_check
  CHECK (asset IN ('BTC', 'XMR', 'USD'));
