DELETE FROM capital_events WHERE asset = 'USD';
ALTER TABLE capital_events DROP CONSTRAINT capital_events_asset_check;
ALTER TABLE capital_events ADD CONSTRAINT capital_events_asset_check
  CHECK (asset IN ('BTC', 'XMR'));
