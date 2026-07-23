import assert from 'node:assert/strict';
import { chromium } from 'playwright';

const input = process.argv[2];
if (!input) {
  throw new Error('usage: node scripts/smoke-www-demo.mjs <deployment-url>');
}

const origin = new URL(input);
const url = (pathname) => new URL(pathname, origin).href;
const bypass = process.env.VERCEL_AUTOMATION_BYPASS_SECRET?.trim();
const headers = bypass ? { 'x-vercel-protection-bypass': bypass } : {};
const allowLocalDevHeaders =
  process.env.SPOCK_WWW_DEMO_DEV === '1' &&
  (origin.hostname === '127.0.0.1' || origin.hostname === 'localhost');

const checkedFetch = async (pathname, expectedType) => {
  const response = await fetch(url(pathname), {
    headers,
    redirect: 'follow',
  });
  assert.equal(response.status, 200, `${pathname} returned ${response.status}`);
  if (expectedType) {
    const actual = response.headers.get('content-type')?.split(';', 1)[0];
    const localEditorStateException =
      allowLocalDevHeaders &&
      pathname === '/demo/api/editor/state' &&
      (actual === undefined || actual === '');
    if (!localEditorStateException) {
      assert.equal(actual, expectedType, `${pathname} has Content-Type ${actual}`);
    }
  }
  return response;
};

const checkedRedirect = async (pathname, destination) => {
  const response = await fetch(url(pathname), {
    headers,
    redirect: 'manual',
  });
  assert.equal(response.status, 308, `${pathname} returned ${response.status}`);
  const location = response.headers.get('location');
  assert.ok(location, `${pathname} has no redirect location`);
  assert.equal(new URL(location, origin).pathname, destination);
};

const checkedMissing = async (pathname) => {
  const response = await fetch(url(pathname), {
    headers,
    redirect: 'manual',
  });
  assert.equal(response.status, 404, `${pathname} returned ${response.status}`);
};

await checkedFetch('/', 'text/html');
await checkedFetch('/demo/', 'text/html');
await checkedFetch('/demo/play', 'text/html');
await checkedFetch('/demo/play/', 'text/html');
await checkedFetch('/demo/_uhura/editor/', 'text/html');
await checkedFetch('/demo/history-fallback-probe', 'text/html');
await checkedMissing('/demo/api/history-fallback-probe');
await checkedMissing('/demo/assets/history-fallback-probe.js');
await checkedRedirect('/demo/create/', '/demo/create');
await checkedRedirect('/demo/reels/', '/demo/reels');
await checkedRedirect('/demo/search/', '/demo/search');
await checkedRedirect('/demo/p/post-lena-glaze/', '/demo/p/post-lena-glaze');
await checkedRedirect('/demo/profile/user-nils/', '/demo/profile/user-nils');
await checkedRedirect(
  '/demo/profile/user-nils/followers/',
  '/demo/profile/user-nils/followers',
);
await checkedRedirect(
  '/demo/profile/user-nils/following/',
  '/demo/profile/user-nils/following',
);
await checkedRedirect('/demo/stories/ring-lena/', '/demo/stories/ring-lena');
await checkedFetch('/demo/api/editor/state', 'application/json');
await checkedFetch(
  '/demo/api/play/wasm/uhura_wasm_bg.wasm',
  'application/wasm',
);

const manifest = await (
  await checkedFetch('/demo/uhura-static-bundle.json', 'application/json')
).json();
assert.equal(manifest.protocol, 'uhura-static-web-bundle/0');
assert.equal(manifest.mountPath, '/demo/');
assert.equal(manifest.playEntry, '/demo/play');
assert.ok(Number.isSafeInteger(manifest.previews) && manifest.previews > 0);

const staticPlay = await (
  await checkedFetch('/demo/api/play/static.json', 'application/json')
).json();
assert.equal(staticPlay.protocol, 'uhura-static-play/0');

const browser = await chromium.launch({ headless: true });
const browserErrors = [];
const backendRequests = [];

