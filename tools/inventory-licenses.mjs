import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';

// Declared metadata inventory, not a legal clearance or distribution notice generator.
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const prefix = process.platform === 'win32' ? ['+stable-x86_64-pc-windows-gnu'] : [];
const result = spawnSync(process.env.CARGO ?? 'cargo', [
  ...prefix, 'metadata', '--offline', '--locked', '--format-version', '1'
], { cwd: root, encoding: 'utf8', maxBuffer: 20 * 1024 * 1024 });
if (result.error) throw result.error;
if (result.status !== 0) {
  console.error('Offline cargo metadata failed. Fetch/build locked dependencies first.');
  process.exit(result.status ?? 1);
}
const cargo = JSON.parse(result.stdout).packages.filter((entry) => entry.source).map((entry) => ({
  kind: 'cargo', name: entry.name, version: entry.version, license: entry.license ?? null
}));
const lock = JSON.parse(fs.readFileSync(path.join(root, 'web/package-lock.json'), 'utf8'));
const npm = Object.entries(lock.packages).filter(([key]) => key).map(([key, entry]) => ({
  kind: 'npm', name: key.split('node_modules/').pop(), version: entry.version, license: entry.license ?? null,
  development: Boolean(entry.dev), optional: Boolean(entry.optional)
}));
const inventory = {
  schema: 'dscc-declared-licenses-v1',
  scope: 'All locked package versions, including development and other-platform dependencies; declared licenses only. npm development does not imply absent from the browser bundle.',
  lockfiles: Object.fromEntries(['Cargo.lock', 'web/package-lock.json'].map((name) => [
    name, createHash('sha256').update(fs.readFileSync(path.join(root, name))).digest('hex')
  ])),
  packages: []
};
for (const [ecosystem, entries] of [['cargo', cargo], ['npm', npm]]) {
  entries.sort((left, right) => left.name.localeCompare(right.name) || left.version.localeCompare(right.version));
  const missing = entries.filter((entry) => !entry.license);
  console.log(`${ecosystem}: ${entries.length} package versions; ${missing.length} missing license declarations.`);
  for (const entry of missing) console.log(`  ${entry.name}@${entry.version}`);
  if (missing.length) process.exitCode = 1;
  inventory.packages.push(...entries);
}
const outputIndex = process.argv.indexOf('--output');
const output = outputIndex === -1 ? path.join(root, 'docs/dependency-licenses.json') : process.argv[outputIndex + 1];
if (!output) throw new Error('--output requires a destination path.');
fs.writeFileSync(path.resolve(output), JSON.stringify(inventory, null, 2) + '\n');
