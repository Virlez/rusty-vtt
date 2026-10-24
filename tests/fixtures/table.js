import { test as base, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { TablePage } from '../pages/table.js';

export const test = base.extend({
  table: async ({ browser }, use) => {
    const dir = await mkdtemp(join(tmpdir(), 'rusty-e2e-'));
    const server = spawn(resolve(process.env.RUSTY_TEST_BINARY || `target/debug/rusty-vtt${process.platform === 'win32' ? '.exe' : ''}`), [], {
      windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'],
      env: { ...process.env, RUSTY_BIND: '127.0.0.1:0', RUSTY_DATA: join(dir, 'world.json'),
        RUSTY_GM_PASSWORD: 'e2e-gm-secret-123456', RUSTY_PLAYERS_JSON: JSON.stringify({ Alice: 'e2e-alice-secret-123456' }), RUSTY_HTTPS: '0' },
    });
    let output = '';
    const contexts = [];
    try {
      const url = await new Promise((resolveUrl, reject) => {
        const timer = setTimeout(() => reject(Error(`Startup timeout: ${output}`)), 15000);
        server.once('error', e => { clearTimeout(timer); reject(e); });
        server.once('exit', code => { clearTimeout(timer); reject(Error(`Server exited ${code}: ${output}`)); });
        const read = chunk => {
          output += chunk;
          const match = output.match(/http:\/\/(127\.0\.0\.1:\d+)/);
          if (match) { clearTimeout(timer); resolveUrl(`http://${match[1]}`); }
        };
        server.stdout.on('data', read);
        server.stderr.on('data', read);
      });
      await use({
        url,
        async open(role = 'gm') {
          const context = await browser.newContext({ baseURL: url, viewport: { width: 1440, height: 1000 } });
          contexts.push(context);
          const page = await context.newPage();
          const app = new TablePage(page);
          await page.goto('/');
          await app.login(role);
          return app;
        },
      });
    } finally {
      await Promise.all(contexts.map(c => c.close()));
      if (server.exitCode === null) await new Promise(resolveExit => { server.once('exit', resolveExit); server.kill(); });
      await rm(dir, { recursive: true, force: true });
    }
  },
});
export { expect };
