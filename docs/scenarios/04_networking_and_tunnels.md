# Network Origin Validation & Tunneling Configuration

**19. Firewalld IP Drops**
*The Pitfall:* The Axum backend was running perfectly locally. Internal `curl` on the VM resolved 200 OK. But the public IP address just hung indefinitely over standard Port 80. Oracle's default networking doesn't just block traffic at their cloud level—the VM itself runs `firewalld`, silently dropping every single public ingress mapping maliciously.
*How we faced it:* Instead of painstakingly opening Oracle subnets and manually debugging `firewall-cmd --list-ports`, we dropped standard ingress routing entirely. We isolated the native routing exclusively through Cloudflare bridging tunnels natively skipping public access totally.
-> *Verify: Oracle Subnets retain maximum isolation ignoring raw traffic while the application routes effectively isolated.*

**20. Restricted Edge Origin Rules**
*The Pitfall:* Our initial setup was elegant: use Cloudflare Pages `_redirects` to proxy `/api/*` requests to the Oracle backend's raw IP `http://68.233.111.2`. It failed silently. Cloudflare Pages aggressively dropped proxying structures attempting to bridge onto unencrypted nodes natively demanding SSL endpoints implicitly.
*How we faced it:* Since we couldn't slap an SSL cert onto a raw IP without complex Certbot routing, we pivoted abruptly deploying native Cloudflare Tunnels mapping SSL securely avoiding explicit proxies comprehensively.
-> *Verify: HTTPS proxies dynamically resolve targeting the newly defined encrypted node structure mapping correctly.*

**21. Cloudflare RPM Distribution 404s**
*The Pitfall:* To run the Tunnel, we tried installing `cloudflared` using standard `dnf` procedures. The package manager violently returned 404 mapping exceptions. The legacy tutorial repositories tracking Cloudflare explicitly abandoned RedHat tracking definitions completely stalling our setup.
*How we faced it:* We abandoned package distributors totally operating strict binary curl definitions directly pulling AMD64 executable formats over Wget injecting safely natively bypassing Linux RPM dependencies safely.
-> *Verify: `cloudflared --version` executes locally responding strictly over valid current patch definitions tracking releases accurately.*

**22. Privileged Escalation of Daemons**
*The Pitfall:* Cloudflared attempted establishing listening hooks but completely crashed citing extreme OS protection blockades. The user accounts were barred from engaging subsystem connections.
*How we faced it:* We violently escalated permission structures directly pushing code bindings onto `/usr/local/bin` mapping explicit `chmod +x` root-level structures safely launching endpoints precisely.
-> *Verify: `cloudflared tunnel run` resolves without halting explicitly executing subsystem network bindings properly.*

**23. Temporary Tunnel Output Parsing**
*The Pitfall:* The quick tunnel generated chaotic `.trycloudflare.com` URLs asynchronously inside logs making scripting deployments impossible because the host changed eternally.
*How we faced it:* We generated extreme tracking parameters using `grep -o 'https://[a-z0-9-]*\.trycloudflare\.com'` parsing the ephemeral architectures autonomously filtering outputs accurately over automation loops tightly.
-> *Verify: String returns exactly the UUID format masking output structures accurately bridging external execution commands uniformly.*

**24. Cloudflare Worker 1003 Banning**
*The Pitfall:* A desperate attempt at writing a custom Cloudflare Worker to `fetch()` the unencrypted IP node explicitly triggered Error 1003. Cloudflare heavily restricts workers from bridging non-TLS custom architectures dynamically.
*How we faced it:* We natively enforced TLS mapping binding explicitly over persistent HTTPS routes forcing active Cloudflare named tunnels completely terminating Error 1003 cleanly.
-> *Verify: Edge payloads process 200 HTTP calls targeting `*.cfargotunnel.com` domains accurately escaping Edge drops efficiently.*

**25. Named Tunnel Creation Collisions**
*The Pitfall:* We tried to standardize the TryCloudflare ephemerality by creating a fixed tunnel named `amrita-api`. The CLI halted stating "already exists". We had historically abandoned a tunnel in the UI.
*How we faced it:* We bypassed arbitrary naming mechanisms actively polling REST targets forcing current UUID extractions exactly locking onto valid historical entities explicitly gracefully.
-> *Verify: JSON parameter returns precise `a0a5a18a-fc04...` bypassing naming duplications accurately locking origin architectures safely.*

**26. Named Tunnel Unauthenticated Ingress**
*The Pitfall:* Running a named tunnel demanded a missing `credentials.json` file. We couldn't fetch it headless, halting ingress capabilities immediately.
*How we faced it:* We bypassed physical credential tracking directly siphoning a massive 168-character token out of the Cloudflare UI passing it aggressively natively to the `--token` boot argument tightly overriding file systems totally.
-> *Verify: `cloudflared tunnel run --token` bootstraps successfully parsing credentials directly avoiding JSON files.*

**27. IPv6 Disallowed Routing Errors**
*The Pitfall:* Despite everything connecting, the frontend pulled broken HTML proxy pages explicitly citing internal IPv6 mapping definitions blocking Cloudflared local resolution dynamically.
*How we faced it:* We modified the daemon `ingress` routing rules commanding proxies into explicitly named `http://localhost:80` avoiding localhost loopback array drops strictly correctly.
-> *Verify: Proxied endpoints resolve raw JSON directly bypassing DNS loop errors correctly targeting active Daemon mapping systems uniformly.*

**28. Systemd Cloudflared Persistence**
*The Pitfall:* The proxy functioned beautifully until we restarted the VM. Cloudflared scripts died asynchronously crashing the site immediately exactly when server maintenance operated organically.
*How we faced it:* We architected a brutalist `cloudflared-tunnel.service` configuration ensuring structural monitoring loops tracked execution environments constantly keeping node networks resiliently attached dynamically.
-> *Verify: `systemctl status cloudflared-tunnel` enforces Active(Running) reporting mapping eternal origin integrations precisely avoiding node crash mapping drops natively.*