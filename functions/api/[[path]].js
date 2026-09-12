// Fallback origin used when KV has no published value yet (fresh deploy,
// KV binding missing in preview, etc). The VM keeps this current via
// POST /api/_origin whenever its cloudflared quick-tunnel URL changes -
// see scripts/amrita_selfheal.sh. Update this manually only as a last resort.
const FALLBACK_ORIGIN = "https://enable-helicopter-carried-melbourne.trycloudflare.com";

const TUNNEL_URL_RE = /^https:\/\/[a-z0-9-]+\.trycloudflare\.com$/;

function sanitizeDownloadFilename(name) {
  if (!name) return null;
  let safe = name.replace(/[\r\n"\\]/g, "").replace(/[^\w .()-]/g, "_").trim().slice(0, 150);
  if (!safe) return null;
  if (!safe.toLowerCase().endsWith(".pdf")) safe += ".pdf";
  return safe;
}

async function handleOriginUpdate(context) {
  const { request, env } = context;
  if (request.method !== "POST") {
    return new Response("Method Not Allowed", { status: 405 });
  }
  if (!env.ORIGIN_UPDATE_SECRET) {
    return new Response("Not configured", { status: 501 });
  }
  const auth = request.headers.get("Authorization") || "";
  if (auth !== `Bearer ${env.ORIGIN_UPDATE_SECRET}`) {
    return new Response("Forbidden", { status: 403 });
  }
  const body = (await request.text()).trim();
  if (!TUNNEL_URL_RE.test(body)) {
    return new Response("Invalid origin URL", { status: 400 });
  }
  if (!env.ORIGIN_KV) {
    return new Response("KV not bound", { status: 501 });
  }
  await env.ORIGIN_KV.put("current", body);
  return new Response("OK", { status: 200 });
}

export async function onRequest(context) {
  const { request, env } = context;
  const url = new URL(request.url);

  if (url.pathname === "/api/_origin") {
    return handleOriginUpdate(context);
  }

  let origin = FALLBACK_ORIGIN;
  try {
    const kvValue = env.ORIGIN_KV ? await env.ORIGIN_KV.get("current", { cacheTtl: 60 }) : null;
    if (kvValue && TUNNEL_URL_RE.test(kvValue)) {
      origin = kvValue;
    }
  } catch (e) {
    // KV unavailable - fall through to FALLBACK_ORIGIN
  }

  const target = origin + url.pathname + url.search;

  if (request.method === "OPTIONS") {
    return new Response(null, {
      status: 204,
      headers: {
        "Access-Control-Allow-Origin": "*",
        "Access-Control-Allow-Methods": "GET, OPTIONS",
        "Access-Control-Allow-Headers": "Content-Type",
      },
    });
  }

  const method = request.method;
  const resp = await fetch(target, {
    method,
    headers: request.headers,
    body: method !== "GET" && method !== "HEAD" ? request.body : undefined,
  });

  const newHeaders = new Headers(resp.headers);
  newHeaders.set("Access-Control-Allow-Origin", "*");

  if (url.pathname === "/api/pdf" && !newHeaders.has("Content-Disposition")) {
    const safeName = sanitizeDownloadFilename(url.searchParams.get("dl"));
    if (safeName) {
      // inline (not attachment): lets the modal's <iframe> preview render the
      // PDF in-browser, while still supplying the real filename for the
      // browser's own Save-As / native PDF-viewer download button. Our own
      // download links force a save anyway via the HTML `download` attribute,
      // which doesn't depend on this header.
      newHeaders.set("Content-Disposition", `inline; filename="${safeName}"`);
    }
  }

  return new Response(resp.body, {
    status: resp.status,
    headers: newHeaders,
  });
}
