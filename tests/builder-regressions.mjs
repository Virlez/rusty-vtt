import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { isolatedServer } from './helpers/server.mjs';
const server = await isolatedServer();
try {
  await server.boot();
  const gm = (await server.login('MJ')).cookie;
  await server.action(gm, { kind: 'module_toggle', id: 'dnd5e-character-builder', enabled: true });
  let state = await server.action(gm, { kind: 'builder_draft_create' });
  const draft = state.world.builder_drafts[0];
  await server.action(gm, {
    kind: 'builder_draft_save',
    id: draft.id,
    version: 0,
    data: { ...draft.data, name: 'Import fixture' },
  });
  await server.action(gm, { kind: 'builder_commit', id: draft.id, version: 1 });
  const world = (await server.request('/api/export', { cookie: gm })).data;
  const index = world.actors.findIndex((actor) => actor.system_id === 'dnd5e');
  const mutations = [
    (data) => {
      data.build = 42;
    },
    (data) => {
      data.build = null;
    },
    (data) => {
      delete data.build;
    },
    (data) => {
      data.build.scores = [];
    },
    (data) => {
      data.build.scores.str = Number.MAX_SAFE_INTEGER;
    },
    (data) => {
      data.classes = [];
    },
    (data) => {
      data.classes[0].id = 'unknown';
    },
    (data) => {
      data.classes[0] = 'guerrier';
    },
    (data) => {
      data.classes[0].level = 0;
    },
    (data) => {
      data.classes.push(structuredClone(data.classes[0]));
    },
    (data) => {
      data.level = 21;
    },
    (data) => {
      data.level = 2;
    },
    (data) => {
      data.hp_dice[0].die = 0;
    },
    (data) => {
      delete data.hp_dice[0].value;
    },
    (data) => {
      data.hp_dice[0].value = '10';
    },
    (data) => {
      data.hit_dice_spent = -1;
    },
    (data) => {
      data.abilities.con = Number.MAX_SAFE_INTEGER;
    },
  ];
  for (const mutate of mutations) {
    const malformed = structuredClone(world);
    mutate(malformed.actors[index].dnd5e);
    const before = await server.state(gm),
      disk = await readFile(server.file, 'utf8');
    await server.request('/api/import', { cookie: gm, body: malformed, status: 400 });
    assert.deepEqual(await server.state(gm), before);
    assert.equal(await readFile(server.file, 'utf8'), disk);
  }
  for (const change of [
    (candidate) => {
      candidate.builder_settings = 42;
    },
    (candidate) => {
      candidate.builder_settings = { bonus_gold: Number.MAX_SAFE_INTEGER };
    },
    (candidate) => {
      candidate.builder_settings = { allow_manual: 'true' };
    },
    (candidate) => {
      candidate.builder_drafts[0].version = Number.MAX_SAFE_INTEGER;
    },
    (candidate) => {
      candidate.builder_drafts[0].hp_roll = 0;
    },
    (candidate) => {
      candidate.actors[index].hp = Number.MAX_SAFE_INTEGER;
    },
    (candidate) => {
      candidate.builder_drafts[0].base_actor = { ...candidate.actors[index], dnd5e: { build: 42 } };
    },
  ]) {
    const malformed = structuredClone(world);
    change(malformed);
    const before = await server.state(gm),
      disk = await readFile(server.file, 'utf8');
    await server.request('/api/import', { cookie: gm, body: malformed, status: 400 });
    assert.deepEqual(await server.state(gm), before);
    assert.equal(await readFile(server.file, 'utf8'), disk);
  }
  await server.request('/api/import', { cookie: gm, body: world });
  const level = await server.action(gm, {
    kind: 'builder_level_start',
    actor_id: world.actors[index].id,
  });
  assert.equal(level.world.builder_drafts.at(-1).mode, 'levelup');
  console.log(
    'PASS — imports 5e : structures imbriquées, classes/niveaux, dés de vie et refus atomiques ; progression valide préservée.',
  );
} finally {
  await server.dispose();
}
