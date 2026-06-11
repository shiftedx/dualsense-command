import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const git = process.platform === 'win32' ? 'git.exe' : 'git';
const trackedFiles = execFileSync(git, ['ls-files'], {
  cwd: repoRoot,
  encoding: 'utf8'
})
  .split(/\r?\n/)
  .filter(Boolean);

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
const allowedTrackedArtifacts = new Set([
  'crates/dscc-agent/assets/forza/ControllerIcons.zip'
]);
const forbiddenTrackedArtifactPattern = /\.(msi|exe|dll|pdb|cab|wixobj|wixpdb|pfx|p12|pem|key|zip|7z|rar)$/i;

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
    pattern: /legacy/i,
    allow: [selfAuditScript]
  }
];

const failures = [];
for (const file of trackedFiles) {
  const normalized = file.replaceAll('\\', '/');
  if (forbiddenTrackedArtifactPattern.test(normalized) && !allowedTrackedArtifacts.has(normalized)) {
    failures.push(`${file}: tracked release, signing, or binary artifact`);
  }
}

for (const file of files) {
  const normalized = file.replaceAll('\\', '/');
  const text = readFileSync(path.join(repoRoot, file), 'utf8');
  if (/-----BEGIN (RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----/.test(text)) {
    failures.push(`${file}: private key material must not be committed`);
  }
  if (/AKIA[0-9A-Z]{16}/.test(text)) {
    failures.push(`${file}: AWS-style access key must not be committed`);
  }
  if (/ghp_[A-Za-z0-9_]{30,}/.test(text)) {
    failures.push(`${file}: GitHub personal access token must not be committed`);
  }
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
