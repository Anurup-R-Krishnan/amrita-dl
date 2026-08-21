const OCI_ORIGIN = "https://pens-regulations-cheats-devoted.trycloudflare.com";
const ALLOWED_PREFIXES = ["/api/"];

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
    return new Response(resp.body, {
      status: resp.status,
      headers: newHeaders,
    });
  },
};
