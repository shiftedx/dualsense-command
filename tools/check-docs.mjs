import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const files = [...new Set(execFileSync('git', ['ls-files', '-z', '--cached', '--others', '--exclude-standard'], {
  cwd: root, encoding: 'utf8'
}).split('\0').filter(Boolean))];
const known = new Set(files);
const problems = [];
let count = 0;
for (const file of files.filter((name) => name.endsWith('.md') && existsSync(path.join(root, name)))) {
  count++;
  const text = readFileSync(path.join(root, file), 'utf8').replace(/```[\s\S]*?```/g, '');
  const destinations = [...text.matchAll(/!?\[[^\]]*\]\((<[^>]+>|[^\s)]+)(?:\s+"[^"]*")?\)/g)]
    .map((match) => match[1]);
  for (const destination of destinations) {
    const url = destination.replace(/^<|>$/g, '');
    if (/^(?:[a-z][a-z\d+.-]*:|#|\/\/)/i.test(url)) continue;
    const target = url.split(/[?#]/, 1)[0];
    if (!target) continue;
    const resolved = path.resolve(path.dirname(path.join(root, file)), decodeURIComponent(target));
    const relative = path.relative(root, resolved).split(path.sep).join('/').replace(/\/$/, '');
    const available = known.has(relative) || files.some((name) => name.startsWith(`${relative}/`));
    if (!existsSync(resolved) || !available) problems.push(`${file}: ${url} (missing, ignored, or wrong case)`);
  }
}
if (problems.length) {
  console.error(problems.join('\n'));
  process.exitCode = 1;
} else {
  console.log(`Documentation links pass across ${count} repository Markdown files.`);
}
