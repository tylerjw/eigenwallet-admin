# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: bugs.spec.ts >> UI bugs from operator review >> swap rows have a profit_usd value populated for completed swaps
- Location: bugs.spec.ts:35:3

# Error details

```
Error: expect(received).toBeGreaterThan(expected)

Expected: > 0
Received:   0
```

# Page snapshot

```yaml
- generic [ref=e2]:
  - banner [ref=e3]:
    - generic [ref=e4]:
      - link "eigenwallet admin" [ref=e5] [cursor=pointer]:
        - /url: /
      - link "Overview" [ref=e6] [cursor=pointer]:
        - /url: /
      - link "Health" [ref=e7] [cursor=pointer]:
        - /url: /health
      - link "Swaps" [ref=e8] [cursor=pointer]:
        - /url: /swaps
      - link "Charts" [ref=e9] [cursor=pointer]:
        - /url: /charts
      - link "Spread" [ref=e10] [cursor=pointer]:
        - /url: /spread
      - link "Competitors" [ref=e11] [cursor=pointer]:
        - /url: /competitors
      - link "ROI" [ref=e12] [cursor=pointer]:
        - /url: /roi
      - link "Wallet rules" [ref=e13] [cursor=pointer]:
        - /url: /wallet-rules
      - button "Logout" [ref=e15] [cursor=pointer]
  - main [ref=e16]:
    - generic [ref=e17]:
      - heading "Overview" [level=1] [ref=e18]
      - generic [ref=e19]:
        - generic [ref=e20]:
          - generic [ref=e21]: BTC balance
          - generic [ref=e22]: 0.11604 BTC
          - generic [ref=e23]: spendable BTC in the maker wallet
        - generic [ref=e24]:
          - generic [ref=e25]: XMR balance
          - generic [ref=e26]: 22.294433 XMR
          - generic [ref=e27]: spendable XMR in the maker wallet
        - generic [ref=e28]:
          - generic [ref=e29]: Total (USD)
          - generic [ref=e30]: $18690.25
          - generic [ref=e31]: BTC + XMR valued at the latest CEX mid
        - generic [ref=e32]:
          - generic [ref=e33]: Active swaps
          - generic [ref=e34]: "0"
          - generic [ref=e35]: swaps still in progress (not yet redeemed/refunded)
        - generic [ref=e36]:
          - generic [ref=e37]: Peers
          - generic [ref=e38]: "21"
          - generic [ref=e39]: active libp2p connections
        - generic [ref=e40]:
          - generic [ref=e41]: Rendezvous
          - generic [ref=e42]: 4/8
          - generic [ref=e43]: registered at 4 of 8 configured rendezvous nodes — peers discover us via these
        - generic [ref=e44]:
          - generic [ref=e45]: Spread
          - generic [ref=e46]: +2.41%
          - generic [ref=e47]: our quoted price vs. CEX mid (positive = we charge a premium for XMR)
        - generic [ref=e48]:
          - generic [ref=e49]: Onion
          - generic [ref=e50]: —
          - generic [ref=e51]: no hidden-service address yet — Tor still bootstrapping
      - paragraph [ref=e52]: Last updated 2026-05-12T00:58:33.544847978+00:00
      - generic [ref=e53]:
        - generic [ref=e54]: Total value (USD, 7d)
        - generic [ref=e55]:
          - generic [ref=e56]: $18,687.74
          - generic [ref=e57]:
            - generic [ref=e58]: May 12, 2026 00:55 UTC
            - generic [ref=e59]: +$4,722.11 (+33.81%)
          - img [ref=e60]:
            - generic [ref=e61]: $18,838
            - generic [ref=e62]: $16,394
            - generic [ref=e63]: $13,950
            - generic [ref=e64]: May 11
            - generic [ref=e65]: May 11
            - generic [ref=e66]: May 12
```

# Test source

