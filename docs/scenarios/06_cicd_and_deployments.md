# CI/CD & Deployment Configurations

**35. Obsolete Bash Invocation Hooks**
*The Pitfall:* Every time we pushed a commit, GitHub Actions failed catastrophically. The build matrices attempted to run legacy scripts like `check_frontend.sh` which we had aggressively deleted while purging obsolete files. Because CI blindly ran historically outdated YAML hooks, it halted the entire deployment pipeline claiming `No such file or directory`.
*How we faced it:* We performed a deep audit into `.github/workflows/ci-cd.yml` surgically purging every ghost invocation, ensuring that the pipeline only trusted pure Rust integration steps perfectly avoiding nonexistent shell files.
-> *Verify: Actions resolve completely parsing natively without encountering non-existent executable scripts bypassing obsolete hooks dynamically.*

**36. Render Platform Migration Ghosts**
*The Pitfall:* Alongside the ghost scripts, our GitHub actions stalled continually as it attempted to deploy the old architecture to Render. Since we nuked the Render instances entirely to avoid billing and moved to Cloudflare Pages natively, the Action threw timeout exceptions trying to hit nullified Webhook URLs, artificially doubling CI time before it invariably failed.
*How we faced it:* We explicitly ripped out all web hooks mapping backwards to Render, fully abandoning their architecture. We migrated deployment reliance solely onto the Wrangler configurations bypassing legacy stubs fundamentally.
-> *Verify: `grep Render .github/workflows/*.yml` yields cleanly zero output confirming complete architectural migration.*

**37. Cloudflare Account ID Ambiguity**
*The Pitfall:* Our Cloudflare credentials resolved, but the deployments missed the target constantly. We were executing updates that somehow dropped into an entirely different pages project (`amritapapers-1x5` instead of `exampapersamrita`). Because Wrangler intuitively guesses targets based on ambiguous naming structures, our production server remained completely outdated despite successful push logs.
*How we faced it:* We ran rigid REST validations explicitly parsing the UUIDs of `exampapersamrita` directly binding the exact 32-character hashes into `wrangler.toml`. By enforcing strict namespaces, Wrangler never guessed again.
-> *Verify: `cat wrangler.toml` reflects the specific accurate naming parameters rejecting duplicate random instances securely.*

**38. Wrangler Auth Cache Poisoning (Code 10000)**
*The Pitfall:* Running Wrangler locally threw a terrifying 10000 level error bounds, completely locking us out of our own deployment. The local `node_modules` cached a completely stale OAuth token that violently rejected manual commands, preventing us from deploying hotfixes explicitly.
*How we faced it:* We systematically purged the hidden local configuration maps, forcing an explicit OAuth reinjection over a clean boundary completely obliterating the cached poison.
-> *Verify: Push matrices acknowledge token identities executing cleanly resolving Edge configuration updates safely.*

**39. API Injection vs OAuth Refreshing**
*The Pitfall:* We fixed local deployments, but CI runners generated detached environments that fundamentally cannot click "Allow" on an OAuth browser popup. When the GitHub Action tried to deploy, Wrangler halted waiting for a human to type in a 2FA code, stalling out the entire Action workflow automatically.
*How we faced it:* We structurally bypassed the interactive loop natively, injecting explicit masked CI variables calling `CLOUDFLARE_API_TOKEN=$CF_TOKEN`. Wrangler detected the silent secret and bypassed graphical authentication instantly executing headless environments seamlessly.
-> *Verify: Automated CI logs correctly display CLI deployments reporting explicit `Success` returns verifying token ingestion securely.*

**40. Dirty Staging Environment Blocking**
*The Pitfall:* We successfully authenticated, but the CI push aborted stating the branch was 'dirty'. Because Wrangler inherently generated a hidden `.wrangler/` lockfile dynamically upon initialization inside the runner, `git` recognized the repository as untracked. Wrangler refused to push non-committed matrices to production automatically.
*How we faced it:* We injected an explicit override parameter `--commit-dirty=true` natively inside actions. By forcing Wrangler to ignore local staging drops generated during the runner isolation, it pushed the payload securely unequivocally.
-> *Verify: GitHub deployment hooks execute ignoring local state discrepancies retaining remote merge updates securely unconditionally.*

**41. Uploading Manifest Anomalies**
*The Pitfall:* During desperate debugging of the CI failures, we attempted raw Cloudflare API pushes bypassing Wrangler natively. We hit profound failures citing `{code: 8000096, message: manifest missing}` completely dropping the upload. The old manual binary configurations couldn't resolve JSON hashing arrays.
*How we faced it:* We explicitly abandoned trying to outsmart the API via `curl` wrappers. We enforced strict Wrangler deployment logic reliably substituting deprecated binary mappings natively.
-> *Verify: `wrangler deploy` triggers successful mapping vectors cleanly resolving correct file structure configurations autonomously avoiding manual validation paths systematically.*