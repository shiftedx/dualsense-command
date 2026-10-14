import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const result = spawnSync(process.env.CARGO ?? 'cargo', [
  ...(process.platform === 'win32' ? ['+stable-x86_64-pc-windows-gnu'] : []),
  'metadata', '--offline', '--locked', '--all-features', '--format-version', '1'
], { cwd: root, encoding: 'utf8', maxBuffer: 20 * 1024 * 1024 });
if (result.error || result.status !== 0) throw result.error ?? new Error(result.stderr);
const packages = JSON.parse(result.stdout).packages.filter((entry) => entry.source).map((entry) => ({
  name: entry.name, version: entry.version, license: entry.license, directory: path.dirname(entry.manifest_path)
}));
// Browser runtime and font assets. Build-only npm tools are not shipped; Rust
// inventory conservatively includes all targets and build dependencies.
for (const name of ['svelte', '@lucide/svelte', '@fontsource-variable/inter', '@fontsource-variable/jetbrains-mono']) {
  const directory = path.join(root, 'web/node_modules', name);
  const entry = JSON.parse(fs.readFileSync(path.join(directory, 'package.json'), 'utf8'));
  packages.push({ name, version: entry.version, license: entry.license, directory });
}
let output = '# Dependency license texts\n\nGenerated from locked installed package source archives by tools/generate-dependency-notices.mjs. Includes all Rust targets and build dependencies; some are not shipped. Exact package license/notice files are preserved below. No package source has been modified. See THIRD_PARTY_NOTICES.md for fonts, icons and optional broker notices.\n';
for (const name of ['Cargo.lock', 'web/package-lock.json']) output += `\n${name} SHA256: ${createHash('sha256').update(fs.readFileSync(path.join(root, name))).digest('hex')}\n`;
const missing = [];
for (const entry of packages.sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version))) {
  const files = [];
  const collect = (directory) => {
    for (const item of fs.readdirSync(directory, { withFileTypes: true })) {
      const full = path.join(directory, item.name);
      if (item.isDirectory() && !['node_modules', '.git', 'target'].includes(item.name)) collect(full);
      else if (item.isFile() && (/^(?:licen[cs]e|copying|notice)(?:[._-].*)?$/i.test(item.name) || (entry.name === 'r-efi' && item.name === 'AUTHORS'))) files.push(full);
    }
  };
  collect(entry.directory);
  // valuable 0.1.1's crate omits its license file. Preserve the upstream text
  // from the exact .cargo_vcs_info commit, not a moving branch.
  if (entry.name === 'valuable' && entry.version === '0.1.1') files.push(path.join(root, 'tools/licenses/valuable-0.1.1-LICENSE'));
  if (!files.length) { missing.push(`${entry.name}@${entry.version} (${entry.license})`); continue; }
  const source = entry.name.startsWith('@') || ['svelte'].includes(entry.name)
    ? `https://www.npmjs.com/package/${entry.name}/v/${entry.version}`
    : `https://crates.io/api/v1/crates/${entry.name}/${entry.version}/download`;
  output += `\n## ${entry.name} ${entry.version}\n\nDeclared license: ${entry.license}\nCorresponding unmodified source archive: ${source}\n`;
  for (const file of files.sort()) {
    const text = fs.readFileSync(file, 'utf8');
    if (text.includes('\0')) throw new Error(`Binary license file: ${entry.name}`);
    const label = file.startsWith(entry.directory + path.sep) ? path.relative(entry.directory, file).replaceAll('\\', '/') : 'upstream LICENSE (9efc29b6e58cef28f6566a47aa7e142a55fead77)';
    output += `\n### ${label}\n\n${text.trim()}\n`;
  }
}
if (missing.length) throw new Error(`Packages without license text: ${missing.join(', ')}`);
const destination = path.join(root, 'DEPENDENCY_LICENSES.txt');
if (process.argv.includes('--check')) {
  if (!fs.existsSync(destination) || fs.readFileSync(destination, 'utf8') !== output) throw new Error('Dependency notices are stale. Run node tools/generate-dependency-notices.mjs.');
} else fs.writeFileSync(destination, output);
console.log(`Dependency notices: ${packages.length} packages, ${Buffer.byteLength(output)} bytes.`);
