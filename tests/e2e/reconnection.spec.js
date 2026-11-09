import { createServer, request } from 'node:http';
import { test, expect } from '../fixtures/table.js';

// Forward real HTTP/SSE traffic through a player-only loopback relay. Pause
// drops its stream and rejects HTTP refreshes; resume releases pending native
// EventSource reconnects. No world response or EventSource implementation is mocked.
async function eventRelay(upstream) {
  let paused = false;
  const streams = new Set(), pending = new Set();
  const server = createServer((incoming, outgoing) => {
    const events = incoming.url === '/api/events';
    const forward = () => {
      if (outgoing.destroyed) return;
      const forwarded = request(new URL(incoming.url, upstream), { method: incoming.method, headers: incoming.headers });
      const disconnect = () => { forwarded.destroy(); outgoing.destroy(); };
      if (events) streams.add(disconnect);
      outgoing.on('close', () => { streams.delete(disconnect); forwarded.destroy(); });
      forwarded.on('error', () => outgoing.destroy());
      forwarded.on('response', response => {
        outgoing.writeHead(response.statusCode, response.headers);
        outgoing.flushHeaders();
        response.on('error', () => outgoing.destroy());
        response.pipe(outgoing);
      });
      if (incoming.method === 'GET' || incoming.method === 'HEAD') forwarded.end();
      else incoming.pipe(forwarded);
    };
    if (!paused) forward();
    else if (events) {
      pending.add(forward);
      outgoing.once('close', () => pending.delete(forward));
    } else outgoing.writeHead(503).end();
  });
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  return {
    url: `http://127.0.0.1:${server.address().port}`,
    get connections() { return streams.size; },
    pause() { paused = true; for (const disconnect of streams) disconnect(); },
    resume() { paused = false; for (const forward of pending) forward(); pending.clear(); },
    async close() {
      const closed = new Promise(resolve => server.close(resolve));
      server.closeAllConnections();
      await closed;
    },
  };
}

test('reconnexion SSE : rattrapage et confidentialité après interruption du flux', async ({ table }) => {
  // Firefox may wait for the server's 15-second SSE keep-alive before onopen.
  test.setTimeout(45000);
  const relay = await eventRelay(table.url);
  try {
    const gm = await table.open(), player = await table.open('player', { baseURL: relay.url });
    await gm.chat('Avant la coupure');
    await expect(player.panel.getByText('Avant la coupure', { exact: true })).toBeVisible();
    await expect.poll(() => relay.connections).toBe(1);

    relay.pause();
    await expect(player.page.locator('#connection-status')).toHaveText('Reconnexion…');
    await gm.chat('Pendant la coupure');
    await gm.chat('/selfroll 1d6');
    const privateRoll = (await gm.state()).world.messages.at(-1);
    await expect(player.panel.getByText('Pendant la coupure', { exact: true })).toHaveCount(0);

    relay.resume();
    // No reload and no later action: onopen must fetch the missed revisions.
    await expect(player.panel.getByText('Pendant la coupure', { exact: true })).toBeVisible({ timeout: 30000 });
    await expect(player.page.locator('#connection-status')).toHaveText('Table synchronisée');
    await expect(player.page.locator('#identity')).toContainText('Alice');
    const messages = (await player.state()).world.messages;
    expect(messages.map(message => message.text)).toEqual(['Avant la coupure', 'Pendant la coupure']);
    expect(messages.some(message => message.id === privateRoll.id)).toBe(false);
    await expect(player.panel.locator('.roll')).toHaveCount(0);

    await gm.chat('Après la reconnexion');
    await expect(player.panel.getByText('Après la reconnexion', { exact: true })).toBeVisible();
    await player.chat('De retour');
    await expect(gm.panel.getByText('De retour', { exact: true })).toBeVisible();
  } finally { await relay.close(); }
});
