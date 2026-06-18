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

const publicRootFiles = new Set([
  'CHANGELOG.md',
  'CONTEXT.md',
  'PRODUCT.md',
  'README.md',
  'SECURITY.md',
  'SUPPORT.md',
  'THIRD_PARTY_NOTICES.md',
  'package.json',
  'Cargo.toml'
]);

const files = Array.from(new Set(execFileSync(git, ['ls-files', '--cached', '--modified', '--others', '--exclude-standard'], {
  cwd: repoRoot,
  encoding: 'utf8'
})
  .split(/\r?\n/)
  .filter(Boolean)
  .filter((file) =>
    file.startsWith('crates/') ||
    file.startsWith('docs/') ||
    file.startsWith('web/src/') ||
    file.startsWith('web/scripts/') ||
    file.startsWith('.github/workflows/') ||
    file.startsWith('packaging/') ||
    file.startsWith('tools/dscc-hidmaestro-broker/') ||
    publicRootFiles.has(file)
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
    codeOnly: true,
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
    codeOnly: true,
    allow: [selfAuditScript]
  },
  {
    name: 'game injection language',
    pattern: /\b(game injection|memory scanning|memory scan|anti-cheat bypass)\b/i,
    codeOnly: true,
    allow: [selfAuditScript]
  },
  {
    name: 'private path disclosure',
    pattern: /\b(full executable path|provider private path|private broker path)\b/i,
    codeOnly: true,
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
    codeOnly: true,
    allow: [selfAuditScript]
  },
  {
    name: 'legacy production surface',
    pattern: /legacy/i,
    codeOnly: true,
    allow: [selfAuditScript]
  },
  {
    name: 'local agent tooling surface',
    pattern: /\b(mattpocock|setup-matt-pocock-skills|superpowers:|\.superpowers\/|Generated with \[Claude Code\]|docs\/agents\/|skills-lock\.json|AGENTS\.md|PROVENANCE\.md|WINDOWS_HANDOFF_PROMPT|AFK agent|impeccable skill)\b/i,
    allow: [selfAuditScript]
  },
  {
    name: 'external user-attachment asset',
    pattern: /github\.com\/user-attachments\/assets/i,
    allow: [selfAuditScript]
  }
];

const failures = [];
const releaseWorkflowPath = path.join(repoRoot, '.github/workflows/release.yml');
const releaseWorkflowText = readFileSync(releaseWorkflowPath, 'utf8');
const pinnedHidMaestroReleaseUrl = releaseWorkflowText.match(/HIDMAESTRO_RELEASE_URL:\s*(\S+)/)?.[1] ?? '';
const pinnedHidMaestroReleaseSha256 = releaseWorkflowText.match(/HIDMAESTRO_RELEASE_SHA256:\s*(\S+)/)?.[1] ?? '';
if (!pinnedHidMaestroReleaseUrl) {
  failures.push('.github/workflows/release.yml: missing HIDMAESTRO_RELEASE_URL');
}
if (!pinnedHidMaestroReleaseSha256) {
  failures.push('.github/workflows/release.yml: missing HIDMAESTRO_RELEASE_SHA256');
}
const thirdPartyNoticesPath = path.join(repoRoot, 'THIRD_PARTY_NOTICES.md');
if (!existsSync(thirdPartyNoticesPath)) {
  failures.push('THIRD_PARTY_NOTICES.md: missing third-party notices file');
} else {
  const thirdPartyNotices = readFileSync(thirdPartyNoticesPath, 'utf8');
  for (const required of [
    'HIDMaestro',
    'MIT License',
    'Copyright (c) 2026 HIDMaestro Contributors',
    pinnedHidMaestroReleaseUrl,
    pinnedHidMaestroReleaseSha256
  ]) {
    if (!thirdPartyNotices.includes(required)) {
      failures.push(`THIRD_PARTY_NOTICES.md: missing required HIDMaestro notice text: ${required}`);
    }
  }
}

const packageMsiPath = path.join(repoRoot, 'packaging/package-msi.ps1');
const packageMsiText = readFileSync(packageMsiPath, 'utf8');
if (!packageMsiText.includes('THIRD_PARTY_NOTICES.txt')) {
  failures.push('packaging/package-msi.ps1: Bridge staging must install THIRD_PARTY_NOTICES.txt beside the broker');
}

for (const file of trackedFiles) {
  const normalized = file.replaceAll('\\', '/');
  if (!existsSync(path.join(repoRoot, file))) continue;
  if (forbiddenTrackedArtifactPattern.test(normalized) && !allowedTrackedArtifacts.has(normalized)) {
    failures.push(`${file}: tracked release, signing, or binary artifact`);
  }
}

for (const file of files) {
  const normalized = file.replaceAll('\\', '/');
  const isPublicText = normalized.startsWith('docs/') || publicRootFiles.has(normalized);
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
      if (rule.codeOnly && isPublicText) continue;
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
