# Automated CI/CD (GitHub Actions)

Continuous Deployment is managed by workflows inside `.github/workflows/`.

## Pipelines

**1. Rust Test Matrix (`ci-cd.yml`)**
Action: Compiles models, enforces `cargo clippy -D warnings`, runs `cargo test`.
*Verify: GitHub Actions UI registers a green checkmark against the active push.*

**2. Cloudflare Pages Deploy (`deploy.yml`)**
Action: Triggers JWT-driven deployment of static assets (`web/`) mapping cross-origin policies.
*Verify: `exampapersamrita.pages.dev` actively reflects DOM changes post-merge.*

## Troubleshooting Auth Errors

Wrangler `Code 10000 Authentication Error` implies expired OAuth caches.
Fix: Rely on JWT direct injection mapped within actions.

**Action Secrets Mapped (`Settings > Secrets and variables > Actions`):**
- `CLOUDFLARE_ACCOUNT_ID` -> Maps to absolute CF Dashboard UUID (`96f9c...`)
- `CLOUDFLARE_API_TOKEN` -> Maps to explicit `Pages:Write` API key generated.
*Verify: Deploys clear authorization checks successfully running `CLOUDFLARE_API_TOKEN=$CF_TOKEN wrangler deploy --commit-dirty=true`.*