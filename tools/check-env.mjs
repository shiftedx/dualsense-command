import { spawnSync } from 'node:child_process';
import { mkdtempSync, existsSync, rmSync } from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import path from 'node:path';

const failures = [];
const run = (command, args, options = {}) => {
  const result = spawnSync(command, args, { encoding: 'utf8', windowsHide: true, ...options });
  if (result.error || result.status !== 0) {
    failures.push(`${command}: ${result.error?.message || result.stderr?.trim() || `exit ${result.status}`}`);
    return false;
  }
  return true;
};

if (Number(process.versions.node.split('.')[0]) < 24) failures.push('Node.js 24 or newer is required.');
if (!existsSync(new URL('../web/node_modules/svelte/package.json', import.meta.url))) {
  failures.push('Web dependencies are missing. Run npm --prefix web ci.');
} else {
  try {
    const requireWeb = createRequire(new URL('../web/package.json', import.meta.url));
    if (!existsSync(requireWeb('playwright').chromium.executablePath())) {
      failures.push('Chromium is missing. Run npm --prefix web exec playwright install chromium.');
    }
  } catch {
    failures.push('Playwright is unavailable. Run npm --prefix web ci.');
  }
}

const prefix = process.platform === 'win32' ? ['+stable-x86_64-pc-windows-gnu'] : [];
const cargoPresent = run('cargo', [...prefix, '--version']);
if (process.platform === 'win32') {
  run('gcc', ['--version']);
  run('dlltool', ['--version']);
} else {
  run('cc', ['--version']);
  if (process.platform === 'linux') run('pkg-config', ['--exists', 'libudev']);
}
if (cargoPresent) {
  const temporaryRoot = path.resolve(tmpdir());
  const directory = mkdtempSync(path.join(temporaryRoot, 'dscc-env-'));
  if (path.dirname(path.resolve(directory)) !== temporaryRoot || !path.basename(directory).startsWith('dscc-env-')) {
    throw new Error('Unexpected environment probe directory.');
  }
  try {
    run('rustc', [...prefix, '--crate-name', 'dscc_env', '-', '-o', path.join(directory, 'probe.exe')], {
      input: 'fn main() {}'
    });
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}
if (process.argv.includes('--broker')) {
  const sdks = spawnSync('dotnet', ['--list-sdks'], { encoding: 'utf8', windowsHide: true });
  if (sdks.error || sdks.status !== 0 || !/^10\./m.test(sdks.stdout ?? '')) {
    failures.push('The broker check requires the .NET 10 SDK; an installed runtime alone is insufficient.');
  }
}
if (failures.length) {
  console.error(`Environment preflight failed:\n${failures.map((failure) => `- ${failure}`).join('\n')}`);
  console.error('See docs/contributing.md for setup. No application was launched.');
  process.exitCode = 1;
} else {
  console.log(`Environment preflight passed${process.argv.includes('--broker') ? ' including .NET 10' : ''}.`);
}
