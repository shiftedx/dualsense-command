import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (name) => fs.readFileSync(path.join(root, name), 'utf8');

export function parseTag(tag) {
  const match = /^v?(\d+\.\d+\.\d+)(-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.exec(tag);
  if (!match) throw new Error(`Invalid release tag: ${tag}; use vMAJOR.MINOR.PATCH[-prerelease].`);
  return { version: match[1], prerelease: Boolean(match[2]), release: tag.replace(/^v/, '') };
}

export function releaseNotes(changelog, version) {
  const blocks = changelog.split(/(?=^# DualSense Command Center )/m);
  const block = blocks.find((entry) => entry.split(/\r?\n/, 1)[0] === `# DualSense Command Center ${version}`);
  if (!block) throw new Error(`Missing CHANGELOG.md section for ${version}.`);
  return block.trim() + '\n';
}

export function checkDistribution(ledger, shippedPaths) {
  if (!Array.isArray(ledger) || ledger.length === 0) throw new Error('Redistribution ledger must be a nonempty array.');
  for (const name of shippedPaths) {
    if (!ledger.some((entry) => entry?.path === name)) throw new Error(`Redistribution ledger missing shipped asset: ${name}.`);
  }
  const blocked = ledger.filter((entry) => !entry?.path || entry.status !== 'verified' || !entry.source || !entry.license);
  if (blocked.length) throw new Error(`Redistribution unresolved: ${blocked.map((entry) => entry?.path ?? 'invalid entry').join(', ')}. Record permission or remove the payload before release.`);
}

export function checkRelease(args = process.argv.slice(2)) {
  const value = (flag) => args.includes(flag) ? args[args.indexOf(flag) + 1] : undefined;
  const version = JSON.parse(read('web/package.json')).version;
  const metadata = [['root package', JSON.parse(read('package.json')).version]];
  const lock = JSON.parse(read('web/package-lock.json'));
  metadata.push(['web lock', lock.version], ['web lock root', lock.packages[''].version]);
  for (const crate of fs.readdirSync(path.join(root, 'crates'))) {
    const manifest = `crates/${crate}/Cargo.toml`;
    metadata.push([manifest, /^version\s*=\s*"([^"]+)"/m.exec(read(manifest))?.[1]]);
  }
  for (const match of read('Cargo.lock').matchAll(/^name = "(dscc-[^"]+)"\r?\nversion = "([^"]+)"/gm)) {
    metadata.push([`Cargo.lock ${match[1]}`, match[2]]);
  }
  metadata.push(['MSI default', /\[string\]\$Version\s*=\s*"([^"]+)"/.exec(read('packaging/package-msi.ps1'))?.[1]]);
  for (const [name, actual] of metadata) {
    if (actual !== version) throw new Error(`${name}: ${actual} does not match ${version}.`);
  }
  const tag = value('--tag');
  const parsed = tag ? parseTag(tag) : { version, release: version, prerelease: false };
  if (parsed.version !== version) throw new Error(`Tag ${tag} does not match repo version ${version}.`);
  const notes = releaseNotes(read('CHANGELOG.md'), parsed.release);
  if (value('--notes')) fs.writeFileSync(path.resolve(value('--notes')), notes);
  if (process.env.GITHUB_OUTPUT && tag) {
    fs.appendFileSync(process.env.GITHUB_OUTPUT, `msi_version=${version}\nprerelease=${parsed.prerelease}\nartifact_suffix=${tag}\n`);
  }
  if (args.includes('--distribution')) {
    const ledger = JSON.parse(read('docs/redistribution.json'));
    const shippedPaths = ['crates/dscc-agent/assets/forza/ControllerIcons.zip', 'web/public/dualsense', 'web/public/controller-diagram', 'crates/dscc-tray/assets/dscc-tray.ico', 'web/public/dualsense-controller.svg', 'web/public/favicon.svg', 'crates/dscc-tray/assets/dualsense-svgrepo-com.svg']
      .filter((name) => fs.existsSync(path.join(root, name)));
    checkDistribution(ledger, shippedPaths);
    const publicFiles = fs.readdirSync(path.join(root, 'web/public'), { withFileTypes: true });
    for (const item of publicFiles) {
      if (!ledger.some((entry) => entry.path === `web/public/${item.name}`)) throw new Error(`Unreviewed public asset: ${item.name}.`);
    }
    for (const entry of ledger) {
      if (!entry.hashes || !Object.keys(entry.hashes).length) throw new Error(`Missing exact asset hashes: ${entry.path}.`);
      const full = path.join(root, entry.path);
      const files = fs.statSync(full).isDirectory() ? fs.readdirSync(full).sort() : [''];
      if (JSON.stringify(files) !== JSON.stringify(Object.keys(entry.hashes).sort())) throw new Error(`Asset file inventory changed: ${entry.path}.`);
      for (const name of files) {
        const hash = createHash('sha256').update(fs.readFileSync(name ? path.join(full, name) : full)).digest('hex');
        if (hash !== entry.hashes[name]) throw new Error(`Asset content changed; review provenance: ${entry.path}/${name}.`);
      }
    }
    const notices = read('DEPENDENCY_LICENSES.txt');
    for (const name of ['Cargo.lock', 'web/package-lock.json']) {
      const hash = createHash('sha256').update(fs.readFileSync(path.join(root, name))).digest('hex');
      if (!notices.includes(`${name} SHA256: ${hash}`)) throw new Error('Dependency notices are stale; regenerate before release.');
    }
  }
  console.log(`Release metadata aligned: ${version}${tag ? ` (${tag})` : ''}.`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { checkRelease(); } catch (error) { console.error(error.message); process.exitCode = 1; }
}
