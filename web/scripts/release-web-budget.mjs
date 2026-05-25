import { gzipSync } from 'node:zlib';
import { readFile, readdir, stat } from 'node:fs/promises';
import path from 'node:path';

const distDir = path.resolve('dist');

const budgets = {
  totalRawBytes: 2_500_000,
  totalGzipBytes: 900_000,
  javascriptGzipBytes: 500_000,
  cssGzipBytes: 150_000,
  fileCount: 150
};

async function walk(dir) {
  const entries = await readdir(dir, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...await walk(fullPath));
    } else if (entry.isFile()) {
      files.push(fullPath);
    }
  }

  return files;
}

function formatBytes(bytes) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KiB`;
  }
  return `${(bytes / 1024 / 1024).toFixed(2)} MiB`;
}

function fileKind(file) {
  if (file.endsWith('.js')) {
    return 'javascript';
  }
  if (file.endsWith('.css')) {
    return 'css';
  }
  return 'other';
}

async function main() {
  try {
    await stat(distDir);
  } catch {
    throw new Error('web/dist is missing. Run `npm run build` before `npm run test:release-size`.');
  }

  const files = await walk(distDir);
  const rows = [];

  for (const file of files) {
    const bytes = await readFile(file);
    rows.push({
      file,
      relative: path.relative(distDir, file).replaceAll(path.sep, '/'),
      kind: fileKind(file),
      rawBytes: bytes.length,
      gzipBytes: gzipSync(bytes, { level: 9 }).length
    });
  }

  const totals = rows.reduce((acc, row) => {
    acc.totalRawBytes += row.rawBytes;
    acc.totalGzipBytes += row.gzipBytes;
    if (row.kind === 'javascript') {
      acc.javascriptGzipBytes += row.gzipBytes;
    }
    if (row.kind === 'css') {
      acc.cssGzipBytes += row.gzipBytes;
    }
    return acc;
  }, {
    totalRawBytes: 0,
    totalGzipBytes: 0,
    javascriptGzipBytes: 0,
    cssGzipBytes: 0
  });

  const largest = [...rows]
    .sort((a, b) => b.gzipBytes - a.gzipBytes)
    .slice(0, 5)
    .map((row) => `${row.relative}: ${formatBytes(row.gzipBytes)} gzip`)
    .join('\n  ');

  const checks = [
    ['file count', rows.length, budgets.fileCount],
    ['total raw size', totals.totalRawBytes, budgets.totalRawBytes],
    ['total gzip size', totals.totalGzipBytes, budgets.totalGzipBytes],
    ['javascript gzip size', totals.javascriptGzipBytes, budgets.javascriptGzipBytes],
    ['css gzip size', totals.cssGzipBytes, budgets.cssGzipBytes]
  ];

  const failures = checks.filter(([, actual, budget]) => actual > budget);

  console.log('Release web budget');
  console.log(`  files: ${rows.length}/${budgets.fileCount}`);
  console.log(`  raw: ${formatBytes(totals.totalRawBytes)}/${formatBytes(budgets.totalRawBytes)}`);
  console.log(`  gzip: ${formatBytes(totals.totalGzipBytes)}/${formatBytes(budgets.totalGzipBytes)}`);
  console.log(`  js gzip: ${formatBytes(totals.javascriptGzipBytes)}/${formatBytes(budgets.javascriptGzipBytes)}`);
  console.log(`  css gzip: ${formatBytes(totals.cssGzipBytes)}/${formatBytes(budgets.cssGzipBytes)}`);
  console.log(`  largest gzip assets:\n  ${largest || 'none'}`);

  if (failures.length > 0) {
    console.error('\nRelease web budget exceeded:');
    for (const [label, actual, budget] of failures) {
      const printableActual = label === 'file count' ? actual : formatBytes(actual);
      const printableBudget = label === 'file count' ? budget : formatBytes(budget);
      console.error(`  ${label}: ${printableActual} > ${printableBudget}`);
    }
    process.exitCode = 1;
  }
}

main().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
