# Frontend Proxying & Edge Handlers

**29. API Parsing HTML Errors**
*The Pitfall:* Our elegant single-page application aggressively threw violent JSON parsing panics whenever you typed something into the search bar. We blindly trusted our proxies. Executing a raw `curl` into the React/Alpine UI exposed that Cloudflare returned raw `index.html` DOM blobs rather than bridging traffic into our sub-millisecond SQLite arrays implicitly.
*How we faced it:* We isolated the frontend logic testing endpoints natively ensuring proxy connections verified application-level isolations confirming raw JSON structures strictly properly.
-> *Verify: `curl -I https://exampapersamrita.pages.dev/api/status` returns strictly `application/json` content-types validating true backend isolation.*

**30. Pages Catch-All Override**
*The Pitfall:* We realized exactly *why* the HTML blobs happened. Cloudflare Pages uses native SPA override patterns that completely ignored our standard static `_redirects` mechanisms inherently overwriting paths dynamically blocking API tunnels cleanly.
*How we faced it:* We aggressively abandoned `_redirects` substituting active Cloudflare edge programmatic interceptors matching path execution boundaries completely safely bypassing SPA defaults dynamically.
-> *Verify: `ls _redirects` generates a 'No such file' exception proving static overrides exist completely nullified.*

**31. Developing Full Cloudflare Functions**
*The Pitfall:* Programming Edge functions explicitly bridging our TLS tunnels spawned nightmare architecture arrays attempting to merge fetch queries bridging trycloudflare setups flawlessly without breaking CORS rules mechanically.
*How we faced it:* We engineered surgical `[[path]].js` acting inherently executing explicit fetch destinations dynamically traversing TLS domains automatically completely overriding local states flawlessly.
-> *Verify: `cat functions/api/[[path]].js` explicitly maps explicit proxy definitions natively bypassing local states perfectly.*

**32. Functions Directory Obfuscation**
*The Pitfall:* We built our functions and ran deployment mechanisms only to find them completely ignored by the build runner. We had accidentally grouped our logic inside `web/functions/`. Wrangler strictly respects root definitions natively dropping buried architecture completely gracefully.
*How we faced it:* We structurally deployed `mv web/functions functions/`, deploying exactly at the node boundary safely bypassing nested obfuscation completely bridging Cloudflare requirements natively cleanly.
-> *Verify: `ls -ld functions` maps successfully inside the root tree avoiding nested SPA obfuscations completely.*

**33. Pre-flight CORS Restraints (OPTIONS)**
*The Pitfall:* Chrome and Firefox aggressively slaughtered standard fetch API integrations throwing invalid CORS configuration drops breaking execution environments unconditionally because intercepted responses explicitly dropped OPTION configurations natively heavily blocking execution safely.
*How we faced it:* We structurally overhauled proxy intercepts injecting arbitrary HTTP 204 intercepts matching `Access-Control-Allow-Origin: *` validating pre-flight mechanisms securely flawlessly overriding tracking bounds dynamically accurately.
-> *Verify: `curl -X OPTIONS -I https://exampapersamrita.pages.dev/api/` replies exactly 204 explicitly confirming CORS bypass configurations safely.*

**34. Fallback API Parsing Toggles**
*The Pitfall:* Frontend execution logic parsed empty string boundaries dynamically formulating duplicate forward slashes (`//api//status`) mapping completely broken paths securely throwing unexpected exceptions safely mechanically cleanly.
*How we faced it:* We stripped API query bases inside `getApiBase()` executing exact string manipulation correctly bypassing duplicate formatting strictly safely bridging network setups uniformly organically.
-> *Verify: Frontend network logs load exact `/api/search` endpoints explicitly dropping incorrect double forward-slash mapping errors.*