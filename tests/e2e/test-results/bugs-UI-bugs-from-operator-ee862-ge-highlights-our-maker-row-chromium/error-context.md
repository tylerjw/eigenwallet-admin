# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: bugs.spec.ts >> UI bugs from operator review >> competitors page highlights our maker row
- Location: bugs.spec.ts:102:3

# Error details

```
Error: expect(locator).toHaveCount(expected) failed

Locator:  locator('table tr[data-is-us="true"], table tr.is-us')
Expected: 1
Received: 0
Timeout:  8000ms

Call log:
  - Expect "toHaveCount" with timeout 8000ms
  - waiting for locator('table tr[data-is-us="true"], table tr.is-us')
    20 × locator resolved to 0 elements
       - unexpected value "0"

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
      - generic [ref=e18]:
        - heading "Competitors" [level=1] [ref=e19]
        - button "Scan now" [ref=e20]
      - generic [ref=e21]:
        - generic [ref=e22]:
          - generic [ref=e23]: Our spread vs CEX mid
          - generic [ref=e24]: +2.39%
        - generic [ref=e25]:
          - generic [ref=e26]: Rank
          - generic [ref=e27]: "#5 of 11"
        - generic [ref=e28]:
          - generic [ref=e29]: Cheapest competitor
          - generic [ref=e30]: +-0.52%
      - generic [ref=e31]:
        - generic [ref=e32]: Scan manual • 2026-05-12 00:21:18
        - table [ref=e33]:
          - rowgroup [ref=e34]:
            - row "Peer Price (BTC/XMR) Min Max Spread vs CEX Status" [ref=e35]:
              - columnheader "Peer" [ref=e36]
              - columnheader "Price (BTC/XMR)" [ref=e37]
              - columnheader "Min" [ref=e38]
              - columnheader "Max" [ref=e39]
              - columnheader "Spread vs CEX" [ref=e40]
              - columnheader "Status" [ref=e41]
          - rowgroup [ref=e42]:
            - row "12D3KooW…grN3sx 0.0052416700 0.0000000000 0.0000000000 2.92 no quote" [ref=e43]:
              - cell "12D3KooW…grN3sx" [ref=e44]
              - cell "0.0052416700" [ref=e45]
              - cell "0.0000000000" [ref=e46]
              - cell "0.0000000000" [ref=e47]
              - cell "2.92" [ref=e48]
              - cell "no quote" [ref=e49]
            - row "12D3KooW…2roYXF 0.0052055300 0.0030000000 4.4752063800 2.21 ok" [ref=e50]:
              - cell "12D3KooW…2roYXF" [ref=e51]
              - cell "0.0052055300" [ref=e52]
              - cell "0.0030000000" [ref=e53]
              - cell "4.4752063800" [ref=e54]
              - cell "2.21" [ref=e55]
              - cell "ok" [ref=e56]
            - row "12D3KooW…6C9EEF 0.0053026300 0.0100000000 0.0500000000 4.12 ok" [ref=e57]:
              - cell "12D3KooW…6C9EEF" [ref=e58]
              - cell "0.0053026300" [ref=e59]
              - cell "0.0100000000" [ref=e60]
              - cell "0.0500000000" [ref=e61]
              - cell "4.12" [ref=e62]
              - cell "ok" [ref=e63]
            - row "12D3KooW…hK7BEa 0.0052141700 0.0050000000 0.1294487100 2.38 ok" [ref=e64]:
              - cell "12D3KooW…hK7BEa" [ref=e65]
              - cell "0.0052141700" [ref=e66]
              - cell "0.0050000000" [ref=e67]
              - cell "0.1294487100" [ref=e68]
              - cell "2.38" [ref=e69]
              - cell "ok" [ref=e70]
            - row "12D3KooW…KysX1r 0.0051853400 0.0020000000 2.5876328200 1.81 ok" [ref=e71]:
              - cell "12D3KooW…KysX1r" [ref=e72]
              - cell "0.0051853400" [ref=e73]
              - cell "0.0020000000" [ref=e74]
              - cell "2.5876328200" [ref=e75]
              - cell "1.81" [ref=e76]
              - cell "ok" [ref=e77]
            - row "12D3KooW…nFPPbR 0.0053872100 0.0100000000 0.1029534200 5.78 ok" [ref=e78]:
              - cell "12D3KooW…nFPPbR" [ref=e79]
              - cell "0.0053872100" [ref=e80]
              - cell "0.0100000000" [ref=e81]
              - cell "0.1029534200" [ref=e82]
              - cell "5.78" [ref=e83]
              - cell "ok" [ref=e84]
            - row "12D3KooW…7H5qm6 0.0062136400 0.0005000000 0.0668049200 22.00 ok" [ref=e85]:
              - cell "12D3KooW…7H5qm6" [ref=e86]
              - cell "0.0062136400" [ref=e87]
              - cell "0.0005000000" [ref=e88]
              - cell "0.0668049200" [ref=e89]
              - cell "22.00" [ref=e90]
              - cell "ok" [ref=e91]
            - row "12D3KooW…VkDGgF 0.0051877500 0.0000000000 0.0000000000 1.86 no quote" [ref=e92]:
              - cell "12D3KooW…VkDGgF" [ref=e93]
              - cell "0.0051877500" [ref=e94]
              - cell "0.0000000000" [ref=e95]
              - cell "0.0000000000" [ref=e96]
              - cell "1.86" [ref=e97]
              - cell "no quote" [ref=e98]
            - row "12D3KooW…RvtGoY 0.0050667600 0.0006000000 0.0019844500 -0.52 ok" [ref=e99]:
              - cell "12D3KooW…RvtGoY" [ref=e100]
              - cell "0.0050667600" [ref=e101]
              - cell "0.0006000000" [ref=e102]
              - cell "0.0019844500" [ref=e103]
              - cell "-0.52" [ref=e104]
              - cell "ok" [ref=e105]
            - row "12D3KooW…vNv1uL 0.0052413900 0.0000000000 0.0000000000 2.91 no quote" [ref=e106]:
              - cell "12D3KooW…vNv1uL" [ref=e107]
              - cell "0.0052413900" [ref=e108]
              - cell "0.0000000000" [ref=e109]
              - cell "0.0000000000" [ref=e110]
              - cell "2.91" [ref=e111]
              - cell "no quote" [ref=e112]
            - row "12D3KooW…U7ZTnT 0.0052818900 0.0050000000 0.8123447300 3.71 ok" [ref=e113]:
              - cell "12D3KooW…U7ZTnT" [ref=e114]
              - cell "0.0052818900" [ref=e115]
              - cell "0.0050000000" [ref=e116]
              - cell "0.8123447300" [ref=e117]
              - cell "3.71" [ref=e118]
              - cell "ok" [ref=e119]
            - row "12D3KooW…w8Fb2s 0.0052152300 0.0010000000 0.0500000000 2.40 ok" [ref=e120]:
              - cell "12D3KooW…w8Fb2s" [ref=e121]
              - cell "0.0052152300" [ref=e122]
              - cell "0.0010000000" [ref=e123]
              - cell "0.0500000000" [ref=e124]
              - cell "2.40" [ref=e125]
              - cell "ok" [ref=e126]
            - row "12D3KooW…ZSXRVj 0.0052365800 0.0000000000 0.0000000000 2.82 no quote" [ref=e127]:
              - cell "12D3KooW…ZSXRVj" [ref=e128]
              - cell "0.0052365800" [ref=e129]
              - cell "0.0000000000" [ref=e130]
              - cell "0.0000000000" [ref=e131]
              - cell "2.82" [ref=e132]
              - cell "no quote" [ref=e133]
            - row "12D3KooW…JUCsRp 0.0052715800 0.0080000000 0.0393284900 3.51 ok" [ref=e134]:
              - cell "12D3KooW…JUCsRp" [ref=e135]
              - cell "0.0052715800" [ref=e136]
              - cell "0.0080000000" [ref=e137]
              - cell "0.0393284900" [ref=e138]
              - cell "3.51" [ref=e139]
              - cell "ok" [ref=e140]
```

# Test source

```ts
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
> 108 |     await expect(ourRow).toHaveCount(1);
      |                          ^ Error: expect(locator).toHaveCount(expected) failed
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