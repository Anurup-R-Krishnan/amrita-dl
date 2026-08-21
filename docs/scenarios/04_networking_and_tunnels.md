# Network Origin Validation & Tunneling Configuration

**19. Firewalld IP Drops**
*The Pitfall (Deep Context):*
After getting the Axum backend permanently running securely via Systemd, we were ecstatic. Internal testing using `curl http://localhost:80` returned the 200 OK JSON payloads perfectly. We could see the SQLite search results flowing beautifully.
We then copied the VM's raw public IP block (`68.233.111.2`), plugged it into the browser to test live access, and it hung indefinitely. Not a 404. Not a connection refused. Just a completely silent TCP timeout where the packet entered some black hole and never came back. We waited a full 30 seconds watching the browser spinner. Nothing.
We immediately suspected the Oracle Cloud VCN. We spent an hour double-checking the subnet ingress rules, adding explicit Port 80 allow rules at the virtual network layer. We re-tested. Still hung. The TCP packets were dying somewhere between the Oracle VCN and the application. The profound and deeply frustrating realization struck us an hour later: Oracle Linux provisions its base images with `firewalld`, a kernel-facing OS-level firewall daemon completely independent of the cloud-level VCN. Even though we had opened Port 80 at the Oracle networking hardware layer, the Linux operating system itself was an entirely separate wall, silently grabbing every incoming packet and throwing them into `/dev/null`. Our IP was perfectly reachable at the network level, but the OS was actively murdering every connection before it even hit the Axum process.

*How we faced it:*
We could have brute-forced our way through `sudo firewall-cmd --add-port=80/tcp --permanent` and opened raw IP access. But that decision carries massive long-term costs: an open raw IP exposed to the internet is immediately picked up by Shodan scanners, hit by brute-force SSH attempts, and becomes an attack surface for every automated bot on the internet within 24 hours. 
Instead, we made the counter-intuitive decision to never open the VM to the internet at all. We kept `firewalld` maxed out and pivoted to Cloudflare Tunnel as the sole ingress path. The tunnel operates entirely over outbound TLS connections initiated by the VM itself, meaning the VM never needs to accept any incoming internet connections, ever. The `firewalld` became our friend: the ultimate outer wall behind which nobody could reach the server directly.
-> *Verify: From an external machine, `curl -m 5 http://<VM_PUBLIC_IP>` times out while `curl -m 5 https://exampapersamrita.pages.dev/api/status` returns 200, proving only the tunnel path is open.


**20. Restricted Edge Origin Rules**
*The Pitfall (Deep Context):*
With the raw IP firewall situation understood, we turned to what seemed like the simplest possible solution: using Cloudflare Pages' native `_redirects` file to proxy all `/api/*` frontend requests directly to the Oracle backend's raw IP. We wrote the redirect rule, deployed it, and tested it.
The result was deeply confusing. The browser showed a perfect `200 OK` response. But the body was not JSON. It was raw HTML. Specifically, it was our own `index.html` served right back to us. The proxy was completely silently failing and Cloudflare Pages was falling back to serving the SPA. No error. No 4xx. A silently fraudulent 200 with wrong data.
After debugging for hours, we discovered that Cloudflare Pages explicitly bans proxying to unencrypted plain `http://` origins. Pages enforces HTTPS-only origin fetching on their Edge, meaning when it encountered our `_redirects` rule pointing at `http://68.233.111.2`, it evaluated the origin as a prohibited unencrypted target and silently dropped the proxy instruction entirely. It then fell back to the SPA catchall and served `index.html`. We were trapped: we needed HTTPS on the origin, but a raw IP address with no domain cannot get a valid SSL certificate.

*How we faced it:*
The only viable solution to this entire class of problems was a Cloudflare Tunnel. The tunnel gives our backend a legitimate `*.cfargotunnel.com` subdomain with a valid Cloudflare-signed TLS certificate. This means our edge proxy now has an HTTPS-compliant target to route to, fully satisfying Cloudflare's strict origin requirements without us needing to manage certificates at all.
-> *Verify: The Pages Function for `/api/*` fetches `https://<tunnel-subdomain>.cfargotunnel.com` and `curl -I` on the deployed site returns `content-type: application/json`, not `text/html`.