try {
  const context = await browser.newContext();
  if (bypass) {
    await context.route('**/*', async (route) => {
      const request = route.request();
      if (new URL(request.url()).origin === origin.origin) {
        await route.continue({
          headers: { ...request.headers(), ...headers },
        });
        return;
      }
      await route.continue();
    });
  }
  context.on('request', (request) => {
    const pathname = new URL(request.url()).pathname;
    if (
      pathname === '/~project/environment' ||
      pathname === '/graphql/v1' ||
      pathname.startsWith('/rest/v1/rpc') ||
      pathname.startsWith('/storage/v1')
    ) {
      backendRequests.push(request.url());
    }
  });
  const watch = (page, surface) => {
    page.on('console', (message) => {
      if (message.type() === 'error') {
        browserErrors.push(`${surface} console: ${message.text()}`);
      }
    });
    page.on('pageerror', (error) => {
      browserErrors.push(`${surface} page: ${error.message}`);
    });
    page.on('requestfailed', (request) => {
      browserErrors.push(
        `${surface} request: ${request.url()} (${request.failure()?.errorText ?? 'failed'})`,
      );
    });
  };

  const editor = await context.newPage();
  watch(editor, 'Editor');
  await editor.goto(url('/demo/'), {
    waitUntil: 'domcontentloaded',
    timeout: 60_000,
  });
  await editor.waitForFunction(
    (expected) =>
      document.querySelectorAll('.editor-frame[data-preview-id]').length ===
      expected,
    manifest.previews,
    { timeout: 60_000 },
  );

  const play = await context.newPage();
  watch(play, 'Play');
  await play.goto(url('/demo/play'), {
    waitUntil: 'domcontentloaded',
    timeout: 60_000,
  });
  await play.waitForFunction(
    () => window.__uhura?.system.status === 'ready',
    undefined,
    { timeout: 60_000 },
  );
  await play.waitForLoadState('networkidle', { timeout: 60_000 });

  const like = play.getByRole('button', { name: 'Like', exact: true }).first();
  await like.waitFor({ state: 'visible', timeout: 30_000 });

  const runtime = await play.evaluate(() => ({
    status: window.__uhura?.system.status,
    hasProvider: window.__uhura?.system.hasProvider,
    actors: window.__uhura?.system.actors.length,
    actor: window.__uhura?.system.actor,
    canSwitchActor: window.__uhura?.system.canSwitchActor,
    hasSession: Boolean(window.__uhura?.session),
    hasProviderHandle: Boolean(window.__uhura?.provider),
    providerInfo: window.__uhura?.provider?.systemInfo(),
  }));
  assert.equal(runtime.status, 'ready');
  assert.equal(runtime.hasProvider, true);
  assert.equal(runtime.actors, 9);
  assert.equal(runtime.canSwitchActor, true);
  assert.equal(typeof runtime.actor, 'string');
  assert.notEqual(runtime.actor.length, 0);
  assert.equal(runtime.hasSession, true);
  assert.equal(runtime.hasProviderHandle, true);
  assert.equal(runtime.providerInfo?.actors.length, 9);
  assert.equal(typeof runtime.providerInfo?.actor, 'string');
  assert.notEqual(runtime.providerInfo.actor.length, 0);

  await like.click();
  await play
    .getByRole('button', { name: 'Unlike', exact: true })
    .first()
    .waitFor({ state: 'visible', timeout: 30_000 });

  for (const pathname of [
    '/demo/create',
    '/demo/reels',
    '/demo/search',
    '/demo/p/post-lena-glaze',
    '/demo/profile/user-nils',
    '/demo/profile/user-nils/followers',
    '/demo/profile/user-nils/following',
    '/demo/stories/ring-lena',
  ]) {
    await play.goto(url(pathname), {
      waitUntil: 'domcontentloaded',
      timeout: 60_000,
    });
    await play.waitForFunction(
      () => window.__uhura?.system.status === 'ready',
      undefined,
      { timeout: 60_000 },
    );
    await play.waitForLoadState('networkidle', { timeout: 60_000 });
  }

  await context.close();
} finally {
  await browser.close();
}

assert.deepEqual(browserErrors, []);
assert.deepEqual(backendRequests, []);
console.log(
  `Uhura www demo smoke passed: ${manifest.previews} Editor previews and browser-local Play mutation`,
);
