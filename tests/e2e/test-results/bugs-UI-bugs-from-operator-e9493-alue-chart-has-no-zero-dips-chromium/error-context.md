# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: bugs.spec.ts >> UI bugs from operator review >> account-value chart has no zero dips
- Location: bugs.spec.ts:25:3

# Error details

```
Error: expect(received).toBeGreaterThan(expected)

Expected: > 100
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
        - generic [ref=e23]:
          - generic [ref=e24]: XMR balance
          - generic [ref=e25]: 22.294433 XMR
        - generic [ref=e26]:
          - generic [ref=e27]: Total (USD)
          - generic [ref=e28]: $18693.49
        - generic [ref=e29]:
          - generic [ref=e30]: Active swaps
          - generic [ref=e31]: "0"
        - generic [ref=e32]:
          - generic [ref=e33]: Peers
          - generic [ref=e34]: "24"
        - generic [ref=e35]:
          - generic [ref=e36]: Rendezvous
          - generic [ref=e37]: 4/8
        - generic [ref=e38]:
          - generic [ref=e39]: Spread
          - generic [ref=e40]: +2.39%
        - generic [ref=e41]:
          - generic [ref=e42]: Onion
          - generic [ref=e43]: —
      - paragraph [ref=e44]: Last updated 2026-05-12T00:27:35.865290923+00:00
      - generic [ref=e45]:
        - generic [ref=e46]: Total value (USD, 7d)
        - generic [ref=e47]:
          - img [ref=e49]:
            - generic "2026-05-11 21:02 UTC • 0.00" [ref=e51]
            - generic "2026-05-11 21:05 UTC • 0.00" [ref=e52]
            - generic "2026-05-11 21:06 UTC • 0.00" [ref=e53]
            - generic "2026-05-11 21:11 UTC • 13,965" [ref=e54]
            - generic "2026-05-11 21:16 UTC • 13,950" [ref=e55]
            - generic "2026-05-11 21:21 UTC • 13,959" [ref=e56]
            - generic "2026-05-11 21:26 UTC • 13,958" [ref=e57]
            - generic "2026-05-11 21:31 UTC • 13,956" [ref=e58]
            - generic "2026-05-11 21:32 UTC • 0.00" [ref=e59]
            - generic "2026-05-11 21:37 UTC • 18,769" [ref=e60]
            - generic "2026-05-11 21:40 UTC • 0.00" [ref=e61]
            - generic "2026-05-11 21:45 UTC • 18,723" [ref=e62]
            - generic "2026-05-11 21:50 UTC • 18,720" [ref=e63]
            - generic "2026-05-11 21:55 UTC • 18,710" [ref=e64]
            - generic "2026-05-11 21:56 UTC • 0.00" [ref=e65]
            - generic "2026-05-11 21:59 UTC • 0.00" [ref=e66]
            - generic "2026-05-11 22:04 UTC • 18,758" [ref=e67]
            - generic "2026-05-11 22:09 UTC • 18,712" [ref=e68]
            - generic "2026-05-11 22:14 UTC • 18,726" [ref=e69]
            - generic "2026-05-11 22:19 UTC • 18,754" [ref=e70]
            - generic "2026-05-11 22:24 UTC • 18,753" [ref=e71]
            - generic "2026-05-11 22:29 UTC • 18,730" [ref=e72]
            - generic "2026-05-11 22:34 UTC • 18,701" [ref=e73]
            - generic "2026-05-11 22:39 UTC • 18,694" [ref=e74]
            - generic "2026-05-11 22:44 UTC • 18,734" [ref=e75]
            - generic "2026-05-11 22:49 UTC • 18,758" [ref=e76]
            - generic "2026-05-11 22:54 UTC • 18,810" [ref=e77]
            - generic "2026-05-11 22:59 UTC • 18,818" [ref=e78]
            - generic "2026-05-11 23:04 UTC • 18,753" [ref=e79]
            - generic "2026-05-11 23:09 UTC • 18,793" [ref=e80]
            - generic "2026-05-11 23:14 UTC • 18,789" [ref=e81]
            - generic "2026-05-11 23:16 UTC • 0.00" [ref=e82]
            - generic "2026-05-11 23:21 UTC • 18,788" [ref=e83]
            - generic "2026-05-11 23:21 UTC • 0.00" [ref=e84]
            - generic "2026-05-11 23:26 UTC • 18,785" [ref=e85]
            - generic "2026-05-11 23:31 UTC • 18,819" [ref=e86]
            - generic "2026-05-11 23:36 UTC • 18,838" [ref=e87]
            - generic "2026-05-11 23:41 UTC • 18,802" [ref=e88]
            - generic "2026-05-11 23:46 UTC • 18,785" [ref=e89]
            - generic "2026-05-11 23:51 UTC • 18,779" [ref=e90]
            - generic "2026-05-11 23:56 UTC • 18,759" [ref=e91]
            - generic "2026-05-12 00:01 UTC • 18,759" [ref=e92]
            - generic "2026-05-12 00:06 UTC • 18,782" [ref=e93]
            - generic "2026-05-12 00:11 UTC • 18,780" [ref=e94]
            - generic "2026-05-12 00:16 UTC • 18,725" [ref=e95]
            - generic "2026-05-12 00:21 UTC • 18,705" [ref=e96]
            - generic "2026-05-12 00:26 UTC • 18,693" [ref=e97]
          - generic [ref=e98]: 47 samples • latest $18693.84 • hover a point for the exact value
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
> 32  |     expect(min).toBeGreaterThan(100);
      |                 ^ Error: expect(received).toBeGreaterThan(expected)
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
  49  |     expect(withProfit.length).toBeGreaterThan(0);
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
```