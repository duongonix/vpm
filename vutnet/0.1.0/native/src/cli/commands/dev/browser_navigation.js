// Client navigation enhances ordinary links and eligible forms. The server
// still renders every target through its regular Router and Controller.
const assetVersion = new URL(import.meta.url).searchParams.get("v");
const assetUrl = path => {
  const url = new URL(path, import.meta.url);
  if (assetVersion !== null) url.searchParams.set("v", assetVersion);
  return url.href;
};
const [{ formNavigationRequest }, { createNavigationPrefetch }] = await Promise.all([
  import(assetUrl("./navigation_forms.js")),
  import(assetUrl("./navigation_prefetch.js")),
]);
const NAVIGATION_STATE = "__vutnetNavigation";
const MAX_REMEMBERED_SCROLL_POSITIONS = 100;

export function installNavigation({
  api,
  hydrate,
  getMount,
  setMount,
  window: win = globalThis.window,
  document: doc = win?.document,
}) {
  if (!win || !doc || !api || typeof api.hydrateView !== "function" ||
      typeof hydrate !== "function" || typeof getMount !== "function" || typeof setMount !== "function") {
    throw new TypeError("Vutnet navigation requires a browser document and hydration runtime");
  }

  const loader = doc.querySelector("script[data-vutnet-client='v1']");
  const clientUrl = loader ? new URL(loader.src, win.location.href).href : null;
  const previousRestoration = win.history.scrollRestoration;
  const scrollPositions = new Map();
  let active = true;
  let activeUrl = win.location.href;
  let requestSequence = 0;
  let controller = null;

  win.history.scrollRestoration = "manual";
  win[NAVIGATION_STATE] = "ready";

  const rememberScroll = key => {
    scrollPositions.delete(key);
    scrollPositions.set(key, { x: win.scrollX, y: win.scrollY });
    while (scrollPositions.size > MAX_REMEMBERED_SCROLL_POSITIONS) {
      scrollPositions.delete(scrollPositions.keys().next().value);
    }
  };

  const setNavigationStatus = (status, url, method, source, error = null) => {
    win[NAVIGATION_STATE] = status;
    const root = doc.documentElement;
    if (status === "loading") root?.setAttribute("data-vutnet-navigation-state", "loading");
    else root?.removeAttribute("data-vutnet-navigation-state");
    const type = status === "loading" ? "vutnet:navigationstart" :
      status === "ready" ? "vutnet:navigationend" : "vutnet:navigationerror";
    doc.dispatchEvent(new win.CustomEvent(type, {
      detail: { url: url.href, method, source, ...(error ? { error: String(error) } : {}) },
    }));
  };

  const prefetcher = createNavigationPrefetch({
    window: win,
    fetchPage: (url, signal) => win.fetch(url.href, {
      method: "GET",
      credentials: "same-origin",
      headers: { Accept: "text/html, application/xhtml+xml" },
      signal,
    }),
    validate: (response, html, url) => isHtml(response) &&
      !response.headers.has("content-disposition") &&
      !response.headers.has("content-security-policy") &&
      !response.headers.has("content-security-policy-report-only") &&
      Boolean(parseHydratableDocument(html, url, clientUrl, win)),
  });

  const eligibleAnchor = anchor => {
    if (!anchor || anchor.hasAttribute("download") || anchor.relList.contains("external")) return null;
    const target = anchor.getAttribute("target");
    if (target && target.toLowerCase() !== "_self") return null;

    let url;
    try {
      url = new URL(anchor.href, doc.baseURI);
    } catch {
      return null;
    }
    if ((url.protocol !== "http:" && url.protocol !== "https:") ||
        url.origin !== win.location.origin || url.href === win.location.href) return null;
    if (url.pathname === win.location.pathname && url.search === win.location.search &&
        url.hash !== win.location.hash) return null;
    return url;
  };

  const eligibleTarget = event => {
    if (event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey ||
        event.shiftKey || event.altKey) return null;
    return eligibleAnchor(event.target?.closest?.("a[href]"));
  };

  const prefetchAnchor = event => {
    const anchor = event.target?.closest?.("a[href]");
    if (!anchor?.relList.contains("prefetch")) return;
    const target = eligibleAnchor(anchor);
    if (target) prefetcher.prefetch(target);
  };

  const onClick = event => {
    const target = eligibleTarget(event);
    if (!target) return;
    event.preventDefault();
    void navigate(target, "push", { method: "GET" }, "link");
  };

  const onSubmit = event => {
    if (event.defaultPrevented) return;
    const submission = formNavigationRequest(event, win, doc);
    if (!submission) return;
    event.preventDefault();
    if (submission.request.method === "POST") prefetcher.invalidate();
    void navigate(submission.url, "push", submission.request, "form");
  };

  const onPopState = () => {
    rememberScroll(activeUrl);
    void navigate(new URL(win.location.href), "pop", { method: "GET" }, "history");
  };

  const navigate = async (target, mode, request = { method: "GET" }, source = "link") => {
    const sequence = ++requestSequence;
    const method = (request.method || "GET").toUpperCase();
    controller?.abort();
    controller = new win.AbortController();
    setNavigationStatus("loading", target, method, source);
    try {
      const requestUrl = new URL(target.href);
      requestUrl.hash = "";
      let response = null;
      let html;
      let finalUrl;
      let redirected = false;
      const cached = method === "GET" ? await prefetcher.take(requestUrl) : null;
      if (sequence !== requestSequence || !active) return;

      if (cached) {
        html = cached.html;
        finalUrl = new URL(cached.url);
      } else {
        response = await win.fetch(requestUrl.href, {
          ...request,
          method,
          credentials: "same-origin",
          headers: { Accept: "text/html, application/xhtml+xml" },
          signal: controller.signal,
        });
        if (sequence !== requestSequence || !active) return;
        redirected = response.redirected;
        finalUrl = new URL(response.url || requestUrl.href);
        if (target.hash && !finalUrl.hash) finalUrl.hash = target.hash;
        if (finalUrl.origin !== win.location.origin || !isHtml(response) ||
            response.headers.has("content-disposition") ||
            response.headers.has("content-security-policy") ||
            response.headers.has("content-security-policy-report-only")) {
          setNavigationStatus("failed", finalUrl, method, source, "unsupported response");
          if (method === "GET" || redirected) fallback(finalUrl, mode);
          return;
        }
        html = await response.text();
      }
      if (sequence !== requestSequence || !active) return;
      if (target.hash && !finalUrl.hash) finalUrl.hash = target.hash;
      const parsed = parseHydratableDocument(html, finalUrl, clientUrl, win);
      if (!parsed) {
        setNavigationStatus("failed", finalUrl, method, source, "response is not hydratable");
        if (method === "GET" || redirected) fallback(finalUrl, mode);
        return;
      }

      const nextRoot = doc.importNode(parsed.documentElement, true);
      nextRoot.querySelector("script[data-vutnet-client]")?.remove();
      const retainedStylesheets = matchStylesheets(doc.documentElement, nextRoot, win.location.href, finalUrl.href);

      if (mode === "push") rememberScroll(activeUrl);
      getMount()?.dispose();
      setMount(null);
      if (!commitDocument(doc, nextRoot, retainedStylesheets)) {
        setNavigationStatus("failed", finalUrl, method, source, "could not commit document");
        if (method === "GET" || redirected) fallback(finalUrl, mode);
        return;
      }
      if (mode === "push") win.history.pushState(win.history.state, "", finalUrl.href);

      try {
        const nextMount = hydrate(doc.documentElement);
        setMount(nextMount);
      } catch (error) {
        setNavigationStatus("failed", finalUrl, method, source, error);
        console.error("Vutnet client navigation hydration failed; loading the page normally", error);
        if (method === "GET" || redirected) fallback(finalUrl, mode);
        return;
      }

      activeUrl = finalUrl.href;
      setNavigationStatus("ready", finalUrl, method, source);
      await restorePosition(finalUrl, mode, scrollPositions, win, doc);
    } catch (error) {
      if (error?.name === "AbortError" || sequence !== requestSequence || !active) return;
      setNavigationStatus("failed", target, method, source, error);
      console.error("Vutnet client navigation failed", error);
      if (method === "GET") fallback(target, mode);
    }
  };

  const fallback = (url, mode) => {
    if (mode === "pop") win.location.reload();
    else win.location.assign(url.href);
  };

  doc.addEventListener("click", onClick);
  doc.addEventListener("submit", onSubmit);
  doc.addEventListener("pointerover", prefetchAnchor);
  doc.addEventListener("focusin", prefetchAnchor);
  win.addEventListener("popstate", onPopState);

  return Object.freeze({
    dispose() {
      if (!active) return;
      active = false;
      requestSequence++;
      controller?.abort();
      prefetcher.dispose();
      doc.removeEventListener("click", onClick);
      doc.removeEventListener("submit", onSubmit);
      doc.removeEventListener("pointerover", prefetchAnchor);
      doc.removeEventListener("focusin", prefetchAnchor);
      win.removeEventListener("popstate", onPopState);
      doc.documentElement?.removeAttribute("data-vutnet-navigation-state");
      win.history.scrollRestoration = previousRestoration;
    },
  });
}

