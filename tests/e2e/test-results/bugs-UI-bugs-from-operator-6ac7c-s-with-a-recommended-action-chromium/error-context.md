# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: bugs.spec.ts >> UI bugs from operator review >> health page explains each non-OK status with a recommended action
- Location: bugs.spec.ts:125:3

# Error details

```
Error: expect(locator).toBeVisible() failed

Locator: locator('.badge-warn:has-text("degraded")').first().locator('xpath=ancestor::div[contains(@class, "tile")]').locator('[data-role="detail"], .text-slate-500').first()
Expected: visible
Timeout: 8000ms
Error: element(s) not found

Call log:
  - Expect "toBeVisible" with timeout 8000ms
  - waiting for locator('.badge-warn:has-text("degraded")').first().locator('xpath=ancestor::div[contains(@class, "tile")]').locator('[data-role="detail"], .text-slate-500').first()

```

```yaml
- banner:
  - link "eigenwallet admin":
    - /url: /
  - link "Overview":
    - /url: /
  - link "Health":
    - /url: /health
  - link "Swaps":
    - /url: /swaps
  - link "Charts":
    - /url: /charts
  - link "Spread":
    - /url: /spread
  - link "Competitors":
    - /url: /competitors
  - link "ROI":
    - /url: /roi
  - link "Wallet rules":
    - /url: /wallet-rules
  - button "Logout"
- main:
  - heading "Health" [level=1]
  - text: asb ok reachable http://asb:9944 bitcoind ok tip 949013 via electrs (bitcoind RPC not exposed) monerod ok height 3672082 / 3672082 synced electrs ok tip 949013 tor degraded Bootstrapping peers ok 25 peers rendezvous degraded 4/8 registered admin-db ok reachable
  - paragraph: As of 2026-05-12T00:28:22.879409251+00:00
```

# Test source

```ts
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
  133 |     const n = await degraded.count();
  134 |     if (n === 0) return; // nothing to assert if everything is OK
  135 |     for (let i = 0; i < n; i++) {
  136 |       const tile = degraded.nth(i).locator('xpath=ancestor::div[contains(@class, "tile")]');
  137 |       // Detail line below the headline must exist and be non-empty.
  138 |       const detail = tile.locator('[data-role="detail"], .text-slate-500');
> 139 |       await expect(detail.first()).toBeVisible();
      |                                    ^ Error: expect(locator).toBeVisible() failed
  140 |     }
  141 |   });
  142 | });
  143 | 
```