const MAX_ENTRIES = 20;
const MAX_AGE_MS = 30_000;

// Prefetch is cache-header gated so personalized SSR pages without an explicit
// freshness policy are never reused as stale navigation responses.
export function createNavigationPrefetch({ window: win, fetchPage, validate }) {
  const entries = new Map();
  const pending = new Map();
  const controllers = new Set();
  let active = true;
  let generation = 0;

  const remember = (key, value) => {
    entries.delete(key);
    entries.set(key, value);
    while (entries.size > MAX_ENTRIES) entries.delete(entries.keys().next().value);
  };

  const prefetch = url => {
    const key = keyFor(url);
    if (!active || !key || entries.has(key) || pending.has(key) || pending.size >= 4) return;
    const controller = new win.AbortController();
    const requestGeneration = generation;
    controllers.add(controller);
    let request;
    request = fetchPage(new URL(key), controller.signal)
      .then(async response => {
        if (!response.ok || !isCacheable(response)) return null;
        const finalUrl = new URL(response.url || key);
        if (finalUrl.origin !== win.location.origin) return null;
        const html = await response.text();
        if (!active || generation !== requestGeneration || !validate(response, html, finalUrl)) return null;
        const ttl = cacheLifetime(response.headers);
        if (ttl <= 0) return null;
        const entry = { html, url: finalUrl.href, expiresAt: Date.now() + ttl };
        remember(key, entry);
        return entry;
      })
      .catch(() => null)
      .finally(() => {
        if (pending.get(key) === request) pending.delete(key);
        controllers.delete(controller);
      });
    pending.set(key, request);
  };

  const take = async url => {
    const key = keyFor(url);
    if (!key) return null;
    let entry = entries.get(key);
    if (entry && entry.expiresAt <= Date.now()) {
      entries.delete(key);
      entry = null;
    }
    if (!entry && pending.has(key)) entry = await pending.get(key);
    if (!entry || entry.expiresAt <= Date.now()) return null;
    remember(key, entry);
    return entry;
  };

  return Object.freeze({
    prefetch,
    take,
    invalidate() {
      generation++;
      entries.clear();
      for (const controller of controllers) controller.abort();
      controllers.clear();
      pending.clear();
    },
    dispose() {
      active = false;
      generation++;
      for (const controller of controllers) controller.abort();
      controllers.clear();
      pending.clear();
      entries.clear();
    },
  });
}

function keyFor(url) {
  const key = new URL(url.href);
  key.hash = "";
  return key.href;
}

function isCacheable(response) {
  if (response.headers.has("content-disposition") ||
      response.headers.has("content-security-policy") ||
      response.headers.has("content-security-policy-report-only")) return false;
  if ((response.headers.get("vary") ?? "").split(",").some(value => value.trim() === "*")) return false;
  return cacheLifetime(response.headers) > 0;
}

function cacheLifetime(headers) {
  const directives = (headers.get("cache-control") ?? "")
    .toLowerCase().split(",").map(value => value.trim());
  if (directives.some(value => value === "no-store" || value === "no-cache")) return 0;
  const maxAge = directives.find(value => value.startsWith("max-age="));
  if (!maxAge) return 0;
  const seconds = Number(maxAge.slice("max-age=".length).replaceAll('"', ""));
  if (!Number.isFinite(seconds) || seconds <= 0) return 0;
  return Math.min(seconds * 1000, MAX_AGE_MS);
}
