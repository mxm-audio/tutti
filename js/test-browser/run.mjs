// The package's browser test: serves the repository, opens `page.html` in each browser named
// (desktop WebKit and Chromium by default; one alone as `npm run test:browser -- webkit`), and
// reports what `page.mjs` found, a skip with its reason. The browsers are Playwright's: `npx
// playwright install webkit chromium` (PLAYWRIGHT_BROWSERS_PATH chooses where they go). Run
// `npm run build:web` first.

import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { extname, join, normalize, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium, webkit } from 'playwright';

const root = fileURLToPath(new URL('../..', import.meta.url));
const types = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript',
  '.mjs': 'text/javascript',
  '.wasm': 'application/wasm',
  '.zip': 'application/zip',
};

const server = createServer(async (request, response) => {
  const path = normalize(join(root, decodeURIComponent(new URL(request.url, 'http://localhost').pathname)));
  if (!path.startsWith(root.endsWith(sep) ? root : root + sep)) {
    response.writeHead(403).end();
    return;
  }
  try {
    const body = await readFile(path);
    response.writeHead(200, { 'Content-Type': types[extname(path)] ?? 'application/octet-stream' }).end(body);
  } catch {
    response.writeHead(404).end();
  }
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
// A page on the machine itself is a secure context, which the browser's storage needs.
const url = `http://127.0.0.1:${server.address().port}/js/test-browser/page.html`;

const engines = { webkit, chromium };
const names = process.argv.slice(2).length > 0 ? process.argv.slice(2) : ['webkit', 'chromium'];
let failed = false;
try {
  for (const name of names) {
    const engine = engines[name];
    if (!engine) throw new Error(`no browser called ${name}: webkit or chromium`);
    // A profile of its own: a fresh context is private browsing, where WebKit gives no storage.
    const profile = await mkdtemp(join(tmpdir(), `tutti-${name}-`));
    const browser = await engine.launchPersistentContext(profile);
    try {
      const page = browser.pages()[0] ?? (await browser.newPage());
      page.on('pageerror', (error) => console.error(`[${name}] ${error.message}`));
      await page.goto(url);
      const results = await (await page.waitForFunction(() => window.results, null, { timeout: 60_000 })).jsonValue();
      for (const result of results) {
        const skipped = result.skipped ? ` # SKIP ${result.skipped}` : '';
        console.log(`${result.ok ? 'ok' : 'not ok'} - ${name}: ${result.name}${skipped}`);
        if (!result.ok) {
          console.log(`  ${result.error.replaceAll('\n', '\n  ')}`);
          failed = true;
        }
      }
    } finally {
      await browser.close();
      await rm(profile, { recursive: true, force: true });
    }
  }
} finally {
  server.close();
}
process.exitCode = failed ? 1 : 0;