**21. Cloudflare RPM Distribution 404s**
*The Pitfall (Deep Context):*
To set up the Cloudflare Tunnel, we needed `cloudflared` installed on the Oracle VM. We went to the official documentation, followed the RedHat/Oracle Linux install path, and ran `sudo dnf install cloudflared`. The package manager churned through dependency resolution for about 20 seconds, then threw absolute garbage:
```
Error: Failed to download metadata for repo 'cloudflared':
Cannot prepare internal mirrorlist: No URLs in mirrorlist
```
404s everywhere. The package repository URL that Cloudflare's documentation pointed to was completely dead. Cloudflare had silently migrated away from directly hosting RPM packages in a traditional dnf-compatible repo format but hadn't cleanly updated the Oracle Linux docs path. We were pointed at a graveyard. The `dnf` package manager kept retrying, hitting the 404, and giving up. There was no fallback.

*How we faced it:*
We killed the package manager approach entirely and went raw. We inspected Cloudflare's GitHub releases page directly, found the specific AMD64 Linux binary release, and pulled it manually:
```bash
wget https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64
chmod +x cloudflared-linux-amd64
sudo mv cloudflared-linux-amd64 /usr/local/bin/cloudflared
```
No package manager. No repo. No dependency resolution. Just a raw binary drop into the system path. This became the canonical deployment pattern: if a package manager fails, pull the binary directly and skip the abstraction layer entirely.
-> *Verify: `cloudflared --version` prints a version string and `which cloudflared` resolves to `/usr/local/bin/cloudflared`.


**22. Privileged Escalation of Daemons**
*The Pitfall (Deep Context):*
With the binary downloaded, we ran `cloudflared tunnel run` from our home directory as the `opc` user. The process launched briefly, then crashed instantly. Checking the stderr output revealed the killer:
```
failed to sufficiently increase receive buffer size (was: 208 KiB, wanted: 7168 KiB, got: 416 KiB)
permission denied opening kernel socket
```
The `cloudflared` daemon, in order to establish its TLS tunnel, needs to open raw kernel network sockets at a privilege level above standard user accounts. Oracle Linux's SELinux configuration in combination with kernel socket hardening explicitly denies this to anything running under `/home/opc`. The daemon was physically barred from touching the networking layer by multiple layers of OS security.

*How we faced it:*
We had already learned this lesson the hard way with the Systemd 203/EXEC error on the Rust binary: user-space home directories are off-limits for system daemons. We relocated the binary:
```bash
sudo mv cloudflared /usr/local/bin/cloudflared
sudo chmod +x /usr/local/bin/cloudflared
```
By moving it into `/usr/local/bin`, we crossed the boundary from "user program" to "system program," giving the OS permission to treat it as a privileged service binary capable of opening the network sockets it needed.
-> *Verify: `sudo /usr/local/bin/cloudflared tunnel run --token <TOKEN>` stays running with no permission-denied lines in output.


**23. Temporary Tunnel Output Parsing**
*The Pitfall (Deep Context):*
To validate that our tunnel concept actually worked end-to-end before committing to a complex named tunnel setup, we ran a Quick Tunnel: `cloudflared tunnel --url http://localhost:80`. This spun up instantly and Cloudflare assigned it an ephemeral URL like `https://random-words-abc123.trycloudflare.com`. We plugged it into our frontend and it worked. The search results returned. The SQLite database was being queried live.
The problem was immediate: our deployment scripts needed to automate this. We needed to programmatically capture the generated URL, inject it into the frontend environment variables, redeploy Pages with the new origin, and restart the daemon every time the server rebooted. But `cloudflared` dumps the generated URL buried in the middle of a wall of colored log output, somewhere between TLS handshake messages and connection pool metrics. It doesn't have a clean `--print-url` flag. We had to parse it out of raw log noise.

