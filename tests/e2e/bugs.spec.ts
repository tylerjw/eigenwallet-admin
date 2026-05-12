// Regression tests for issues caught during operator UI review on 2026-05-12.
// Each test is named after the bug and asserts the *fixed* behavior, so the
// suite goes red before the fix lands and green once it does.

import { test, expect } from '@playwright/test';
import { login, api } from './helpers';

test.describe('UI bugs from operator review', () => {
  test.beforeEach(async ({ page }) => {
    await login(page);
  });

  test('attribution chart has no zero-total dips', async ({ page }) => {
    const dto = await api<{
      actual: { t: string; v: string }[];
      sample_count: number;
    }>(page, '/api/charts/attribution');
    expect(dto.sample_count).toBeGreaterThan(2);
    const minActual = Math.min(...dto.actual.map((p) => parseFloat(p.v)));
    // All snapshots have non-zero balance; total should always be a real USD
    // value. The bug manifested as a few dips to $0 from CEX cache misses.
    expect(minActual).toBeGreaterThan(100);
  });

  test('account-value chart has no zero dips', async ({ page }) => {
    const dto = await api<{ points: { t: string; v: string }[] }>(
      page,
      '/api/charts/account-value',
    );
    if (dto.points.length === 0) return;
    const min = Math.min(...dto.points.map((p) => parseFloat(p.v)));
    expect(min).toBeGreaterThan(100);
  });

  test('swap rows have a profit_usd value populated for completed swaps', async ({
    page,
  }) => {
    const dto = await api<{ rows: { state: string; profit_usd: string | null }[] }>(
      page,
      '/api/swaps',
    );
    const completed = dto.rows.filter((r) =>
      r.state.toLowerCase().includes('redeemed'),
    );
    expect(completed.length).toBeGreaterThan(0);
    // At least *some* completed swaps must have a profit number — the bug was
    // that every row had profit_usd: null.
    const withProfit = completed.filter((r) => r.profit_usd !== null);
    expect(withProfit.length).toBeGreaterThan(0);
  });

  test('swap state filter "completed" returns non-zero rows', async ({ page }) => {
    await page.goto('/swaps');
    await page.getByRole('button', { name: 'completed', exact: true }).click();
    await page.waitForTimeout(500);
    const rowCount = await page.locator('table tbody tr').count();
    expect(rowCount).toBeGreaterThan(0);
  });

  test('swap state filter "refunded" returns rows when there are refunds', async ({
    page,
  }) => {
    // There are known refunded swaps in the DB; filter should surface them.
    await page.goto('/swaps');
    await page.getByRole('button', { name: 'refunded', exact: true }).click();
    await page.waitForTimeout(500);
    const rowCount = await page.locator('table tbody tr').count();
    expect(rowCount).toBeGreaterThan(0);
  });

  test('chart period buttons cause a re-fetch (different period -> different sample count when data is sparse it can match, but the request must be sent)', async ({
    page,
  }) => {
    await page.goto('/charts');
    const reqs: string[] = [];
    page.on('request', (req) => {
      if (req.url().includes('/api/charts/account-value')) reqs.push(req.url());
    });
    await page.getByRole('button', { name: '24h', exact: true }).click();
    await page.waitForTimeout(800);
    await page.getByRole('button', { name: '30d', exact: true }).click();
    await page.waitForTimeout(800);
    // We should see at least two requests after the page loaded (the initial
    // is racey).
    expect(reqs.length).toBeGreaterThan(1);
  });

  test('chart denom toggle re-fetches and re-renders', async ({ page }) => {
    await page.goto('/charts');
    const reqs: string[] = [];
    page.on('request', (req) => {
      const u = req.url();
      if (u.includes('/api/charts/account-value')) reqs.push(u);
    });
    await page.getByRole('button', { name: 'BTC', exact: true }).click();
    await page.waitForTimeout(800);
    await page.getByRole('button', { name: 'USD', exact: true }).click();
    await page.waitForTimeout(800);
    expect(reqs.length).toBeGreaterThanOrEqual(2);
  });

  test('competitors page highlights our maker row', async ({ page }) => {
    await page.goto('/competitors');
    // Wait for the table to render.
    await expect(page.locator('table')).toBeVisible({ timeout: 15_000 });
    // There must be exactly one row tagged as "us".
    const ourRow = page.locator('table tr[data-is-us="true"], table tr.is-us');
    await expect(ourRow).toHaveCount(1);
  });

  test('overview page has explanatory help text for unfamiliar tiles', async ({
    page,
  }) => {
    await page.goto('/');
    // Each of these tiles should be explained somewhere on the page — either
    // an inline subtitle or a help icon with a tooltip.
    for (const term of ['Onion', 'Rendezvous']) {
      const explanation = page
        .locator(`text=/${term}.*(reachable|registered|rendezvous|relay|hidden|peer)/i`)
        .first();
      await expect(explanation).toBeVisible();
    }
  });

  test('health page explains each non-OK status with a recommended action', async ({
    page,
  }) => {
    await page.goto('/health');
    await page.waitForTimeout(1000);
    // For any tile with the "degraded" badge, there should be a detail line
    // explaining what it means and what (if anything) to do.
    const degraded = page.locator('.badge-warn:has-text("degraded")');
    const n = await degraded.count();
    if (n === 0) return; // nothing to assert if everything is OK
    for (let i = 0; i < n; i++) {
      const tile = degraded.nth(i).locator('xpath=ancestor::div[contains(@class, "tile")]');
      // Detail line below the headline must exist and be non-empty.
      const detail = tile.locator('[data-role="detail"], .text-slate-500');
      await expect(detail.first()).toBeVisible();
    }
  });
});
