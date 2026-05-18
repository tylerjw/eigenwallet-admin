# Recycle runbook — Kraken, US operator

Manual procedure for rebalancing the maker wallet by moving accumulated
BTC back into XMR via Kraken. Cadence: every 1–2 weeks, batched. Per-swap
recycling on Kraken is wildly more expensive (withdrawal fees dominate);
do not do it.

Eigenwallet ASB is one-directional by protocol — the maker only sells
XMR and only receives BTC — so the BTC float **always** drifts up.
This is a structural inventory bleed, not a misconfiguration; rebalancing
is permanent operational overhead, and the only question is how cheaply
you can do it.

## Pre-flight (5 min)

1. **Mempool check** — [mempool.space](https://mempool.space). If next-block
   sat/vB ≥ 50, wait 12–24 h. Sunday early-UTC tends to be cheapest;
   this is folklore, not data.
2. **Address whitelist confirmed** — Kraken → Funding → Withdraw → Bitcoin
   and Monero. Verify the maker wallet's BTC and XMR receive addresses
   are already on the whitelist. Kraken's 1-hour add delay will derail
   you mid-recycle otherwise.
3. **Generate a fresh Kraken BTC deposit address** — Funding → Deposit →
   Bitcoin → Generate New Address. Use a new address per batch; never
   reuse.
4. **Glance at XMR/USDT depth** — Kraken Pro → XMR/USDT. If touch spread
   is > 0.5%, the book is unusual that day. Defer or slice smaller.

## Step 1 — Pause the maker

On `/overview`, hit **Pause maker**. asb rolls the off-market config
(`ask_spread = 5.0`, `max_buy_btc = 0`) in ~30–60 s. In-flight swaps
continue settling; new ones can't start. This protects you from a
"BTC withdrawn mid-quote" race.

## Step 2 — Withdraw BTC from maker wallet to Kraken

Use the **controller**, not `asb` directly. Running `asb withdraw-btc`
from inside the asb pod hangs on Bitcoin wallet lock contention with
the live daemon, and it expects an interactive TTY for confirmation
prompts that `kubectl exec` can't reliably provide. The controller
goes through asb's JSON-RPC (port 9944) so no lock, no TTY, no prompts.

```sh
kubectl exec -n eigenwallet deploy/asb-controller -- \
  asb-controller --url=http://asb:9944 withdraw-btc \
  <kraken_btc_deposit_address> "0.05 BTC"
```

Arguments are **positional** (address first, amount second). Amount
accepts `"0.05 BTC"` or `"5000000 sat"`; omit to sweep the wallet.

Batch the entire accumulated BTC float **minus a reserve of ~0.005 BTC**
so the pod has something to quote with when you resume. Don't drain to
zero.

The controller prints `Withdrew X BTC in transaction <txid>` immediately.
Save the txid — you'll log it later. Wait for **3 confirmations** (~30 min,
faster at low mempool fees) before Kraken credits the deposit. asb's
fee selection uses a 1-block confirmation target by default; at low
mempool sat/vB you'll pay near the floor (~1.25 sat/vB on the 2026-05-17
recycle; 80-input sweep, ~5,500 vB, ~$0.70 total fee).

## Step 3 — BTC → USDT

