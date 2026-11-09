import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { isolatedServer } from './helpers/server.mjs';

const server = await isolatedServer();
const ids = (list) => list.map((entry) => entry.id);
try {
  await server.boot();
  const gm = (await server.login('MJ')).cookie;
  await server.request('/api/events', { status: 401 });
  const original = (await server.request('/api/export', { cookie: gm })).data;
  const world = structuredClone(original);
  world.actors = ['Alice', 'Bob'].map((owner) => ({
    id: owner,
    name: owner,
    owner,
    hp: 12,
    max_hp: 20,
    notes: `PRIVATE_${owner}_NOTES`,
    foundry: { secret: `PRIVATE_${owner}_FOUNDRY` },
    items: [{ id: `${owner}-item`, name: `PRIVATE_${owner}_ITEM` }],
    dnd5e: { level: 2, build: { secret: `PRIVATE_${owner}_BUILD` } },
  }));
  world.builder_drafts = ['Alice', 'Bob'].map((owner) => ({
    id: `${owner}-draft`,
    owner,
    data: { secret: `PRIVATE_${owner}_DRAFT` },
  }));
  world.journals = [
    { id: 'public-journal', public: true, content: 'Public' },
    { id: 'private-journal', public: false, content: 'GM_JOURNAL_SECRET' },
  ];
  world.items = [
    { id: 'public-item', public: true },
    { id: 'private-item', public: false, name: 'GM_ITEM_SECRET' },
  ];
  world.tables = [
    { id: 'public-table', public: true },
    { id: 'private-table', public: false, name: 'GM_TABLE_SECRET' },
  ];
  world.messages = ['public', 'gm', 'blind', 'self', 'whisper'].map((mode) => ({
    id: mode,
    author: 'Alice',
    mode,
    recipients: ['Bob'],
    text: `MESSAGE_${mode}`,
  }));
  // Also exercise an inactive scene: secrets must not be filtered only in active_scene.
  world.scenes = [world.scenes[0], { ...structuredClone(world.scenes[0]), id: 'inactive' }];
  for (const [index, scene] of world.scenes.entries()) {
    scene.vision_enabled = true;
    scene.tokens = [
      { id: `source-${index}`, name: 'Source', owner: 'Alice', x: 100, y: 100, vision_cells: 2 },
      { id: `near-${index}`, name: 'Near', owner: 'MJ', x: 150, y: 100, vision_cells: 0 },
      { id: `far-${index}`, name: 'VISION_SECRET', owner: 'MJ', x: 900, y: 900, vision_cells: 0 },
      {
        id: `hidden-${index}`,
        name: 'HIDDEN_SECRET',
        owner: 'Alice',
        x: 110,
        y: 100,
        hidden: true,
      },
      { id: `fog-${index}`, name: 'FOG_SECRET', owner: 'Alice', x: 120, y: 100 },
    ];
    scene.fog = [{ x: 120, y: 100, w: 1, h: 1 }];
    scene.notes = [
      { id: 'near-note', x: 150, y: 100, text: 'Public note' },
      { id: 'fog-note', x: 120, y: 100, text: 'FOG_NOTE_SECRET' },
      { id: 'far-note', x: 900, y: 900, text: 'VISION_NOTE_SECRET' },
    ];
  }
  world.combat = {
    order: ['hidden-0', 'far-0', 'near-0', 'source-0'].map((id) => ({ id, name: id })),
    turn: 0,
    round: 3,
  };
  await server.request('/api/import', { cookie: gm, body: world });
  const expectedMessages = {
    MJ: ['public', 'gm', 'blind'],
    Alice: ['public', 'gm', 'self', 'whisper'],
    Bob: ['public', 'whisper'],
    Eve: ['public'],
  };
  function check(name, state) {
    const view = state.world;
    const serialized = JSON.stringify(state);
    for (const message of world.messages.filter(
      (message) => !expectedMessages[name].includes(message.id),
    )) {
      assert.equal(serialized.includes(message.text), false, `${name}: leaked private message`);
    }
    if (name !== 'MJ') {
      for (const secret of [
        'GM_JOURNAL_SECRET',
        'GM_ITEM_SECRET',
        'GM_TABLE_SECRET',
        'HIDDEN_SECRET',
        'FOG_SECRET',
        'VISION_SECRET',
        'FOG_NOTE_SECRET',
        'VISION_NOTE_SECRET',
      ]) {
        assert.equal(serialized.includes(secret), false, `${name}: leaked ${secret}`);
      }
      for (const owner of ['Alice', 'Bob'].filter((owner) => owner !== name)) {
        assert.equal(
          serialized.includes(`PRIVATE_${owner}_`),
          false,
          `${name}: leaked ${owner}'s private data`,
        );
      }
    }
    assert.equal(state.user.name, name);
    assert.equal(state.user.role, name === 'MJ' ? 'gm' : 'player');
    assert.deepEqual(ids(view.messages), expectedMessages[name]);
    assert.deepEqual(
      view.messages,
      world.messages.filter((message) => expectedMessages[name].includes(message.id)),
    );
    for (const actor of view.actors) {
      const source = world.actors.find((entry) => entry.id === actor.id);
      if (name === 'MJ' || name === actor.owner) assert.deepEqual(actor, source);
      else {
        const publicFields = { ...source };
        delete publicFields.foundry;
        assert.deepEqual(actor, { ...publicFields, notes: '', items: [], dnd5e: { level: 2 } });
      }
    }
    assert.deepEqual(
      view.builder_drafts,
      world.builder_drafts.filter((draft) => name === 'MJ' || draft.owner === name),
    );
    for (const key of ['journals', 'items', 'tables']) {
      assert.deepEqual(
        view[key],
        world[key].filter((entry) => name === 'MJ' || entry.public),
      );
    }
    for (const [index, scene] of view.scenes.entries()) {
      assert.deepEqual(
        ids(scene.tokens),
        name === 'MJ'
          ? ids(world.scenes[index].tokens)
          : name === 'Alice'
            ? [`source-${index}`, `near-${index}`]
            : [],
      );
      const tokenIds =
        name === 'MJ'
          ? ids(world.scenes[index].tokens)
          : name === 'Alice'
            ? [`source-${index}`, `near-${index}`]
            : [];
      const noteIds =
        name === 'MJ'
          ? ['near-note', 'fog-note', 'far-note']
          : name === 'Alice'
            ? ['near-note']
            : [];
      assert.deepEqual(
        scene.tokens,
        world.scenes[index].tokens.filter((token) => tokenIds.includes(token.id)),
      );
      assert.deepEqual(
        scene.notes,
        world.scenes[index].notes.filter((note) => noteIds.includes(note.id)),
      );
    }
    assert.deepEqual(
      ids(view.combat.order),
      name === 'MJ' ? ids(world.combat.order) : name === 'Alice' ? ['near-0', 'source-0'] : [],
    );
    assert.equal(view.combat.turn, 0);
    if (name === 'MJ')
      assert.deepEqual(view, {
        ...world,
        messages: world.messages.filter((message) => expectedMessages.MJ.includes(message.id)),
      });
  }
  const sessions = {};
  for (const name of Object.keys(expectedMessages)) {
    const login = await server.login(name);
    sessions[name] = login.cookie;
    check(name, login.data); // Login itself returns a full snapshot.
    check(name, await server.state(login.cookie));
    if (name !== 'MJ') await server.request('/api/export', { cookie: login.cookie, status: 403 });
  }
  const streams = await Promise.all(Object.values(sessions).map((cookie) => server.events(cookie)));
  try {
    const result = await server.action(gm, { kind: 'chat', text: 'SSE_GM_SECRET', mode: 'gm' });
    world.messages = (await server.request('/api/export', { cookie: gm })).data.messages;
    for (const name of ['MJ']) expectedMessages[name].push(world.messages.at(-1).id);
    const frames = await Promise.all(streams.map((stream) => stream.next()));
    for (const frame of frames)
      assert.equal(
        frame,
        `data: ${result.revision}`,
        'SSE must contain only a revision, never world data',
      );
    for (const [name, cookie] of Object.entries(sessions)) check(name, await server.state(cookie));
    // Action responses must also use the caller's projection.
    const aliceResult = await server.action(sessions.Alice, {
      kind: 'chat',
      text: 'ALICE_SELF_SECRET',
      mode: 'self',
    });
    const exported = (await server.request('/api/export', { cookie: gm })).data;
    world.messages = exported.messages;
    expectedMessages.Alice.push(world.messages.at(-1).id);
    check('Alice', aliceResult);
    for (const stream of streams)
      assert.equal(await stream.next(), `data: ${aliceResult.revision}`);
    for (const [name, cookie] of Object.entries(sessions)) check(name, await server.state(cookie));
  } finally {
    for (const stream of streams) stream.close();
  }
  // Projection must never erase private information from the source or the save.
  assert.deepEqual((await server.request('/api/export', { cookie: gm })).data, world);
  assert.deepEqual(JSON.parse(await readFile(server.file, 'utf8')), world);
  console.log(
    'PASS — confidentialité : snapshots login/HTTP/action, révisions SSE, MJ/propriétaire/autre joueur, scènes inactives et source intacte.',
  );
} finally {
  await server.dispose();
}
