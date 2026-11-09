import assert from 'node:assert/strict';
import { mkdir, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { isolatedServer } from './helpers/server.mjs';

const server = await isolatedServer();
try {
  // Invalid JSON and valid JSON with an invalid schema must fail closed,
  // preserving the exact bytes for recovery instead of replacing them with demo().
  for (const [bytes, diagnostic] of [
    ['{"name":"truncated', /Sauvegarde JSON invalide/],
    ['{"name":"bad schema"}', /Structure de sauvegarde invalide/],
  ]) {
    await writeFile(server.file, bytes);
    await server.boot({ failure: diagnostic });
    assert.equal(await readFile(server.file, 'utf8'), bytes);
    await server.stop();
  }
  await rm(server.file);
  await server.boot();
  let gm = (await server.login('MJ')).cookie;
  await server.action(gm, { kind: 'chat', text: 'durable baseline' });
  const baseline = (await server.request('/api/export', { cookie: gm })).data;
  // Exact export/import round trip, then actual process restart.
  await server.request('/api/import', { cookie: gm, body: baseline });
  assert.deepEqual((await server.request('/api/export', { cookie: gm })).data, baseline);
  await server.stop();
  await server.boot();
  gm = (await server.login('MJ')).cookie;
  assert.deepEqual((await server.state(gm)).world, baseline);

  const tmp = join(server.dir, 'world.tmp');
  const stream = await server.events(gm);
  try {
    for (const failure of ['write', 'rename']) {
      for (const endpoint of ['/api/action', '/api/import']) {
        const before = await server.state(gm);
        const disk = await readFile(server.file, 'utf8');
        const imported = structuredClone(before.world);
        imported.name = 'MUST_NOT_COMMIT';
        const body = endpoint === '/api/action' ? { kind: 'chat', text: 'MUST_NOT_COMMIT' } : imported;
        if (failure === 'write') await mkdir(tmp);
        else { await rm(server.file); await mkdir(server.file); }
        try {
          await server.request(endpoint, { cookie: gm, body, status: 500 });
          assert.deepEqual(await server.state(gm), before, `${failure}/${endpoint}: world or revision changed`);
          if (failure === 'write') assert.equal(await readFile(server.file, 'utf8'), disk);
          else assert.equal((await stat(server.file)).isDirectory(), true);
        } finally {
          await rm(tmp, { recursive: true, force: true });
          if (failure === 'rename') {
            await rm(server.file, { recursive: true });
            await writeFile(server.file, disk);
          }
        }
        // If a failed operation emitted an event, it will be ahead of this one.
        const recovered = await server.action(gm, { kind: 'chat', text: `recovered ${failure} ${endpoint}` });
        assert.equal(recovered.revision, before.revision + 1);
        assert.equal(await stream.next(), `data: ${recovered.revision}`);
        assert.deepEqual(JSON.parse(await readFile(server.file, 'utf8')), recovered.world);
      }
    }
  } finally { stream.close(); }

  // Simultaneous requests must serialize without lost appends or duplicate revisions.
  const before = await server.state(gm);
  const names = Array.from({ length: 12 }, (_, i) => `Concurrent journal ${i}`);
  const results = await Promise.all(names.map(name => server.action(gm, { kind: 'add_journal', name, content: name, public: true })));
  assert.deepEqual(results.map(result => result.revision).sort((a, b) => a - b), names.map((_, i) => before.revision + i + 1));
  const final = await server.state(gm);
  assert.equal(final.revision, before.revision + names.length);
  for (const name of names) assert.equal(final.world.journals.filter(entry => entry.name === name).length, 1);
  assert.equal(new Set(final.world.journals.map(entry => entry.id)).size, final.world.journals.length);
  assert.deepEqual(JSON.parse(await readFile(server.file, 'utf8')), final.world);
  await server.stop();
  await server.boot();
  gm = (await server.login('MJ')).cookie;
  assert.deepEqual((await server.state(gm)).world, final.world);
  assert.deepEqual((await server.request('/api/export', { cookie: gm })).data, final.world);
  console.log('PASS — persistance : corruption préservée, export/import, échecs écriture/renommage atomiques, reprise, concurrence et redémarrage.');
} finally { await server.dispose(); }