Kraken Pro → **BTC/USDT** (Kraken's ticker `XBT/USDT` is an alias) →
Sell → **Limit, post-only**.

- Slice into 2–3 clips. For a $750–1500 batch, 2 clips is fine; for
  $10k+ batches, 3 clips of roughly equal size keeps each well inside
  BTC/USDT's deep book.
- Place at the **current best ask** (your side of the touch, top of
  the red ladder). Post-only on a sell would reject if placed at the
  bid as a crossing order, so join the ask ladder instead. No
  undercutting (don't go below the bid), no widening (don't price
  further away than the inside ask).
- If a clip doesn't fill in ~10 min, cancel and re-place at the new touch.
- **Never go market. Never disable post-only.** The patience pays the
  difference between maker (0.20%) and taker (0.35%) — 15 bps on a
  $5k clip is real money.
- If the book moves persistently against you for >30 min, accept that
  today is a bad day. Pause and resume tomorrow.

Expected: **0.20% maker fee** at the default volume tier, plus
~0.02–0.05% slippage on a small clip in deep BTC/USDT. The BTC leg
is reliably the cheap, fast half of the recycle.

## Step 4 — USDT → XMR

Kraken Pro → **XMR/USDT** → Buy → **Limit, post-only**.

- **Slice smaller** than the BTC leg. The XMR book thins above ~$2k.
- For a $1k batch this is one clip; for $5–10k batches do 3–5 clips
  of $1k each, spaced 5–10 min; for $15–25k batches, 6–8 clips of
  $2.5–3.5k each balances depth-impact vs. operator time-cost (a
  $20k batch in $1k clips spread across 5 min spacing is ~3 hours
  of trading attention — too long).
- Place at the **current best bid** (your side of the touch, top of
  the green ladder). Post-only on a buy would reject if placed at the
  ask as a crossing order, so join the bid ladder instead.
- Re-peg every 5–10 min if unfilled, **but not every time a bot
  jumps you by 1 tick** — see "Reading the book" below.
- If the orderbook is empty above the touch and you're not filling,
  that's the book telling you XMR sellers want a premium today. Wait
  an hour or resume tomorrow.

Expected: **0.20% maker fee** + **0.10–0.25% effective spread** at
the XMR book depth. The XMR leg is structurally the expensive, slow
half of the recycle.

### Reading the book

XMR/USDT spread is structurally ~10× wider than BTC/USDT (e.g. ~10 bps
vs. ~1 bp), because Monero has lower exchange volume and several venues
have delisted it (Binance, OKX), concentrating activity on fewer makers
without the HFT competition that compresses BTC's spread to a fraction
of a basis point. Two consequences:

- **Wider spread = better outcome for you as the patient maker.** Each
  fill of your bid is a counterparty paying the 10 bps spread to you
  vs. crossing as a taker. Don't be tempted by tiny price improvements;
  the structural spread is yours to capture.
- **Inside touch is decorative.** You will see microscopic orders
  (e.g. 0.01 XMR ≈ $4) sitting one tick inside your bid, and again
  one tick inside the best ask. These are queue-jumper bots: they
  buy first-in-line priority at trivial cost in order to (a) be the
  tripwire that signals incoming aggressive flow, (b) capture both
  sides of the spread by being inside on each side, and (c) paint
  the book to look tighter than it really is. Real depth starts at
  *your* level. **Don't chase them by re-pegging to outbid by 1 tick**
  — they'll just re-jump for another cent, and you're moving the book
  against yourself for free intel they take. Sit; their tiny size dies
  first to real aggressive flow, and your size fills as the flow
  keeps going.

## Step 5 — Withdraw XMR to maker wallet

Funding → Withdraw → Monero → select whitelisted address → **Send Max**
→ submit. 2FA + email confirmation.

Kraken's email sequence on a withdrawal is 3 stages — useful to know
which one you're looking at:

1. **"You've initiated a withdrawal"** — Kraken received the request,
   queued for ops review. No on-chain action yet.
2. **"Funds being processed"** / **"Funds being sent"** — ops released,
   broadcast imminent.
3. **"Withdrawal sent"** — broadcast on-chain, includes the Monero txid.

Wall-clock between #1 and #3 is typically 5–30 min for routine
withdrawals; mid-sized batches ($10k+) sometimes stretch to 60 min if
they get flagged for manual review. If you're past 90 min Pending with
no movement, check Kraken's status page before opening a support ticket.

After broadcast, **10 Monero confirmations** are needed (~20 min — 2-min
block time). The asb monero wallet auto-syncs, so as soon as confs land,
admin's `/overview` will reflect the new XMR balance.

XMR withdrawal fee: flat ~0.0001 XMR, negligible (<$0.05).

Programmatic status from your laptop (uses the read-only API key set up
in `homelab/scripts/kraken-query.py`):

```sh
homelab/scripts/kraken-query.py WithdrawStatus '{"asset": "XMR"}'
```

## Step 6 — Resume maker

Once XMR has landed and `/overview` shows the new XMR balance, hit
**Resume maker** on `/overview`. asb rolls back onto the pre-pause
spread (verbatim restore from `maker_config_history`).

## Step 7 — Record the event

Until the `rebalance_events` table + `/recycle` page exist (see "Why
we don't automate execution" below), recycles are logged as **two
paired rows in `capital_events`** — one BTC `withdraw`, one XMR
`deposit` — tied together via the notes field. This preserves the
operator's true cost basis for ROI math and gives the optimizer
historical recycle costs to learn from.

Pattern (run on the admin Postgres):

```sql
INSERT INTO capital_events
  (occurred_at, direction, asset, amount_atomic, usd_value_at_event, notes)
VALUES
  ('<broadcast-time-utc>', 'withdraw', 'BTC',
   <sats>, <gross-usdt-from-btc-sells>,
   'Recycle leg out: asb → Kraken via <btc-txid>. Got back <xmr> XMR ' ||
   '($<gross-usdt-spent-on-xmr> at buy-leg prices). Round-trip cost ' ||
   '$<total-fees> (<pct>%) in Kraken fees.'),
  ('<xmr-land-time-utc>', 'deposit', 'XMR',
   <piconero>, <gross-usdt-spent-on-xmr>,
   'Recycle leg: asb sent <btc> BTC to Kraken at <broadcast-time>, ' ||
   'got back <xmr> XMR. Paired with the BTC withdrawal. Kraken flat ' ||
   'fee 0.0001 XMR. Monero txid <xmr-txid>.');
```

Atomic units: BTC × 1e8 (satoshi), XMR × 1e12 (piconero).

Pulling the numbers programmatically:

```sh
# Trade fills + fees for the recycle window
homelab/scripts/kraken-query.py TradesHistory '{"start": <epoch>}'

# XMR withdrawal txid (after Kraken broadcasts)
homelab/scripts/kraken-query.py WithdrawStatus '{"asset": "XMR"}'

# Live balances on the maker
kubectl exec -n eigenwallet deploy/asb-controller -- \
  asb-controller --url=http://asb:9944 bitcoin-balance
kubectl exec -n eigenwallet deploy/asb-controller -- \
  asb-controller --url=http://asb:9944 monero-balance
```

Then on `/spread` → Optimizer settings, update
`amortized_recycle_cost_usd` to match the actual realized per-swap
cost (= total_recycle_usd_cost / expected_swaps_until_next_recycle).
The optimizer's floor term depends on this; a stale estimate makes
its recommendations worse, not better.

## Cost expectations

For a US operator at Kraken's default 0.20%/0.35% maker/taker tier,
batched bi-weekly through the BTC → USDT → XMR path:

| Batch size | Est. total cost | As % of batch |
|---|---|---|
| $750  | ~$15–22 | 2.0–3.0% |
| $1500 | ~$20–28 | 1.4–1.9% |
| $5000 | ~$30–45 | 0.6–0.9% |
| $10000 | ~$55–85 | 0.55–0.85% |

The fixed BTC withdrawal fee (~$5–15) dominates at small batch sizes.
Double the batch ≈ halve the percentage cost. Bi-weekly batches of
< $1000 are not economically viable on Kraken.

## Realized recycles

| Date | Batch | BTC out | XMR in | Total fees | % of batch | Notes |
|---|---|---|---|---|---|---|
| 2026-04-15 | ~$2k | 0.0271 BTC | 5.7183 XMR | ~$58 slip+fees | ~2.9% | Small batch, fixed BTC withdraw fee dominates |
| 2026-04-18 / 25 | ~$1.4k | 0.0180 BTC | 3.7569 XMR | — | — | Split-deposit (2.998 immediate + 0.7589 batched on 04-25) |
| 2026-05-17 | $17.4k | 0.22255688 BTC | 44.2580 XMR | $75.24 | **0.43%** | Below runbook band; BTC sell 0.23% all-in, XMR buy 0.20% maker + ~0.18% effective spread. Mempool was at 1 sat/vB so on-chain fee was trivial |

The 2026-05-17 batch confirms the bottom of the band is reachable at
$15k+ batch sizes when both legs cooperate (deep BTC liquidity + calm
XMR book).

## What to avoid

- **TradeOgre** — defunct (May 2026 RCMP seizure).
- **MEXC, KuCoin, OKX** from a US IP — geofenced.
- **Instant swappers** (FixedFloat, ChangeNow, SimpleSwap) — headline
  0.5–1% service fee is on top of a 1–3% built-in rate markup. Net
  cost 2.5–4%.
- **Haveno / RetoSwap** as primary — slow, deeper-spread, BTC↔XMR
  liquidity at $5k+ batches not reliable in 2026.
- **Recycling via eigenwallet taker side** — you're paying another
  maker's 2–5% spread, which is exactly the trap you're trying to
  escape.
- **Disabling post-only** to force a faster fill — the taker fee
  premium burns more than the time saved is worth.
- **Recycling per swap** instead of batching — withdrawal fees dominate;
  this is the single biggest controllable cost.

## Why competitors quote 1–2%

Ranked by likelihood:

1. **Non-US makers on MEXC, Binance, OKX.** XMR/USDT trades at 0%/0.05%
   maker/taker on MEXC vs. your 0.20%/0.35%. Their recycle cost is
   roughly 0.10–0.20% all-in; yours is 0.55–0.85% best case. This
   single factor explains most of the gap.
2. **Higher Kraken volume tier.** At $250k+/month they pay 0.10% maker
   (half your fee). Requires real volume — don't wash-trade to qualify.
3. **Two-way flow makers.** Some platforms see XMR→BTC takers as well,
   so the maker only recycles the net imbalance rather than every swap.
   The eigenwallet protocol is one-directional, so this doesn't apply
   to you.
4. **Running as a privacy service, not a profit business.** Several
   long-running makers (e.g. Seth for Privacy) historically treated
   their ASB as ecosystem contribution; spread covers operational
   cost, not opportunity cost of capital.
5. **Self-mined XMR.** A CPU-miner using P2Pool can replenish XMR
   without a BTC→XMR recycle leg at all. Speculation, no confirmed
   public examples.

You have a structural ~2–2.5% recycle floor that an MEXC operator
doesn't. Don't race them to the bottom. Compete on uptime, completion
rate, and recognizable peer-id reputation; quote a notch above the
cheapest and accept the volume you get.

## Why we don't automate execution

| What | Build? | Why |
|---|---|---|
| Kraken withdraw API | **No** | Requires `Withdraw Funds` permission on the API key. Key compromise = full account drain. The address whitelist barely mitigates this. The risk dwarfs the operational gain at 24 recycles/year. |
| Kraken order placement API | **No** | Doable, but at bi-weekly cadence the automation saves ~8 hours/year and adds a maintenance burden. Order slicing depends on live book depth that a human reads better than a heuristic. |
| `rebalance_events` table + `/recycle` page | **Yes** | Forces logging discipline. Lets the optimizer learn actual `amortized_recycle_cost_usd` from real data instead of operator guesses. Read-only — no withdrawal authority required. Not yet built. |
| Pre-flight checks page | **Yes** | A `/recycle` page showing mempool sat/vB, current asb balances, suggested batch size, last recycle date + cost, and this checklist inline. Read-only. Not yet built. |
| Kraken read-only book snapshot | **Maybe** | Public API, no auth. Could surface "XMR/USDT touch spread is 0.18%, good day" or "0.55%, defer." Modest value; build after the table + form. |
| Auto-pause / auto-resume around recycle | **No** | The pause is a deliberate operator decision (you might want to keep accepting BTC while waiting for Kraken to credit). Don't automate decisions. |

## Sources

The recommendations above are synthesized from the recycling-research
and Kraken-tactics agents that ran during the maker setup. Cited
sources:

- [Kraken fee schedule](https://www.kraken.com/features/fee-schedule)
- [Kraken cryptocurrency withdrawal fees & minimums](https://support.kraken.com/articles/360000767986-cryptocurrency-withdrawal-fees-and-minimums)
- [Kraken XMR withdrawal fee announcement](https://x.com/krakenfx/status/1057097814012944384)
- [Kraken withdrawal-fee overview](https://withdrawalfees.com/exchanges/kraken)
- [Kraken market & limit orders / post-only docs](https://support.kraken.com/articles/7570598822932-market-and-limit-orders)
- [Haveno DEX review — Baltex 2026](https://baltex.io/blog/ecosystem/haveno-dex-review-future-of-decentralized-monero-trading)
- [RetoSwap (largest Haveno instance)](https://retoswap.com/)
- [Monero P2P DEX survey](https://arxiv.org/html/2505.02392v3)
- [eigenwallet review (KYCnot.me)](https://kycnot.me/service/eigenwallet)
- [Seth For Privacy — running an atomic swap provider](https://sethforprivacy.com/archives/run-an-atomic-swap-provider-advanced/)
- [eigenwallet/core](https://github.com/eigenwallet/core)
