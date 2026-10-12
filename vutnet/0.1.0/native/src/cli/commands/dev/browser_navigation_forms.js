// Convert only forms whose request can be reproduced with same-origin fetch.
// Unsupported targets and encodings stay on the browser's native submit path.
export function formNavigationRequest(event, win, doc) {
  const form = event.target;
  if (form?.localName !== "form") return null;

  const submitter = event.submitter ?? null;
  const target = formOverride(submitter, "formtarget", "formTarget", form.target);
  if (target && target.toLowerCase() !== "_self") return null;

  let url;
  try {
    const action = formOverride(submitter, "formaction", "formAction", form.action);
    url = new URL(action || win.location.href, doc.baseURI);
  } catch {
    return null;
  }
  if ((url.protocol !== "http:" && url.protocol !== "https:") ||
      url.origin !== win.location.origin) return null;

  const method = formOverride(submitter, "formmethod", "formMethod", form.method || "get")
    .toLowerCase();
  if (method !== "get" && method !== "post") return null;

  const data = formData(form, submitter, win);
  if (!data) return null;

  if (method === "get") {
    const query = new URLSearchParams();
    for (const [name, value] of data.entries()) {
      query.append(name, typeof value === "string" ? value : value.name);
    }
    url.search = query.toString();
    return { url, request: { method: "GET" } };
  }

  const enctype = formOverride(
    submitter,
    "formenctype",
    "formEnctype",
    form.enctype || "application/x-www-form-urlencoded",
  ).split(";", 1)[0].trim().toLowerCase();
  if (enctype === "multipart/form-data") {
    return { url, request: { method: "POST", body: data } };
  }
  if (enctype !== "application/x-www-form-urlencoded") return null;

  const body = new URLSearchParams();
  for (const [name, value] of data.entries()) {
    body.append(name, typeof value === "string" ? value : value.name);
  }
  return { url, request: { method: "POST", body } };
}

function formOverride(submitter, attribute, property, fallback) {
  return submitter?.hasAttribute(attribute) ? submitter[property] : fallback;
}

function formData(form, submitter, win) {
  try {
    return submitter ? new win.FormData(form, submitter) : new win.FormData(form);
  } catch {
    // Older browsers may lack FormData(form, submitter); preserve the submitter
    // name/value explicitly while keeping native form validation behavior.
    try {
      const data = new win.FormData(form);
      if (submitter?.name && !submitter.disabled) data.append(submitter.name, submitter.value);
      return data;
    } catch {
      return null;
    }
  }
}
