const OCI_ORIGIN = "https://pens-regulations-cheats-devoted.trycloudflare.com";

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

  const resp = await fetch(target, {
    method: context.request.method,
    headers: context.request.headers,
  });

  const newHeaders = new Headers(resp.headers);
  newHeaders.set("Access-Control-Allow-Origin", "*");

  return new Response(resp.body, {
    status: resp.status,
    headers: newHeaders,
  });
}
