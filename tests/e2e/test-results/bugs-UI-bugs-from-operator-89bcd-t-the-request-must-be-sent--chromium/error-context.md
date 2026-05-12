# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: bugs.spec.ts >> UI bugs from operator review >> chart period buttons cause a re-fetch (different period -> different sample count when data is sparse it can match, but the request must be sent)
- Location: bugs.spec.ts:71:3

# Error details

```
Test timeout of 60000ms exceeded while running "beforeEach" hook.
```

```
Error: page.waitForURL: Test timeout of 60000ms exceeded.
=========================== logs ===========================
waiting for navigation to "**/" until "load"
  navigated to "https://eigen.home.weaver-labs.xyz/login"
============================================================
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
      - heading "Sign in" [level=1] [ref=e18]
      - generic [ref=e19]:
        - generic [ref=e20]:
          - text: Password
          - textbox "Password" [active] [ref=e21]
        - button "Sign in" [ref=e22]
```

# Test source

```ts
  1  | import { Page, expect } from '@playwright/test';
  2  | import { readFileSync } from 'node:fs';
  3  | 
  4  | export function getAdminPassword(): string {
  5  |   // Pulls the locally-stored password from the operator's mac.
  6  |   try {
  7  |     return readFileSync('/tmp/eigen-admin-pass.txt', 'utf8').trim();
  8  |   } catch {
  9  |     throw new Error(
  10 |       'Missing /tmp/eigen-admin-pass.txt — these tests assume the operator has the dev-rotated password saved there.',
  11 |     );
  12 |   }
  13 | }
  14 | 
  15 | export async function login(page: Page): Promise<void> {
  16 |   const pw = getAdminPassword();
  17 |   // Bypass the UI form: directly POST credentials so the session cookie is
  18 |   // set, then navigate. Avoids racing the Leptos client-side redirect after
  19 |   // ActionForm submit.
  20 |   const res = await page.request.post('/api/auth/login', {
> 21 |     headers: { 'content-type': 'application/x-www-form-urlencoded' },
     |          ^ Error: page.waitForURL: Test timeout of 60000ms exceeded.
  22 |     form: { password: pw },
  23 |   });
  24 |   if (!res.ok()) throw new Error(`login failed: HTTP ${res.status()}`);
  25 |   await page.goto('/');
  26 | }
  27 | 
  28 | /** Hit a Leptos server-fn POST endpoint via the page's session cookie. */
  29 | export async function api<T = unknown>(page: Page, path: string): Promise<T> {
  30 |   const baseURL = (page.context() as any)._options?.baseURL ?? '';
  31 |   const res = await page.request.post(path, {
  32 |     headers: { 'content-type': 'application/cbor' },
  33 |     data: '',
  34 |   });
  35 |   if (!res.ok()) throw new Error(`${path} -> HTTP ${res.status()}`);
  36 |   return (await res.json()) as T;
  37 | }
  38 | 
```