# Frontend Proxying & Edge Handlers

**29. API Parsing HTML Errors**
*The Pitfall (Deep Context):* Everything looked perfect in local testing. We deployed the React/Alpine SPA directly onto Cloudflare Pages. Our frontend users typed query data into the search bar, expecting instant JSON results. Instead, the entire frontend violently crashed, throwing `SyntaxError: Unexpected token < in JSON at position 0`. 
We blindly assumed our proxies were working. We executed a raw `curl` into the React DOM and discovered, to our absolute horror, that Cloudflare Pages had intrinsically dropped our backend API proxy queries entirely. Instead of routing traffic into our sub-millisecond SQLite backend, Cloudflare intercepted the `/api/search` requests and fed the user the `index.html` Blob instead. The frontend was literally trying to parse the raw HTML code of its own homepage as a JSON database array.

*How we faced it:* We aggressively verified backend array outputs securely, proving the Rust server was alive. We then isolated the frontend logic, stopping any browser-based debugging, and relied purely on native Unix `curl` commands to test the physical headers of the proxy responses until we forced Cloudflare to respect the JSON boundaries.
-> *Verify: `curl -I https://exampapersamrita.pages.dev/api/status` returns strictly `application/json` content-types validating true backend isolation.*


**30. Pages Catch-All Override**
*The Pitfall (Deep Context):* We eventually realized exactly *why* our JSON payloads had magically turned into HTML blobs. Our original routing mechanism relied completely on a standard `_redirects` file, instructing Cloudflare to forward ` /api/* ` to the Oracle VM. 
What we didn't document was that Cloudflare Pages uses native SPA (Single Page Application) override patterns. Because we enabled SPA routing (so that frontend React router links wouldn't 404), Cloudflare prioritized the SPA catch-all rule *above* our `_redirects` file. It completely ignored our API proxy and evaluated `/api/*` as a missing frontend page, unconditionally falling back to serving `index.html`. Our routing was dead by design.

*How we faced it:* We had to aggressively abandon standard static proxy files. We deleted `_redirects` entirely. Instead, we substituted active Cloudflare edge programmatic interceptors (Cloudflare Functions) which execute *before* the static SPA resolution, completely safely bypassing the SPA default catch-all dynamically.
-> *Verify: `ls _redirects` generates a 'No such file' exception proving static overrides exist completely nullified.*


**31. Developing Full Cloudflare Functions**
*The Pitfall (Deep Context):* Throwing away `_redirects` meant we had to construct a Cloudflare Page Function from scratch to bridge the gap. Programming Edge functions to explicitly bridge our TLS tunnels spawned nightmare architectural issues. We attempted to merge fetch queries across `trycloudflare.com` setups, but the browser natively blocked them. Trying to proxy raw HTTP fetch requests via a serverless function introduced massive cross-origin complexities that broke CORS mechanically.

*How we faced it:* We engineered surgical wildcard routes using `[[path]].js`. Acting inherently at the Edge, this function intercepts the `/api/` path, extracts the query, and executes an explicit Node `fetch()` destination dynamically onto the TLS tunnel. By acting as a literal middleman on the Edge, it automatically overrides local browser CORS states flawlessly, making the browser believe it is querying the same exact domain.
-> *Verify: `cat functions/api/[[path]].js` explicitly maps proxy definitions natively bypassing local states perfectly.*


**32. Functions Directory Obfuscation**
*The Pitfall (Deep Context):* After writing the incredibly clever `[[path]].js` function, we ran Wrangler to deploy it, only to find the Edge API completely 404'ing. We scoured the logs and found that Wrangler had completely ignored the code we just wrote. 
We had accidentally grouped our logic inside the `web/functions/` directory, treating it like frontend code. Wrangler, however, rigidly respects root definitions natively. It requires the `functions/` directory to exist at the absolute root of the working repository, entirely independent of the `web/` payload. It silently dropped our buried architecture gracefully.

*How we faced it:* We structurally migrated the codebase with `mv web/functions functions/`, deploying exactly at the repository node boundary. By abandoning the nested frontend structure, we safely bypassed the obfuscation, Bridging Cloudflare's rigid deployment requirements cleanly.
-> *Verify: `ls -ld functions` maps successfully inside the root tree avoiding nested SPA obfuscations completely.*


**33. Pre-flight CORS Restraints (OPTIONS)**
*The Pitfall (Deep Context):* Although Edge functions solved the direct fetch, Chrome and Firefox aggressively slaughtered standard frontend API integrations because of pre-flight checks. Before a browser sends a POST or a complex GET, it sends an `OPTIONS` request. Our Cloudflare Function was blindly proxying this `OPTIONS` request to the Rust backend, which had no idea how to handle it. The intercepted response lacked the explicit `Access-Control-Allow-Origin` headers, heavily blocking execution safely and killing the user's search query before it even left the browser.

*How we faced it:* We structurally overhauled the proxy intercepts within `[[path]].js`. We injected an arbitrary header check: if the request method is `OPTIONS`, the script instantly returns a strict HTTP `204 No Content` response injected with `Access-Control-Allow-Origin: *`. This satisfies the browser's pre-flight paranoia natively, flawlessly overriding tracking bounds dynamically and accurately.
-> *Verify: `curl -X OPTIONS -I https://exampapersamrita.pages.dev/api/` replies exactly 204 explicitly confirming CORS bypass configurations safely.*


**34. Fallback API Parsing Toggles**
*The Pitfall (Deep Context):* While developing locally, the frontend execution logic parsed API URL paths using a global `getApiBase()` utility. But our string concatenation was flawed. It formulated duplicate forward slashes (e.g., `//api//search`), which mapped completely broken paths organically. In local DEV environments this often resolves gracefully, but in Cloudflare Edge route definitions, double slashes instantly throw unexpected HTTP 400 exceptions mapping securely to 404 missing resource errors.

*How we faced it:* We stripped away the arbitrary conditional checks inside the frontend `getApiBase()` executing exact string manipulation correctly. By strictly verifying zero-length URL injection appending, we guaranteed exact single-slash resolutions securely, bridging network setups uniformly.
-> *Verify: Frontend network logs load exact `/api/search` endpoints explicitly dropping incorrect double forward-slash mapping errors.*