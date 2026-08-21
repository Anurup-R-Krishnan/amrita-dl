# Automated CI/CD (GitHub Actions)

We have configured a fully automated pipelines using GitHub Actions located within `.github/workflows/`. Whenever code is pushed to the `main` branch, it automatically validates the Rust backends and leverages direct JWT injection to deploy static elements to Cloudflare Pages.

## Architecture

1. **`ci-cd.yml`**:
   - Compiles Rust `amrita-server` across all target matrices natively.
   - Executes `cargo clippy --bin server -D warnings`.
   - Executes `cargo test`.
   - (Deprecated: Local bash execution validations like `check_frontend.sh` were stripped).

2. **`deploy.yml`**:
   - Automatically scopes the `web/` active layout.
   - Pushes static distributions to the `exampapersamrita` Pages routing.

## Deployment Triggering Limitations

During deployment debugging, Wrangler occasionally triggers `Code 10000 Authentication Error` despite valid OAuth routines. When this fails:
1. OAuth scopes tied to `wrangler pages deploy` may become stale.
2. The pipeline execution circumvents this securely by explicitly assigning JWT via environment pipelines: `CLOUDFLARE_API_TOKEN=$CF_TOKEN wrangler deploy`.
3. To bypass dirty-git locks halting deployments mid-air, the action invokes `--commit-dirty=true` ensuring synchronous repository representations.

### Managing Secrets
Operations demand exact matching of tokens inside the GitHub repository Settings:
- `CLOUDFLARE_ACCOUNT_ID`
- `CLOUDFLARE_API_TOKEN`
