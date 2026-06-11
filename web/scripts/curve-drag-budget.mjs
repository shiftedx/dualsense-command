// web/scripts/curve-drag-budget.mjs
// Measures DOM mutation volume and frame times during a scripted curve drag
// in mock mode. Usage: node scripts/curve-drag-budget.mjs [--url http://...]
// Without --url it spawns `npm run dev:mock` on a free port and stops it after.
import { spawn, spawnSync } from 'node:child_process';
import net from 'node:net';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';

const webRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const urlArgIndex = process.argv.indexOf('--url');
const externalUrl = urlArgIndex >= 0 ? process.argv[urlArgIndex + 1] : null;
const host = '127.0.0.1';

function findOpenPort(startPort) {
  return new Promise((resolve, reject) => {
    const tryPort = (candidate) => {
      const server = net.createServer();
      server.unref();
      server.once('error', (error) => {
        if (error.code === 'EADDRINUSE' || error.code === 'EACCES') tryPort(candidate + 1);
        else reject(error);
      });
      server.listen(candidate, host, () => {
        server.close(() => resolve(candidate));
      });
    };
    tryPort(startPort);
  });
}

function startServer(port) {
  const npmCommand = process.platform === 'win32' ? 'npm.cmd' : 'npm';
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

async function waitForServer(url, output, timeoutMs = 30000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
    } catch {
      /* not up yet */
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(`dev server did not come up at ${url}\n${output()}`);
}

async function main() {
  let server = null;
  let baseUrl = externalUrl;
  if (!baseUrl) {
    const port = await findOpenPort(5180);
    baseUrl = `http://${host}:${port}`;
    server = startServer(port);
  }

  try {
    if (server) await waitForServer(baseUrl, server.output);

    const browser = await chromium.launch();
    try {
      const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
      await page.goto(baseUrl);
      await page.waitForSelector('.app-toolbar', { timeout: 15000 });
      await page.evaluate(() => {
        window.location.hash = '#/tuning';
      });
      const frame = page.locator('.dm-curve-frame').first();
      await frame.waitFor({ timeout: 15000 });

      await page.evaluate(() => {
        window.__dragMetrics = { mutations: 0, frames: [] };
        const observer = new MutationObserver((records) => {
          window.__dragMetrics.mutations += records.length;
        });
        observer.observe(document.body, { childList: true, subtree: true, attributes: true, characterData: true });
        let last = performance.now();
        const tick = (now) => {
          window.__dragMetrics.frames.push(now - last);
          last = now;
          window.__dragMetrics.raf = requestAnimationFrame(tick);
        };
        window.__dragMetrics.raf = requestAnimationFrame(tick);
      });

      const box = await frame.boundingBox();
      const startX = box.x + box.width * 0.3;
      const endX = box.x + box.width * 0.7;
      const y = box.y + box.height * 0.5;
      const MOVES = 240;
      await page.mouse.move(startX, y);
      await page.mouse.down();
      for (let i = 1; i <= MOVES; i += 1) {
        const x = startX + ((endX - startX) * i) / MOVES;
        const wobble = Math.sin(i / 8) * box.height * 0.2;
        await page.mouse.move(x, y + wobble);
        await new Promise((resolve) => setTimeout(resolve, 16));
      }
      await page.mouse.up();

      const metrics = await page.evaluate(() => {
        cancelAnimationFrame(window.__dragMetrics.raf);
        return { mutations: window.__dragMetrics.mutations, frames: window.__dragMetrics.frames };
      });
      const frames = metrics.frames.filter((ms) => ms > 0).sort((a, b) => a - b);
      const pick = (q) => frames[Math.min(frames.length - 1, Math.floor(frames.length * q))] ?? 0;
      console.log(
        JSON.stringify(
          {
            moves: MOVES,
            mutations: metrics.mutations,
            mutationsPerMove: Number((metrics.mutations / MOVES).toFixed(1)),
            frameP50Ms: Number(pick(0.5).toFixed(1)),
            frameP95Ms: Number(pick(0.95).toFixed(1)),
            frameMaxMs: Number(frames[frames.length - 1]?.toFixed(1) ?? 0)
          },
          null,
          2
        )
      );
    } finally {
      await browser.close();
    }
  } finally {
    if (server) await stopServer(server.child);
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
});
