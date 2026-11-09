import { readFile, writeFile, mkdir } from 'node:fs/promises';

const records = (await readFile('coverage/lcov.info', 'utf8')).split('end_of_record');
const rows = [];
for (const record of records) {
  const filename = record.match(/^SF:(.+)$/m)?.[1];
  if (!filename || /(?:property_tests|builder_tests)\.rs$/.test(filename)) continue;
  const source = await readFile(filename, 'utf8');
  const testStart = source.search(/#\[cfg\(test\)\]\s*mod tests/);
  const cutoff = testStart < 0 ? Infinity : source.slice(0, testStart).split('\n').length;
  const lines = [...record.matchAll(/^DA:(\d+),(\d+)/gm)].filter((match) => +match[1] < cutoff);
  const covered = lines.filter((match) => +match[2] > 0).length;
  rows.push({ file: filename.split('/').at(-1), covered, total: lines.length });
}
const covered = rows.reduce((sum, row) => sum + row.covered, 0);
const total = rows.reduce((sum, row) => sum + row.total, 0);
const table = [
  '| Fichier | Lignes couvertes | Couverture |',
  '| --- | ---: | ---: |',
  ...rows.map(
    (row) =>
      `| ${row.file} | ${row.covered}/${row.total} | ${((100 * row.covered) / row.total).toFixed(1)} % |`,
  ),
  `| **Total** | **${covered}/${total}** | **${((100 * covered) / total).toFixed(1)} %** |`,
].join('\n');
const report =
  '# Couverture Rust unitaire\n\n' +
  table +
  '\n\nLignes applicatives instrumentées hors modules de test. Cette mesure exclut les suites HTTP/E2E et ne mesure pas les branches ni le JavaScript.\n';
await mkdir('coverage', { recursive: true });
await writeFile('coverage/summary.md', report);
console.log(report);
if (process.env.GITHUB_STEP_SUMMARY) {
  const { appendFile } = await import('node:fs/promises');
  await appendFile(process.env.GITHUB_STEP_SUMMARY, report);
}
