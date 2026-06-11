import { spawn, spawnSync } from 'node:child_process';
import net from 'node:net';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';

const webRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const host = '127.0.0.1';
const requestedPort = Number(process.env.DSCC_VISUAL_SMOKE_PORT ?? 0);
const port = requestedPort > 0 ? requestedPort : await findOpenPort(5174);
const baseUrl = `http://${host}:${port}`;
const npmCommand = process.platform === 'win32' ? 'npm.cmd' : 'npm';
const routeChecks = [
  { hash: '#/status', pattern: /Status|Everything|controller/i },
  { hash: '#/tuning', pattern: /Tuning|Profile/i },
  { hash: '#/advanced/controller', pattern: /Controller details|Live input/i },
  // Button mapping needs a game scope first; main() selects one via the
  // toolbar before this route runs.
  { hash: '#/advanced/button-mapping', pattern: /Button Mapping|Default mirror/i }
];
// Old routes keep working forever; each lands on the new home for its content.
const legacyRedirectChecks = [{ from: '#/games', to: '#/tuning' }];
const viewports = [
  { width: 1366, height: 768 },
  { width: 1440, height: 900 },
  { width: 390, height: 844 }
];

function findOpenPort(startPort) {
  return new Promise((resolve, reject) => {
    const tryPort = (candidate) => {
      const server = net.createServer();
      server.unref();
      server.once('error', (error) => {
        if (error.code === 'EADDRINUSE' || error.code === 'EACCES') {
          tryPort(candidate + 1);
          return;
        }
        reject(error);
      });
      server.listen(candidate, host, () => {
        server.close(() => resolve(candidate));
      });
    };
    tryPort(startPort);
  });
}

function startServer() {
  const command = process.platform === 'win32' ? 'cmd.exe' : npmCommand;
  const args = process.platform === 'win32'
    ? ['/d', '/s', '/c', `${npmCommand} run dev:mock -- --port ${port} --strictPort`]
    : ['run', 'dev:mock', '--', '--port', String(port), '--strictPort'];
  const child = spawn(command, args, {
    cwd: webRoot,
    stdio: ['ignore', 'pipe', 'pipe'],
    detached: process.platform !== 'win32',
    env: { ...process.env, BROWSER: 'none' }
  });
  let output = '';
  child.stdout.on('data', (chunk) => {
    output += chunk.toString();
  });
  child.stderr.on('data', (chunk) => {
    output += chunk.toString();
  });
  return { child, output: () => output };
}

async function waitForServer(output) {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(baseUrl);
      if (response.ok) return;
    } catch {
      // Keep polling until Vite is ready.
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(`Timed out waiting for ${baseUrl}\n${output()}`);
}

async function routeSnapshot(page) {
  return page.evaluate(() => {
    const doc = document.documentElement;
    const before = window.scrollY;
    window.scrollTo(0, doc.scrollHeight);
    const after = window.scrollY;
    window.scrollTo(0, before);
    return {
      hash: location.hash,
      text: document.body.innerText,
      scrollHeight: doc.scrollHeight,
      clientHeight: doc.clientHeight,
      scrollWidth: doc.scrollWidth,
      clientWidth: doc.clientWidth,
      canReachBottom: doc.scrollHeight <= window.innerHeight + 2 || after > before
    };
  });
}

async function main() {
  const server = startServer();
  try {
    await waitForServer(server.output);
    const browser = await chromium.launch({ headless: true });
    const failures = [];
    for (const viewport of viewports) {
      const page = await browser.newPage({ viewport });
      const consoleErrors = [];
      page.on('console', (message) => {
        if (message.type() === 'error') consoleErrors.push(message.text());
      });
      page.on('pageerror', (error) => {
        consoleErrors.push(error.message);
      });

      for (const check of routeChecks) {
        if (check.hash === '#/advanced/button-mapping') {
          // Button mapping is guarded behind a game scope; pick the mock
          // running game from the Tuning header's game dropdown so the route
          // does not bounce.
          await page.goto(`${baseUrl}/#/tuning`, { waitUntil: 'domcontentloaded' });
          await page.waitForTimeout(300);
          await page.click('.tuning-header-game');
          await page.click('.tuning-menu .tuning-menu-item:has(.tuning-running-dot)');
          await page.waitForTimeout(500);
        }
        await page.goto(`${baseUrl}/${check.hash}`, { waitUntil: 'domcontentloaded' });
        await page.waitForTimeout(300);
        const snapshot = await routeSnapshot(page);
        const label = `${viewport.width}x${viewport.height} ${check.hash}`;
        if (snapshot.hash !== check.hash) failures.push(`${label}: landed on ${snapshot.hash}`);
        if (!check.pattern.test(snapshot.text)) failures.push(`${label}: expected route text was missing`);
        if (!snapshot.canReachBottom) failures.push(`${label}: page content could not scroll to the bottom`);
        if (snapshot.scrollWidth > snapshot.clientWidth + 2) failures.push(`${label}: horizontal overflow ${snapshot.scrollWidth - snapshot.clientWidth}px`);
        if (check.hash === '#/advanced/button-mapping' && !/Default mirror only|No writable|read-only|inputs mapped/i.test(snapshot.text)) {
          failures.push(`${label}: mapping session copy (read-only or live layout) was missing`);
        }
      }

      for (const redirect of legacyRedirectChecks) {
        await page.goto(`${baseUrl}/${redirect.from}`, { waitUntil: 'domcontentloaded' });
        await page.waitForTimeout(300);
        const finalHash = await page.evaluate(() => location.hash);
        if (finalHash !== redirect.to) {
          failures.push(`${viewport.width}x${viewport.height} ${redirect.from}: redirected to ${finalHash}, expected ${redirect.to}`);
        }
      }
      if (consoleErrors.length) {
        failures.push(`${viewport.width}x${viewport.height}: console errors: ${consoleErrors.slice(0, 5).join(' | ')}`);
      }
      await page.close();
    }
    await browser.close();
    if (failures.length) throw new Error(`Visual smoke failed:\n- ${failures.join('\n- ')}`);
    console.log(`visual smoke passed for ${routeChecks.length} routes across ${viewports.length} viewports`);
  } finally {
    await stopServer(server.child);
  }
}

async function stopServer(child) {
  if (!child.pid || child.killed) return;
  if (process.platform === 'win32') {
    spawnSync('taskkill.exe', ['/pid', String(child.pid), '/t', '/f'], { stdio: 'ignore' });
    return;
  }
  const exited = new Promise((resolve) => {
    child.once('exit', resolve);
  });
  try {
    process.kill(-child.pid, 'SIGTERM');
  } catch {
    child.kill('SIGTERM');
  }
  const timeout = await Promise.race([
    exited.then(() => 'exited'),
    new Promise((resolve) => setTimeout(() => resolve('timeout'), 2_000))
  ]);
  if (timeout === 'timeout') {
    try {
      process.kill(-child.pid, 'SIGKILL');
    } catch {
      child.kill('SIGKILL');
    }
    await Promise.race([
      exited,
      new Promise((resolve) => setTimeout(resolve, 1_000))
    ]);
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
});
