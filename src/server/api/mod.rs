//! Implementation modules called from the page-level Leptos `#[server]`
//! functions. Each `fetch`/`list`/`compute` here returns a DTO from
//! `crate::types`; the server function only adapts the error type.

pub mod capital;
pub mod charts;
pub mod competitors;
pub mod health;
pub mod maker;
pub mod market;
pub mod overview;
pub mod roi;
pub mod spread;
pub mod swaps;
pub mod wallet_rules;
