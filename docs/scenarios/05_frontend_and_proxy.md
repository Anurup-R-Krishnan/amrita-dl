# Frontend Proxying & Edge Handlers

**29. API Parsing HTML Errors**
- *Roadblock:* The SPA UI violently threw JSON parse errors upon executing valid queries.
- *Fix:* Executed direct `curl` against the endpoint, revealing Pages returned raw `index.html` source codes rather than bridging to the backend DB queries.
-> *Verify: `curl -I https://exampapersamrita.pages.dev/api/status` returns strictly `application/json` content-types validating true backend isolation.*

**30. Pages Catch-All Override**
- *Roadblock:* Cloudflare Pages processes native routing logic intercepting active branches as SPA fallbacks completely superseding standard `_redirects` proxy configurations.
- *Fix:* Scrapped static file redirectors inherently substituting explicit programmatic interceptors acting before the DOM resolution.
-> *Verify: `ls _redirects` generates a 'No such file' exception proving static overrides exist completely nullified.*

**31. Developing Full Cloudflare Functions**
- *Roadblock:* Bypassing `_redirects` natively required generating advanced proxy mechanisms bridging the TLS tunneling requirements completely over Edge resolution variables.
- *Fix:* Engineered `[[path]].js` acting natively over `functions/api/` injecting dynamic `fetch` targets bridging Cross-Origin resolutions automatically routing traffic to the exact `trycloudflare.com` endpoints.
-> *Verify: `cat functions/api/[[path]].js` explicitly maps explicit proxy definitions natively bypassing local states perfectly.*

**32. Functions Directory Obfuscation**
- *Roadblock:* Native Wrangler builds entirely ignored proxy definitions accidentally located against frontend configurations as `web/functions/`.
- *Fix:* Re-positioned structure issuing `mv web/functions functions/`, deploying exactly at root boundaries matching strict Cloudflare structural deployment requirements mechanically.
-> *Verify: `ls -ld functions` maps successfully inside the root tree avoiding nested SPA obfuscations completely.*

**33. Pre-flight CORS Restraints (OPTIONS)**
- *Roadblock:* Intercepted functions threw invalid CORS constraint exceptions omitting `OPTIONS` request validations natively killing Chrome/Firefox query mechanisms.
- *Fix:* Refactored proxy integrations generating strict `HTTP 204 No Content` returns against pre-flight headers natively resolving `Access-Control-Allow-Origin: *` properly.
-> *Verify: `curl -X OPTIONS -I https://exampapersamrita.pages.dev/api/` replies exactly 204 explicitly confirming CORS bypass configurations safely.*

**34. Fallback API Parsing Toggles**
- *Roadblock:* Frontend base hooks parsed string fallbacks improperly duplicating URL injection pathways natively breaking string formulations.
- *Fix:* Stripped variables strictly checking zero-length injection appending explicit query bases cleanly inside `getApiBase()` executing string manipulations accurately over local setups.
-> *Verify: Frontend network logs load exact `/api/search` endpoints explicitly dropping incorrect double forward-slash mapping errors.*
