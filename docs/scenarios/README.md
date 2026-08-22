# 62 Roadblocks: Postmortem & Resolution Index

This directory maintains the immutable, fully decoupled roadblock catalog mapping structural fixes required during initial infrastructure deployments and the search sorting overhaul.

All entries strictly adhere to Action/Reaction validation loops validating structural states.

### Modules

- **[01. Compute & Compilation Constraints](./01_compute_and_compilation.md)** *(Roadblocks 1-6)*
  Memory optimization, binary compilation, OS level blocking, and dependency mapping inside restricted environments.
- **[02. Deployment Orchestration & Execution](./02_deployment_and_daemons.md)** *(Roadblocks 7-13, 59-60)*
  Remote SSH execution, pathing overrides, detached polling hooks, persistent Systemd setups, Cloudflare token scoping, and hot binary swaps on a live daemon.
- **[03. Data Integrity & Syncing](./03_data_integrity_and_sync.md)** *(Roadblocks 14-18)*
  Database SQLite concurrency states, file transfer bypassing, Object storage Rclone deployments, and removing native shell injections.
- **[04. Network Origin Validation & Tunneling](./04_networking_and_tunnels.md)** *(Roadblocks 19-28, 61-62)*
  Firewall drops, Daemons, Cloudflare Named Tunnels, IPv6 routing drops, Cloudflared OS permissions, SELinux exec labels, and quick tunnel ephemerality.
- **[05. Frontend Proxying & Edge Handlers](./05_frontend_and_proxy.md)** *(Roadblocks 29-34)*
  Cloudflare Pages SPA catch-alls, replacing `_redirects` with Native Request Handlers, mitigating Cross-Origin `OPTIONS` preflights.
- **[06. CI/CD & Deployment Configurations](./06_cicd_and_deployments.md)** *(Roadblocks 35-41)*
  Orphaned Bash hooks, Render platform migrations, exact Wrangler definitions preventing 10000 Authentication drops.
- **[07. Repository Hygiene & Brutalist Frameworks](./07_repo_hygiene_and_strictness.md)** *(Roadblocks 42-50)*
  Brutalist AI compliance, obliterating conversational slop, strict ASCII documentation rendering, mitigating catastrophic tracking leaks.
- **[08. Search Ranking & Sort Determinism](./08_search_ranking_and_sorting.md)** *(Roadblocks 51-58)*
  Hardcoded relevance overriding user sorts, unstable pagination tiebreakers, stale production binaries, raw versus sanitized title ordering, digit-led fallback sinking, bin target naming, and end-to-end sort verification.
