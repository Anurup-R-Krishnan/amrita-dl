const OCI_ORIGIN = "https://enable-helicopter-carried-melbourne.trycloudflare.com";

function sanitizeDownloadFilename(name) {
  if (!name) return null;
  let safe = name.replace(/[\r\n"\\]/g, "").replace(/[^\w .()-]/g, "_").trim().slice(0, 150);
  if (!safe) return null;
  if (!safe.toLowerCase().endsWith(".pdf")) safe += ".pdf";
  return safe;
}

export async function onRequest(context) {
  const url = new URL(context.request.url);
  const target = OCI_ORIGIN + url.pathname + url.search;

  if (context.request.method === "OPTIONS") {
    return new Response(null, {
      status: 204,
      headers: {
        "Access-Control-Allow-Origin": "*",
        "Access-Control-Allow-Methods": "GET, OPTIONS",
        "Access-Control-Allow-Headers": "Content-Type",
      },
    });
  }

  const method = context.request.method;
  const resp = await fetch(target, {
    method,
    headers: context.request.headers,
    body: method !== "GET" && method !== "HEAD" ? context.request.body : undefined,
  });

  const newHeaders = new Headers(resp.headers);
  newHeaders.set("Access-Control-Allow-Origin", "*");

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
}
