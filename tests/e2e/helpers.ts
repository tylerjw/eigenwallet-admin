import { Page, expect } from '@playwright/test';
import { readFileSync } from 'node:fs';

export function getAdminPassword(): string {
  // Pulls the locally-stored password from the operator's mac.
  try {
    return readFileSync('/tmp/eigen-admin-pass.txt', 'utf8').trim();
  } catch {
    throw new Error(
      'Missing /tmp/eigen-admin-pass.txt — these tests assume the operator has the dev-rotated password saved there.',
    );
  }
}

export async function login(page: Page): Promise<void> {
  const pw = getAdminPassword();
  // Bypass the UI form: directly POST credentials so the session cookie is
  // set, then navigate. Avoids racing the Leptos client-side redirect after
  // ActionForm submit.
  const res = await page.request.post('/api/auth/login', {
    headers: { 'content-type': 'application/x-www-form-urlencoded' },
    form: { password: pw },
  });
  if (!res.ok()) throw new Error(`login failed: HTTP ${res.status()}`);
  await page.goto('/');
}

/** Hit a Leptos server-fn POST endpoint via the page's session cookie. */
export async function api<T = unknown>(page: Page, path: string): Promise<T> {
  const baseURL = (page.context() as any)._options?.baseURL ?? '';
  const res = await page.request.post(path, {
    headers: { 'content-type': 'application/cbor' },
    data: '',
  });
  if (!res.ok()) throw new Error(`${path} -> HTTP ${res.status()}`);
  return (await res.json()) as T;
}
