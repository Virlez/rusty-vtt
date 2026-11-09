import { mkdir } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';

await mkdir('coverage', { recursive: true });
const result = spawnSync(
  'cargo',
  ['llvm-cov', '--locked', '--lcov', '--output-path', 'coverage/lcov.info'],
  { stdio: 'inherit' },
);
if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status ?? 1);
await import('./coverage-summary.mjs');