*How we faced it:*
We studied the exact log format and built a precise grep to extract just the URL:
```bash
cloudflared tunnel --url http://localhost:80 2>&1 | grep -o 'https://[a-z0-9-]*\.trycloudflare\.com'
```
By piping stderr into stdout (`2>&1`) and running a targeted regex against the exact URL format Cloudflare uses, we reliably extracted just the URL. This was piped downstream into the deployment scripts to inject the dynamic origin into the frontend configuration. Fragile by nature, but functional as a proof-of-concept before we moved to a stable named tunnel.
-> *Verify: Running the quick tunnel and piping through the grep prints exactly one URL matching `https://[a-z0-9-]*.trycloudflare.com`.


**24. Cloudflare Worker 1003 Banning**
*The Pitfall (Deep Context):*
Before we fully committed to the tunnel architecture, we tried one more alternative: writing a custom Cloudflare Worker to act as the proxy. The Worker would sit on the Edge, receive `/api/*` requests from the frontend, and `fetch()` the Oracle VM directly. We deployed it. We tested it. The Worker returned a cryptic:
```
Error 1003: Direct IP access not allowed
```
Cloudflare's network actively and explicitly bans Workers from making outbound fetch requests to raw IP addresses. Error 1003 is a hard security block enforced at the network level to prevent Cloudflare's own infrastructure from being used to proxy traffic to bare IP addresses that aren't behind Cloudflare's own CDN. We were trying to use Cloudflare's resources to route around Cloudflare's system, and they caught us instantly.

*How we faced it:*
Error 1003 killed the Worker approach permanently. There was no way around it without an actual domain or tunnel. We committed fully to the tunnel architecture: the VM runs `cloudflared`, which gives the backend a legitimate `*.cfargotunnel.com` domain that Cloudflare's own network trusts and routes to without triggering the 1003 security block.
-> *Verify: The Worker/Function route fetches the tunnel hostname and browser devtools show `/api/search` returning HTTP 200 JSON instead of error 1003.


**25. Named Tunnel Creation Collisions**
*The Pitfall (Deep Context):*
The Quick Tunnel approach was fundamentally broken for production: every time `cloudflared` restarted, it generated a completely different random subdomain. This meant the frontend was always pointing at a dead URL after any server restart. We needed a stable, permanent tunnel with a fixed endpoint.
We ran `cloudflared tunnel create amrita-api` to establish a permanent named tunnel. The command returned:
```
Error: failed to create tunnel: Tunnel with name 'amrita-api' already exists.
```
Weeks earlier, during initial experimentation, one of us had casually created a test tunnel with that name through the Cloudflare dashboard and completely forgotten about it. The name was permanently occupied in the Cloudflare account namespace, blocking our CI script from creating a fresh one. And we couldn't find the old tunnel in the dashboard because it had been abandoned in a half-configured state.

*How we faced it:*
We hit the Cloudflare API directly to list all existing tunnels in the account:
```bash
curl -s -X GET "https://api.cloudflare.com/client/v4/accounts/$ACCOUNT_ID/cfd_tunnel" \
  -H "Authorization: Bearer $CF_TOKEN" | jq '.result[] | {id, name, status}'
```
This surfaced the abandoned tunnel with its UUID `a0a5a18a-fc04...`. Rather than fighting to create a new tunnel, we adopted this existing one, wired it up with fresh credentials, and it worked perfectly. The ghost tunnel became our production tunnel.
-> *Verify: The cfd_tunnel list call returns each existing tunnel's id and name, and no two tunnels share the name `amrita-api`.


**26. Named Tunnel Unauthenticated Ingress**
*The Pitfall (Deep Context):*
With the tunnel UUID in hand, we tried running it: `cloudflared tunnel run a0a5a18a-fc04...`. The daemon launched, tried to authenticate, and immediately crashed:
```
failed to load tunnel credentials file: open /root/.cloudflared/a0a5a18a-fc04....json: no such file or directory
```
`cloudflared` expects a local `credentials.json` file to exist on disk at a specific path. This file contains the tunnel's private key material and must be placed there during the `cloudflared tunnel create` step. But since we adopted an existing tunnel rather than creating a fresh one, we never went through that step and the credentials file never existed. Creating it manually would require downloading private key material through the Cloudflare dashboard, securely transferring it to the VM, and placing it at exactly the right path. In a headless automated CI environment, this was an unacceptable manual dependency.

