import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';

const binary = resolve(process.env.RUSTY_TEST_BINARY || `target/debug/rusty-vtt${process.platform === 'win32' ? '.exe' : ''}`);
for (const suite of ['smoke', 'builder-smoke', 'advanced', 'qa-audit', 'security-audit']) {
  const result = spawnSync(process.execPath, [`tests/${suite}.mjs`, binary], {
    stdio: 'inherit', windowsHide: true, timeout: 120000,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status || 1);
}
