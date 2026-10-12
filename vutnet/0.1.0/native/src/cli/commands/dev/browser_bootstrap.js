const assetVersion = new URL(import.meta.url).searchParams.get("v");
const assetUrl = path => {
  const url = new URL(path, import.meta.url);
  if (assetVersion !== null) url.searchParams.set("v", assetVersion);
  return url;
};
const [{ instantiate }, { installNavigation }] = await Promise.all([
  import(assetUrl("./client.js").href),
  import(assetUrl("./navigation.js").href),
]);

let registry;
let api;
let mount;
let navigation;
window.__vutnetHydration = "loading";

try {
  const response = await fetch(assetUrl("./client.wasm"));
  if (!response.ok) {
    throw new Error(`browser client request failed: ${response.status}`);
  }

  const registryResponse = await fetch(assetUrl("./client.registry.json"));
  if (!registryResponse.ok) throw new Error(`client registry request failed: ${registryResponse.status}`);
  registry = await registryResponse.json();
  if (registry.protocol !== "vutnet.client-registry" || registry.version !== 1 ||
      !Array.isArray(registry.pages)) throw new TypeError("unsupported Vutnet client registry");

  api = await instantiate(await response.arrayBuffer(), {
    vut: { _view_render_native: () => 0 },
  });
  const hydrate = container => {
    const element = container.ownerDocument.querySelector("script[data-vutnet-bootstrap='v2']");
    if (!element) throw new TypeError("Vutnet hydration bootstrap is missing");
    const bootstrap = JSON.parse(element.textContent ?? "");
    const page = registry.pages.find(entry => entry.id === bootstrap.page_id);
    if (!page) throw new TypeError(`no browser Page registered for ${bootstrap.page_id}`);
    const factory = api[page.factory];
    if (typeof factory !== "function") throw new TypeError(`browser Page ${page.factory} is not exported`);
    const options = page.decoder === null ? {} : { decode: api[page.decoder] };
    if (page.decoder !== null && typeof options.decode !== "function") {
      throw new TypeError(`browser decoder ${page.decoder} is not exported`);
    }
    return api.hydrateView(container, factory, options);
  };
  mount = hydrate(document.documentElement);
  navigation = installNavigation({
    api,
    hydrate,
    getMount: () => mount,
    setMount: value => { mount = value; },
  });
  window.__vutnetHydration = "hydrated";
  window.addEventListener(
    "pagehide",
    () => {
      navigation.dispose();
      mount?.dispose();
      api.dispose();
    },
    { once: true },
  );
} catch (error) {
  navigation?.dispose();
  mount?.dispose();
  api?.dispose();
  window.__vutnetHydration = "failed";
  console.error("Vutnet browser hydration failed", error);
}
