import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const git = process.platform === 'win32' ? 'git.exe' : 'git';
const files = Array.from(new Set(execFileSync(git, ['ls-files', '--cached', '--modified', '--others', '--exclude-standard'], {
  cwd: repoRoot,
  encoding: 'utf8'
})
  .split(/\r?\n/)
  .filter(Boolean)
  .filter((file) =>
    file.startsWith('crates/') ||
    file.startsWith('web/src/') ||
    file.startsWith('web/scripts/') ||
    file.startsWith('.github/workflows/') ||
    file.startsWith('packaging/') ||
    file.startsWith('tools/dscc-hidmaestro-broker/')
  )))
  .filter((file) => existsSync(path.join(repoRoot, file)));

const selfAuditScript = /web\/scripts\/source-audit\.mjs$/;

const rules = [
  {
    name: 'raw HID route surface',
    pattern: /\b(raw_hid|raw-hid|raw hid-byte|raw hid byte)\b/i,
    allow: [
      /web\/src\/app\/supportBundle\.ts$/,
      /web\/src\/App\.svelte$/,
      /crates\/dscc-agent\/src\/support_bundle\.rs$/,
      selfAuditScript
    ]
  },
  {
    name: 'driver payload route surface',
    pattern: /\bdriver payload\b/i,
    allow: [selfAuditScript]
  },
  {
    name: 'game injection language',
    pattern: /\b(game injection|memory scanning|memory scan|anti-cheat bypass)\b/i,
    allow: [selfAuditScript]
  },
  {
    name: 'private path disclosure',
    pattern: /\b(full executable path|provider private path|private broker path)\b/i,
    allow: [
      /web\/src\/app\/supportBundle\.ts$/,
      /web\/src\/App\.svelte$/,
      /crates\/dscc-agent\/src\/support_bundle\.rs$/,
      selfAuditScript
    ]
  },
  {
    name: 'stub production surface',
    pattern: /\b(TODO: ship|STUB|stubbed|placeholder implementation)\b/i,
    allow: [selfAuditScript]
  },
  {
    name: 'legacy production surface',
    pattern: /\blegacy\b/i,
    allow: [selfAuditScript]
  }
];

const failures = [];
for (const file of files) {
  const normalized = file.replaceAll('\\', '/');
  const text = readFileSync(path.join(repoRoot, file), 'utf8');
  const lines = text.split(/\r?\n/);
  for (const [index, line] of lines.entries()) {
    for (const rule of rules) {
      if (!rule.pattern.test(line)) continue;
      if (rule.allow.some((allowed) => allowed.test(normalized))) continue;
      failures.push(`${file}:${index + 1}: ${rule.name}: ${line.trim()}`);
    }
  }
}

if (failures.length) {
  console.error(`source audit failed with ${failures.length} finding(s):`);
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(`source audit passed across ${files.length} tracked/local files`);
