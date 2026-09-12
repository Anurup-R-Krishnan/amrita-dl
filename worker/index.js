const OCI_ORIGIN = "https://enable-helicopter-carried-melbourne.trycloudflare.com";
const ALLOWED_PREFIXES = ["/api/"];

function sanitizeDownloadFilename(name) {
  if (!name) return null;
  let safe = name.replace(/[\r\n"\\]/g, "").replace(/[^\w .()-]/g, "_").trim().slice(0, 150);
  if (!safe) return null;
  if (!safe.toLowerCase().endsWith(".pdf")) safe += ".pdf";
  return safe;
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const isApi = ALLOWED_PREFIXES.some(p => url.pathname.startsWith(p));
    if (!isApi) {
      return new Response("Not Found", { status: 404 });
    }
    const target = OCI_ORIGIN + url.pathname + url.search;
    const proxied = new Request(target, {
      method: request.method,
      headers: request.headers,
      body: request.method !== "GET" && request.method !== "HEAD" ? request.body : undefined,
      redirect: "follow",
    });
    const resp = await fetch(proxied);
    const newHeaders = new Headers(resp.headers);
    newHeaders.set("Access-Control-Allow-Origin", "*");
    newHeaders.set("Access-Control-Allow-Methods", "GET, OPTIONS");
    newHeaders.set("Access-Control-Allow-Headers", "Content-Type");
    if (request.method === "OPTIONS") {
      return new Response(null, { status: 204, headers: newHeaders });
    }
    if (url.pathname === "/api/pdf" && !newHeaders.has("Content-Disposition")) {
      const safeName = sanitizeDownloadFilename(url.searchParams.get("dl"));
      if (safeName) {
        newHeaders.set("Content-Disposition", `attachment; filename="${safeName}"`);
      }
    }
    return new Response(resp.body, {
      status: resp.status,
      headers: newHeaders,
    });
  },
};