function commitDocument(doc, nextRoot, retainedStylesheets) {
  const currentRoot = doc.documentElement;
  const currentHead = doc.head;
  const currentBody = doc.body;
  const nextHead = nextRoot.querySelector("head");
  const nextBody = nextRoot.querySelector("body");
  if (!currentRoot || !currentHead || !currentBody || !nextHead || !nextBody) return false;

  for (const [nextLink, currentLink] of retainedStylesheets) {
    const marker = nextLink.getAttribute("data-vut-site");
    if (marker === null) currentLink.removeAttribute("data-vut-site");
    else currentLink.setAttribute("data-vut-site", marker);
  }
  syncAttributes(currentRoot, nextRoot);
  syncAttributes(currentHead, nextHead);
  syncAttributes(currentBody, nextBody);
  commitChildren(currentHead, nextHead, retainedStylesheets);
  commitChildren(currentBody, nextBody, retainedStylesheets);
  return true;
}

function commitChildren(currentParent, nextParent, retainedNodes) {
  const children = [...nextParent.childNodes].map(node => retainedNodes.get(node) ?? node);
  const retained = new Set(children);
  let cursor = null;
  for (let index = children.length - 1; index >= 0; index--) {
    const child = children[index];
    if (child.parentNode !== currentParent || child.nextSibling !== cursor) {
      currentParent.insertBefore(child, cursor);
    }
    cursor = child;
  }
  for (const child of [...currentParent.childNodes]) {
    if (!retained.has(child)) child.remove();
  }
}