*How we faced it:*
We discovered the `--token` flag. Cloudflare's API can generate a self-contained, single-token representation of the tunnel credentials:
```bash
curl -s -X GET "https://api.cloudflare.com/client/v4/accounts/$ACCOUNT_ID/cfd_tunnel/$TUNNEL_ID/token" \
  -H "Authorization: Bearer $CF_TOKEN" | jq -r '.result'
```
This returns a massive 168+ character base64-encoded token that contains all the authentication material. We could pass this directly to cloudflared at runtime without ever needing a credentials file on disk:
```bash
cloudflared tunnel run --token $TUNNEL_TOKEN
```
The credentials file requirement was completely bypassed. Automation restored.
-> *Verify: `cloudflared tunnel run --token $TUNNEL_TOKEN` connects with zero references to a credentials `.json` path in logs.


**27. IPv6 Disallowed Routing Errors**
*The Pitfall (Deep Context):*
The tunnel authenticated. The daemon started. We tested the `*.cfargotunnel.com` endpoint from the browser and got an HTML error page from Cloudflare itself:
```
Error: Failed to parse upstream HTTP response (from cloudflared):
Unsupported URL scheme. The server URL must begin with http:// or https://
Your cloudflare Tunnel is up but the origin appears to be having DNS issues.
```
After an hour of debugging, we found the root cause buried in the default tunnel ingress config. Without an explicit `ingress` block in the `cloudflared` config, the daemon attempts to resolve the upstream service using the tunnel hostname as a DNS name. This triggers an IPv6 DNS lookup on a system where IPv6 is either restricted or misconfigured, which Oracle Linux 8 frequently does on E2.1.Micro instances.

*How we faced it:*
We wrote an explicit ingress rule that bypassed DNS resolution entirely and pointed directly at the local loopback:
```yaml
ingress:
  - service: http://localhost:80
```
By hardcoding the literal loopback IP as the upstream target, we eliminated the DNS resolution step completely. The tunnel now routes directly to the running Axum process on port 80 without needing to resolve any hostname.
-> *Verify: With `ingress: - service: http://localhost:80` set, `curl https://<tunnel-host>/api/status` returns JSON with no 'Unsupported URL scheme' or DNS errors.


**28. Systemd Cloudflared Persistence**
*The Pitfall (Deep Context):*
Everything was functioning. The tunnel was live, the frontend was reaching the SQLite backend through Cloudflare's edge, and search results were appearing. We celebrated. Then two days later, we noticed the site was completely down. Again.
We We connected over SSH and into the VM and checked: the `amrita-server` Rust binary was running fine. But `cloudflared` was dead. It had silently exited at some point. We could not tell exactly when, and with it, the entire HTTPS tunnel had collapsed. Because the tunnel was started manually in our SSH session over a week ago, closing that terminal had eventually sent signals that caused the process to exit. Without automation, every server restart, every maintenance window, every unexpected process crash would silently kill the public-facing endpoint completely.

*How we faced it:*
We wrote a proper Systemd service unit: `/etc/systemd/system/cloudflared-tunnel.service`. This registered `cloudflared` as a first-class OS daemon alongside the Rust backend. The service is configured with `Restart=always` and `RestartSec=5` meaning even if the process crashes or the tunnel connection drops, Systemd will revive it within 5 seconds automatically. On every boot, Systemd brings it back up before any user session is established. The tunnel is now as reliable as the OS itself.
```bash
sudo systemctl enable cloudflared-tunnel
sudo systemctl start cloudflared-tunnel
```
-> *Verify: After killing the process with `sudo pkill cloudflared`, `systemctl status cloudflared-tunnel` shows it back `active (running)` within 10 seconds.