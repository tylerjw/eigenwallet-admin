# eigenwallet-admin

Web admin console for the eigenwallet ASB market maker running in Tyler's homelab k8s cluster. Tailscale-only access, single-user password auth, mobile-friendly UI.

Status: **design only**. See [`PLAN.md`](./PLAN.md) for the architecture, tech-stack decisions, feature breakdown, and phased rollout. No code yet.

## Why

Today, operating the maker requires `kubectl exec -it deploy/asb-controller -- asb-controller --url=http://asb:9944 <cmd>`. This repo will replace that with a Rust+Axum web UI that exposes market position, swap history, system health, the taker market, and a controlled path for spread changes and BTC→XMR recycles.

## What's in the box (planned)

- **Backend**: Rust + axum + diesel + PostgreSQL (via CloudNativePG)
- **Frontend**: Leptos (Rust, SSR + hydration) — see plan for the Leptos-vs-React rationale
- **Auth**: argon2 password hash + tower-sessions cookie, single user
- **Deploy**: GHA-built container in ghcr.io, k8s manifests in `homelab/apps/eigenwallet-admin/`, Tailscale userspace ProxyClass for ingress

## Open questions for the operator

See section 10 of `PLAN.md`. Highlights:

- ROI methodology: mark-to-market vs cost-basis tracking
- Dynamic pricing (auto-adjust spread): integrate into console or keep as separate script?
- Taker tooling: keep the patched-fork swap-cli as a separate deployment or fold its capabilities into the admin?
- Backup/recovery posture for the postgres history DB

Once those are answered, work proceeds per the phased rollout in `PLAN.md` (v1 ~ 1 week, v2/v3 follow-on).
