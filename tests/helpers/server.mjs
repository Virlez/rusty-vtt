// Real server, isolated world, OS-assigned port. Never opens campaign data.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

export async function isolatedServer() {
  const dir = await mkdtemp(join(tmpdir(), 'rusty-contract-'));
  const file = join(dir, 'world.json');
  const binary = resolve(process.argv[2] || `target/debug/rusty-vtt${process.platform === 'win32' ? '.exe' : ''}`);
  const passwords = { MJ: 'contract-gm-secret-123456', Alice: 'contract-alice-secret-123456', Bob: 'contract-bob-secret-123456', Eve: 'contract-eve-secret-123456' };
  let child, url, output;
  const streams = new Set();
  async function stop() {
    for (const stream of streams) stream.close();
    streams.clear();
    if (child && child.exitCode === null && child.signalCode === null) {
      await new Promise(resolveExit => { child.once('exit', resolveExit); child.kill(); });
    }
  }
  async function boot({ failure } = {}) {
    output = '';
    child = spawn(binary, [], {
      windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'],
      env: { ...process.env, RUSTY_BIND: '127.0.0.1:0', RUSTY_DATA: file,
        RUSTY_GM_PASSWORD: passwords.MJ, RUSTY_HTTPS: '0',
        RUSTY_PLAYERS_JSON: JSON.stringify(Object.fromEntries(Object.entries(passwords).filter(([name]) => name !== 'MJ'))) },
    });
    return new Promise((resolveBoot, reject) => {
      const timer = setTimeout(() => { child.kill(); reject(Error(`Server startup timeout: ${output}`)); }, 15000);
      const finish = fn => { clearTimeout(timer); fn(); };
      child.once('error', e => finish(() => reject(e)));
      child.once('exit', code => finish(() => {
        if (failure) {
          try { assert.notEqual(code, 0); assert.match(output, failure); resolveBoot(); }
          catch (e) { reject(e); }
        } else reject(Error(`Server exited ${code}: ${output}`));
      }));
      const read = chunk => {
        output += chunk;
        const match = output.match(/http:\/\/(127\.0\.0\.1:\d+)/);
        if (match) finish(() => {
          url = `http://${match[1]}`;
          if (failure) reject(Error('Corrupt world was accepted'));
          else resolveBoot();
        });
      };
      child.stdout.on('data', read);
      child.stderr.on('data', read);
    });
  }
  async function request(path, { body, cookie, status = 200 } = {}) {
    const response = await fetch(url + path, {
      method: body === undefined ? 'GET' : 'POST',
      headers: { 'Content-Type': 'application/json', ...(cookie ? { Cookie: cookie } : {}) },
      body: body === undefined ? undefined : JSON.stringify(body), signal: AbortSignal.timeout(5000),
    });
    const raw = await response.text();
    assert.equal(response.status, status, `${path}: ${raw}`);
    return { data: status === 200 ? JSON.parse(raw) : raw, cookie: response.headers.get('set-cookie')?.split(';')[0] };
  }
  async function events(cookie) {
    const controller = new AbortController();
    const openingTimer = setTimeout(() => controller.abort(Error('SSE connection timeout')), 5000);
    let response;
    try { response = await fetch(url + '/api/events', { headers: { Cookie: cookie }, signal: controller.signal }); }
    finally { clearTimeout(openingTimer); }
    assert.equal(response.status, 200);
    assert.match(response.headers.get('content-type'), /text\/event-stream/);
    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let buffered = '';
    const stream = {
      close() { controller.abort(); streams.delete(stream); },
      async next() {
        const timer = setTimeout(() => controller.abort(Error('SSE event timeout')), 5000);
        try {
          for (;;) {
            const end = buffered.indexOf('\n\n');
            if (end >= 0) {
              const frame = buffered.slice(0, end);
              buffered = buffered.slice(end + 2);
              if (frame.split('\n').some(line => line.startsWith('data:'))) return frame;
              continue; // Ignore keep-alive comments.
            }
            const { value, done } = await reader.read();
            assert.equal(done, false, 'SSE ended before its event');
            buffered += decoder.decode(value, { stream: true }).replaceAll('\r\n', '\n');
          }
        } finally { clearTimeout(timer); }
      },
    };
    streams.add(stream);
    return stream;
  }
  return {
    dir, file, boot, stop, request, events,
    login: async name => request('/api/login', { body: { name, password: passwords[name] } }),
    state: async cookie => (await request('/api/state', { cookie })).data,
    action: async (cookie, body, status = 200) => (await request('/api/action', { cookie, body, status })).data,
    async dispose() { await stop(); await rm(dir, { recursive: true, force: true }); },
  };
}
