import { spawnSync } from 'node:child_process';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

// Run every step from the repo root so `npm run check:perf` works from the
// root and from web/ alike (`npm --prefix web run test:button-map` and the
// cargo workspace both resolve relative to it).
const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

const cargoPrefix = process.platform === 'win32' ? ['+stable-x86_64-pc-windows-gnu'] : [];
const npmCommand = process.platform === 'win32' ? 'npm.cmd' : 'npm';

run('cargo', [
  ...cargoPrefix,
  'test',
  '--workspace',
  '--all-features',
  'perf_guard'
]);
run(npmCommand, ['--prefix', 'web', 'run', 'test:button-map']);

function run(command, args) {
  const invocation =
    process.platform === 'win32' && command.endsWith('.cmd')
      ? ['cmd.exe', ['/d', '/s', '/c', command, ...args]]
      : [command, args];
  const result = spawnSync(invocation[0], invocation[1], { stdio: 'inherit', cwd: repoRoot });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
