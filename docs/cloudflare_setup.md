# ☁️ Cloudflare Deployment & Architecture Guide

Cloudflare acts as our Frontend Host (Pages), DNS Manager, and Edge Cache. This guide details how the frontend connects to the OCI backend and how to deploy it from scratch.

## 1. Cloudflare Pages Setup (Frontend Hosting)

Instead of using a separate server to host HTML/JS, we use **Cloudflare Pages** which serves our frontend (`web/` directory) directly from edge nodes worldwide, costing $0.

### Creating the Project
We create the project via the Wrangler CLI. The project name determines the default `.pages.dev` URL.
```bash
# Initialize a new Pages project named "exampapersamrita"
CLOUDFLARE_API_TOKEN="..." npx wrangler pages project create exampapersamrita --production-branch=main
```

### Deploying the Code
To push the current state of the `web/` folder up to Cloudflare:
```bash
CLOUDFLARE_API_TOKEN="..." npx wrangler pages deploy web --project-name exampapersamrita --commit-dirty=true
```
*Note: We pass `--commit-dirty=true` to deploy the current local state regardless of uncommitted git changes.*

---

## 2. The API Proxy (`_redirects`)

Browsers block cross-origin requests (CORS) by default. To make our frontend at `exampapersamrita.pages.dev` talk to our OCI VM at `api.amritapapers.dev` smoothly, we use Cloudflare Pages' native reverse proxy feature.

Inside `web/_redirects`, we have:
```text
/api/* https://api.amritapapers.dev/api/:splat 200
```
**How this works:**
1. User searches on the frontend.
2. The browser fetches `/api/search?q=math`.
3. Cloudflare intercepts this and automatically forwards it to the OCI backend (`https://api.amritapapers.dev/api/search?q=math`).
4. The user's browser thinks it never left the frontend domain, bypassing all CORS issues completely.

---

## 3. DNS Configuration (Connecting Frontend to Backend)

Once the OCI VM is running, we must route `api.amritapapers.dev` to the Oracle Cloud Server.

1. Go to the [Cloudflare Dashboard](https://dash.cloudflare.com) -> Select your domain (`amritapapers.dev`).
2. Navigate to **DNS > Records**.
3. Click **Add Record**:
   - **Type:** `A`
   - **Name:** `api`
   - **IPv4 address:** `<Paste your OCI VM Public IP>`
   - **Proxy status:** ☁️ (Proxied - Orange Cloud)
4. Click **Save**.

### Why Proxy the API?
By Orange-Clouding the API endpoint, Cloudflare dynamically caches the API responses (if headers allow) and provides free DDoS protection to the tiny Oracle VM, ensuring it never runs out of memory under heavy load.

---

## 4. Custom Domains (Optional)

By default, the site is live at `https://exampapersamrita.pages.dev`. To attach your premium domain (e.g., `www.amritapapers.dev`):

1. Go to **Pages** in the Cloudflare Dashboard.
2. Select **exampapersamrita**.
3. Go to the **Custom Domains** tab.
4. Click **Set up a custom domain**.
5. Enter your domain (e.g., `www.amritapapers.dev`) and click **Activate**. Cloudflare will automatically append it to your DNS records.

---

## Summary of the Flow
User ➡️ Cloudflare Pages (Frontend UI) ➡️ `fetch(/api/...)` ➡️ Cloudflare Proxy ➡️ OCI VM (Rust API) ➡️ Database Query ➡️ `302 Redirect` ➡️ OCI Object Storage ➡️ User downloads PDF directly!
