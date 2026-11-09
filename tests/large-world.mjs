import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { isolatedServer } from './helpers/server.mjs';

const server = await isolatedServer({ requestTimeout: 30000 });
try {
  await server.boot();
  const gm = (await server.login('MJ')).cookie;
  const alice = (await server.login('Alice')).cookie;
  const baseline = (await server.request('/api/export', { cookie: gm })).data;
  for (const count of [1000, 10000]) {
    const world = structuredClone(baseline);
    world.actors = Array.from({ length: count }, (_, i) => ({
      id: `actor-${i}`,
      name: `Personnage ${i}`,
      owner: i % 2 ? 'Alice' : 'Bob',
      hp: 20,
      max_hp: 20,
      notes: 'Private campaign data '.repeat(16),
      items: [],
    }));
    const bytes = Buffer.byteLength(JSON.stringify(world));
    await server.request('/api/import', { cookie: gm, body: world });
    const times = [];
    for (let i = 0; i < 3; i++) {
      const start = performance.now();
      const view = await server.state(alice);
      times.push(performance.now() - start);
      assert.equal(view.world.actors.length, count);
      assert.equal(view.world.actors[0].notes, '');
      assert.equal(view.world.actors[1].notes, world.actors[1].notes);
    }
    const revision = (await server.state(gm)).revision;
    const results = await Promise.all(
      Array.from({ length: 8 }, async (_, i) => {
        const start = performance.now();
        const result = await server.action(gm, { kind: 'update_actor', id: `actor-${i}`, hp: i });
        times.push(performance.now() - start);
        return result;
      }),
    );
    assert.deepEqual(
      results.map((r) => r.revision).sort((a, b) => a - b),
      Array.from({ length: 8 }, (_, i) => revision + i + 1),
    );
    const final = await server.state(gm);
    for (let i = 0; i < 8; i++) assert.equal(final.world.actors[i].hp, i);
    assert.deepEqual(JSON.parse(await readFile(server.file, 'utf8')), final.world);
    times.sort((a, b) => a - b);
    console.log(
      `LOAD — ${count} acteurs, ${bytes} octets : médiane ${times[Math.floor(times.length / 2)].toFixed(0)} ms, p95 ${times[Math.ceil(times.length * 0.95) - 1].toFixed(0)} ms (lectures/actions ; indicatif, hors SLA).`,
    );
  }
  console.log(
    'PASS — mondes volumineux : projections privées, écritures concurrentes, révisions et sauvegarde cohérentes.',
  );
} finally {
  await server.dispose();
}