function syncAttributes(current, next) {
  for (const attribute of [...current.attributes]) {
    if (!next.hasAttribute(attribute.name)) current.removeAttribute(attribute.name);
  }
  for (const attribute of next.attributes) {
    if (current.getAttribute(attribute.name) !== attribute.value) {
      current.setAttribute(attribute.name, attribute.value);
    }
  }
}

function isHtml(response) {
  const contentType = response.headers.get("content-type")?.toLowerCase() ?? "";
  return contentType.includes("text/html") || contentType.includes("application/xhtml+xml");
}

function parseHydratableDocument(source, responseUrl, expectedClientUrl, win) {
  const parsed = new win.DOMParser().parseFromString(source, "text/html");
  const bootstraps = parsed.querySelectorAll("script[data-vutnet-bootstrap='v2']");
  const loaders = parsed.querySelectorAll("script[data-vutnet-client='v1']");
  if (bootstraps.length !== 1 || loaders.length !== 1 || !expectedClientUrl) return null;
  if (bootstraps[0].type !== "application/json" || loaders[0].type !== "module") return null;
  if (new URL(loaders[0].src, responseUrl).href !== expectedClientUrl) return null;
  if (parsed.querySelector("meta[http-equiv='Content-Security-Policy' i]")) return null;

  // Imported documents cannot execute page scripts like a normal navigation.
  // Keep this enhancement conservative and let the browser handle such pages.
  for (const script of parsed.querySelectorAll("script")) {
    if (script !== bootstraps[0] && script !== loaders[0]) return null;
  }
  try {
    const bootstrap = JSON.parse(bootstraps[0].textContent ?? "");
    if (bootstrap.protocol !== "vutnet.hydration" || bootstrap.version !== 2 ||
        typeof bootstrap.page_id !== "string" ||
        bootstrap.plan?.version !== 2 || !Array.isArray(bootstrap.plan.nodes)) return null;
  } catch {
    return null;
  }
  return parsed;
}

function matchStylesheets(currentRoot, nextRoot, currentUrl, nextUrl) {
  const current = new Map();
  for (const link of currentRoot.querySelectorAll("link[rel~='stylesheet'][href]")) {
    const key = stylesheetKey(link, currentUrl);
    if (key) current.set(key, link);
  }
  const retained = new Map();
  for (const link of nextRoot.querySelectorAll("link[rel~='stylesheet'][href]")) {
    const key = stylesheetKey(link, nextUrl);
    const match = key && current.get(key);
    if (match) retained.set(link, match);
  }
  return retained;
}

function stylesheetKey(link, baseUrl) {
  try {
    // Hydration site IDs differ by Page; retained nodes get the new marker at commit.
    const attrs = [...link.attributes]
      .filter(attribute => attribute.name !== "href" && attribute.name !== "data-vut-site")
      .map(attribute => [attribute.name, attribute.value])
      .sort(([left], [right]) => left.localeCompare(right));
    return JSON.stringify([new URL(link.getAttribute("href"), baseUrl).href, attrs]);
  } catch {
    return null;
  }
}

async function restorePosition(url, mode, positions, win, doc) {
  await new Promise(resolve => win.requestAnimationFrame(() => win.requestAnimationFrame(resolve)));
  if (mode === "pop" && positions.has(url.href)) {
    const position = positions.get(url.href);
    win.scrollTo(position.x, position.y);
    return;
  }
  if (url.hash) {
    let id = url.hash.slice(1);
    try { id = decodeURIComponent(id); } catch { /* use the original fragment */ }
    const target = doc.getElementById(id);
    if (target) {
      target.scrollIntoView();
      return;
    }
  }
  win.scrollTo(0, 0);
}