```ts
  1   | // Regression tests for issues caught during operator UI review on 2026-05-12.
  2   | // Each test is named after the bug and asserts the *fixed* behavior, so the
  3   | // suite goes red before the fix lands and green once it does.
  4   | 
  5   | import { test, expect } from '@playwright/test';
  6   | import { login, api } from './helpers';
  7   | 
  8   | test.describe('UI bugs from operator review', () => {
  9   |   test.beforeEach(async ({ page }) => {
  10  |     await login(page);
  11  |   });
  12  | 
  13  |   test('attribution chart has no zero-total dips', async ({ page }) => {
  14  |     const dto = await api<{
  15  |       actual: { t: string; v: string }[];
  16  |       sample_count: number;
  17  |     }>(page, '/api/charts/attribution');
  18  |     expect(dto.sample_count).toBeGreaterThan(2);
  19  |     const minActual = Math.min(...dto.actual.map((p) => parseFloat(p.v)));
  20  |     // All snapshots have non-zero balance; total should always be a real USD
  21  |     // value. The bug manifested as a few dips to $0 from CEX cache misses.
  22  |     expect(minActual).toBeGreaterThan(100);
  23  |   });
  24  | 
  25  |   test('account-value chart has no zero dips', async ({ page }) => {
  26  |     const dto = await api<{ points: { t: string; v: string }[] }>(
  27  |       page,
  28  |       '/api/charts/account-value',
  29  |     );
  30  |     if (dto.points.length === 0) return;
  31  |     const min = Math.min(...dto.points.map((p) => parseFloat(p.v)));
  32  |     expect(min).toBeGreaterThan(100);
  33  |   });
  34  | 
  35  |   test('swap rows have a profit_usd value populated for completed swaps', async ({
  36  |     page,
  37  |   }) => {
  38  |     const dto = await api<{ rows: { state: string; profit_usd: string | null }[] }>(
  39  |       page,
  40  |       '/api/swaps',
  41  |     );
  42  |     const completed = dto.rows.filter((r) =>
  43  |       r.state.toLowerCase().includes('redeemed'),
  44  |     );
  45  |     expect(completed.length).toBeGreaterThan(0);
  46  |     // At least *some* completed swaps must have a profit number — the bug was
  47  |     // that every row had profit_usd: null.
  48  |     const withProfit = completed.filter((r) => r.profit_usd !== null);
> 49  |     expect(withProfit.length).toBeGreaterThan(0);
      |                               ^ Error: expect(received).toBeGreaterThan(expected)
  50  |   });
  51  | 
  52  |   test('swap state filter "completed" returns non-zero rows', async ({ page }) => {
  53  |     await page.goto('/swaps');
  54  |     await page.getByRole('button', { name: 'completed', exact: true }).click();
  55  |     await page.waitForTimeout(500);
  56  |     const rowCount = await page.locator('table tbody tr').count();
  57  |     expect(rowCount).toBeGreaterThan(0);
  58  |   });
  59  | 
  60  |   test('swap state filter "refunded" returns rows when there are refunds', async ({
  61  |     page,
  62  |   }) => {
  63  |     // There are known refunded swaps in the DB; filter should surface them.
  64  |     await page.goto('/swaps');
  65  |     await page.getByRole('button', { name: 'refunded', exact: true }).click();
  66  |     await page.waitForTimeout(500);
  67  |     const rowCount = await page.locator('table tbody tr').count();
  68  |     expect(rowCount).toBeGreaterThan(0);
  69  |   });
  70  | 
  71  |   test('chart period buttons cause a re-fetch (different period -> different sample count when data is sparse it can match, but the request must be sent)', async ({
  72  |     page,
  73  |   }) => {
  74  |     await page.goto('/charts');
  75  |     const reqs: string[] = [];
  76  |     page.on('request', (req) => {
  77  |       if (req.url().includes('/api/charts/account-value')) reqs.push(req.url());
  78  |     });
  79  |     await page.getByRole('button', { name: '24h', exact: true }).click();
  80  |     await page.waitForTimeout(800);
  81  |     await page.getByRole('button', { name: '30d', exact: true }).click();
  82  |     await page.waitForTimeout(800);
  83  |     // We should see at least two requests after the page loaded (the initial
  84  |     // is racey).
  85  |     expect(reqs.length).toBeGreaterThan(1);
  86  |   });
  87  | 
  88  |   test('chart denom toggle re-fetches and re-renders', async ({ page }) => {
  89  |     await page.goto('/charts');
  90  |     const reqs: string[] = [];
  91  |     page.on('request', (req) => {
  92  |       const u = req.url();
  93  |       if (u.includes('/api/charts/account-value')) reqs.push(u);
  94  |     });
  95  |     await page.getByRole('button', { name: 'BTC', exact: true }).click();
  96  |     await page.waitForTimeout(800);
  97  |     await page.getByRole('button', { name: 'USD', exact: true }).click();
  98  |     await page.waitForTimeout(800);
  99  |     expect(reqs.length).toBeGreaterThanOrEqual(2);
  100 |   });
  101 | 
  102 |   test('competitors page highlights our maker row', async ({ page }) => {
  103 |     await page.goto('/competitors');
  104 |     // Wait for the table to render.
  105 |     await expect(page.locator('table')).toBeVisible({ timeout: 15_000 });
  106 |     // There must be exactly one row tagged as "us".
  107 |     const ourRow = page.locator('table tr[data-is-us="true"], table tr.is-us');
  108 |     await expect(ourRow).toHaveCount(1);
  109 |   });
  110 | 
  111 |   test('overview page has explanatory help text for unfamiliar tiles', async ({
  112 |     page,
  113 |   }) => {
  114 |     await page.goto('/');
  115 |     // Each of these tiles should be explained somewhere on the page — either
  116 |     // an inline subtitle or a help icon with a tooltip.
  117 |     for (const term of ['Onion', 'Rendezvous']) {
  118 |       const explanation = page
  119 |         .locator(`text=/${term}.*(reachable|registered|rendezvous|relay|hidden|peer)/i`)
  120 |         .first();
  121 |       await expect(explanation).toBeVisible();
  122 |     }
  123 |   });
  124 | 
  125 |   test('health page explains each non-OK status with a recommended action', async ({
  126 |     page,
  127 |   }) => {
  128 |     await page.goto('/health');
  129 |     await page.waitForTimeout(1000);
  130 |     // For any tile with the "degraded" badge, there should be a detail line
  131 |     // explaining what it means and what (if anything) to do.
  132 |     const degraded = page.locator('.badge-warn:has-text("degraded")');
  133 |     const n = await degraded.count();
  134 |     if (n === 0) return; // nothing to assert if everything is OK
  135 |     for (let i = 0; i < n; i++) {
  136 |       const tile = degraded.nth(i).locator('xpath=ancestor::div[contains(@class, "tile")]');
  137 |       // Detail line below the headline must exist and be non-empty.
  138 |       const detail = tile.locator('[data-role="detail"], .text-slate-500');
  139 |       await expect(detail.first()).toBeVisible();
  140 |     }
  141 |   });
  142 | });
  143 | 
```