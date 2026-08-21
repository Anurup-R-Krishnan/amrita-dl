# Frontend Proxying & Edge Handlers

**29. API Parsing HTML Errors**

*The Pitfall (Deep Context):*
Everything looked perfect in local testing. We deployed the SPA onto Cloudflare Pages. Users typed queries into the search bar expecting instant JSON results. Instead the entire frontend crashed, throwing `SyntaxError: Unexpected token '<' in JSON at position 0`.

We blindly assumed our proxy was working. Running a raw `curl` against the deployed endpoint revealed the horror: Cloudflare Pages had dropped our backend API proxy entirely. Instead of routing `/api/search` traffic into our sub-millisecond SQLite backend, Cloudflare intercepted the request and served the `index.html` blob instead. The frontend was literally trying to parse the raw HTML of its own homepage as a JSON database array. The `<` at position 0 was the first character of `<!DOCTYPE html>`.

*How we faced it:*
We stopped browser-based debugging completely. Browsers hide headers, cache aggressively, and follow redirects silently, all of which masked the true failure. We relied purely on Unix `curl` commands to inspect the physical response headers:
```bash
curl -i https://exampapersamrita.pages.dev/api/status
```
This showed `content-type: text/html` where `application/json` belonged, proving the failure was at the Edge routing layer, not the Rust backend. We then verified the backend itself was alive by curling it directly through the tunnel. With the layer of failure isolated, we could fix the actual broken component instead of guessing.

-> *Verify: `curl -I https://exampapersamrita.pages.dev/api/status` returns `content-type: application/json`.*


**30. Pages Catch-All Override**

*The Pitfall (Deep Context):*
We eventually realized exactly why our JSON payloads had turned into HTML blobs. Our original routing mechanism relied on a standard `_redirects` file instructing Cloudflare to forward `/api/*` to the Oracle VM.

What we had not documented was that Cloudflare Pages uses SPA (Single Page Application) catch-all patterns. Because we enabled SPA routing so that frontend router links would not 404, Cloudflare prioritized the SPA catch-all rule above our `_redirects` file. It evaluated `/api/*` as a missing frontend page and unconditionally fell back to serving `index.html`. Our API routing was dead by design, silently and with no error anywhere in the deploy logs.

*How we faced it:*
We abandoned static proxy files entirely. We deleted `_redirects` from the repository and substituted programmatic Edge interceptors: Cloudflare Pages Functions. Functions execute before static asset resolution, which means they preempt the SPA catch-all instead of competing with it. API routes became code, not config, and code always wins the routing order.

-> *Verify: `ls _redirects` returns `No such file or directory` in the deploy root, and `/api/*` requests never return HTML.*


**31. Developing Full Cloudflare Functions**

*The Pitfall (Deep Context):*
Throwing away `_redirects` meant we had to construct a Cloudflare Pages Function from scratch to bridge the gap. Programming Edge functions to bridge our TLS tunnel spawned a fresh set of architectural issues. We attempted to merge fetch queries across ephemeral `trycloudflare.com` hosts, but the browser blocked them as cross-origin. Proxying raw fetch requests through a serverless function introduced CORS complexities that broke the browser mechanically.

*How we faced it:*
We engineered a wildcard route using `functions/api/[[path]].js`. Acting at the Edge, this function intercepts every `/api/` path, extracts the query, and executes an explicit `fetch()` toward the TLS tunnel hostname. Because the function runs on the same domain the browser is already talking to, the browser believes it is querying the exact same origin. CORS ceases to exist as a concept for the frontend:
```js
export async function onRequest(context) {
  const url = new URL(context.request.url);
  return fetch(`https://<tunnel-host>${url.pathname}${url.search}`, context.request);
}
```
The Edge function is a literal same-origin middleman.

-> *Verify: `cat functions/api/[[path]].js` shows the proxy definition, and a browser search returns results with no CORS errors in the console.*


**32. Functions Directory Obfuscation**

*The Pitfall (Deep Context):*
After writing the clever `[[path]].js` function, we ran Wrangler to deploy it, only to find the Edge API still 404ing. We scoured the logs and discovered Wrangler had completely ignored the code we just wrote.

We had accidentally grouped our logic inside `web/functions/`, treating it like frontend code. Wrangler rigidly respects root definitions: it requires the `functions/` directory to exist at the absolute root of the deployed repository, entirely independent of the `web/` static payload directory. It silently dropped our buried architecture without a single warning in the build output. No error, no log line, just a missing route.

*How we faced it:*
We structurally migrated the code with `mv web/functions functions/`, placing it exactly at the repository root boundary. By conforming to Cloudflare's rigid directory contract instead of fighting it, the deployment immediately picked up the function and the API route went live on the next deploy.

-> *Verify: `ls -ld functions` succeeds at the repo root, and `functions/api/[[path]].js` exists at that exact level.*


**33. Pre-flight CORS Restraints (OPTIONS)**

*The Pitfall (Deep Context):*
Although Edge functions solved the direct fetch, Chrome and Firefox still slaughtered standard frontend API integrations because of pre-flight checks. Before a browser sends a POST or a complex GET with custom headers, it sends an `OPTIONS` request first. Our Cloudflare Function blindly proxied this `OPTIONS` request to the Rust backend, which had no handler for it. The intercepted response lacked the explicit `Access-Control-Allow-Origin` header. The browser blocked the actual search request before it ever left the machine, and the user saw nothing but a silent failure.

*How we faced it:*
We overhauled the proxy intercept inside `[[path]].js` with an explicit method check: if the request method is `OPTIONS`, the function instantly returns HTTP `204 No Content` with `Access-Control-Allow-Origin: *` and the allowed methods/headers attached. This satisfies the browser's pre-flight paranoia at the Edge without ever touching the Rust backend:
```js
if (context.request.method === "OPTIONS") {
  return new Response(null, {
    status: 204,
    headers: {
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
      "Access-Control-Allow-Headers": "Content-Type",
    },
  });
}
```
The backend never needs to know CORS exists.

-> *Verify: `curl -X OPTIONS -I https://exampapersamrita.pages.dev/api/` replies exactly `HTTP/2 204` with `access-control-allow-origin: *` in the headers.*


**34. Fallback API Parsing Toggles**

*The Pitfall (Deep Context):*
While developing locally, the frontend parsed API URL paths using a global `getApiBase()` utility. Our string concatenation was flawed: it formulated duplicate forward slashes, producing paths like `//api//search`. Local dev environments resolve these gracefully. But Cloudflare Edge route definitions treat double slashes as distinct paths, instantly throwing HTTP 400s that map to missing-resource 404s. The same code worked on our laptops and died in production, the worst class of bug.

*How we faced it:*
We stripped the arbitrary conditional checks inside `getApiBase()` and replaced them with exact string manipulation. By strictly verifying zero-length segment joins, we guaranteed single-slash resolution regardless of environment:
```js
const base = import.meta.env.VITE_API_BASE ?? "";
export function getApiBase() {
  return base.replace(/\/+$/, "");
}
```
The base never carries a trailing slash, the route always carries exactly one leading slash, and the concatenation can no longer produce doubles.

-> *Verify: Browser devtools network tab shows requests hitting exactly `/api/search` with a single slash, and no 400/404s appear for API calls.*
