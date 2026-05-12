// Hand-written diesel schema matching migrations/2026-05-10-000001_initial.
// `diesel print-schema` would regenerate this once a real DB is available.

diesel::table! {
    admin_credentials (id) {
        id -> Uuid,
        password_hash -> Text,
        created_at -> Timestamptz,
        rotated_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    balance_snapshots (taken_at) {
        taken_at -> Timestamptz,
        btc_sat -> Int8,
        xmr_atomic -> Numeric,
        btc_usd -> Numeric,
        xmr_usd -> Numeric,
        total_usd -> Numeric,
        total_btc -> Numeric,
    }
}

diesel::table! {
    swaps (swap_id) {
        swap_id -> Text,
        peer_id -> Text,
        state -> Text,
        btc_sat -> Int8,
        xmr_atomic -> Numeric,
        started_at -> Timestamptz,
        completed_at -> Nullable<Timestamptz>,
        btc_usd_at_completion -> Nullable<Numeric>,
        xmr_usd_at_completion -> Nullable<Numeric>,
        profit_usd -> Nullable<Numeric>,
        raw_log_excerpt -> Nullable<Jsonb>,
    }
}

diesel::table! {
    capital_events (id) {
        id -> Uuid,
        occurred_at -> Timestamptz,
        direction -> Text,
        asset -> Text,
        amount_atomic -> Numeric,
        usd_value_at_event -> Nullable<Numeric>,
        notes -> Nullable<Text>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    cex_prices (sampled_at) {
        sampled_at -> Timestamptz,
        btc_usd -> Nullable<Numeric>,
        xmr_usd -> Nullable<Numeric>,
        btc_xmr -> Nullable<Numeric>,
        sources -> Array<Text>,
    }
}

diesel::table! {
    competitor_scans (scan_id) {
        scan_id -> Uuid,
        started_at -> Timestamptz,
        completed_at -> Nullable<Timestamptz>,
        trigger -> Text,
        raw_output -> Nullable<Text>,
    }
}

diesel::table! {
    competitor_quotes (id) {
        id -> Int8,
        scan_id -> Uuid,
        peer_id -> Text,
        multiaddr -> Nullable<Text>,
        price_btc_per_xmr -> Nullable<Numeric>,
        min_btc -> Nullable<Numeric>,
        max_btc -> Nullable<Numeric>,
        reachable -> Bool,
        reason_if_unreachable -> Nullable<Text>,
        version -> Nullable<Text>,
    }
}

diesel::table! {
    maker_config_history (id) {
        id -> Int8,
        changed_at -> Timestamptz,
        previous_toml -> Text,
        new_toml -> Text,
        restart_observed_at -> Nullable<Timestamptz>,
        notes -> Nullable<Text>,
    }
}

diesel::joinable!(competitor_quotes -> competitor_scans (scan_id));

diesel::allow_tables_to_appear_in_same_query!(
    admin_credentials,
    balance_snapshots,
    swaps,
    capital_events,
    cex_prices,
    competitor_scans,
    competitor_quotes,
    maker_config_history,
);
