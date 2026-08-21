# CI/CD & Deployment Configurations

**35. Obsolete Bash Invocation Hooks**

*The Pitfall (Deep Context):*
As we moved our infrastructure to pure Rust and got Cloudflare Pages running, we activated GitHub Actions for continuous deployment. Every push failed the pipeline catastrophically in under 10 seconds.

We opened the Action logs expecting a Rust compilation error. Instead, the build matrix was attempting to execute legacy testing scripts like `check_frontend.sh` and `lint_python.sh`. During earlier optimization phases we had purged those obsolete files from the repository completely. But the CI YAML blindly trusted its historical hook definitions and halted the entire pipeline claiming `No such file or directory`. Ghosts of our old architecture were blocking pushes of the new one.

*How we faced it:*
We performed a deep programmatic audit of `.github/workflows/ci-cd.yml`, surgically dropping into each matrix to purge every ghost invocation. We ensured the pipeline only trusted steps backed by files that actually exist, forcing CI to respect the true modern state of the project:
```bash
grep -n '\.sh' .github/workflows/*.yml   # find every shell hook reference
```
Every hit was either deleted or replaced with its Rust equivalent (`cargo test`, `cargo clippy`).

-> *Verify: `grep -c 'check_frontend\|lint_python' .github/workflows/*.yml` returns zero, and the next push passes the setup stage without `No such file or directory`.*


**36. Render Platform Migration Ghosts**

*The Pitfall (Deep Context):*
As soon as we fixed the bash hooks, we triggered the Action again. It compiled Rust successfully, passed tests, then stalled on the deployment phase for six minutes before crashing with `Connection Timeout`.

Digging into the deploy step revealed the CI was blindly POSTing to the old Render architecture. We had nuked all Render instances weeks earlier to avoid billing costs and migrated the backend purely onto the Oracle VM and Cloudflare Pages. But the GitHub Action was still hitting a dead, nullified webhook URL. The TCP handshake had nothing on the other end, so the runner waited out its full timeout window before failing. Six wasted minutes on every push, blocking deployment to the new Cloudflare Edge.

*How we faced it:*
We ripped out every webhook and deploy step referencing Render, abandoning that architectural footprint in the YAML entirely. Deployment responsibility moved solely onto the Wrangler publish step targeting Cloudflare Pages, with no legacy stubs remaining anywhere in the pipeline definition.

-> *Verify: `grep -i render .github/workflows/*.yml` yields zero output, and the deploy stage no longer stalls before failing.*


**37. Cloudflare Account ID Ambiguity**

*The Pitfall (Deep Context):*
With CI finally executing the Wrangler deploy step, it returned a green checkmark: "Deployed successfully." We refreshed the live website and absolutely nothing had updated.

We ran the CI three more times. Green every time. No updates on the live server. Finally we pulled the raw Wrangler deploy JSON output and saw the truth: the Cloudflare credentials resolved fine, but Wrangler was pushing into an entirely different, abandoned Pages project named `amritapapers-1x5` instead of `exampapersamrita`. Because Wrangler guesses targets based on ambiguous naming structures and cached environment state, our actual production site stayed stale despite successful push logs. The worst failure mode: a pipeline that lies to you with a green checkmark.

*How we faced it:*
We refused to let Wrangler guess anymore. We ran explicit REST validations against the Cloudflare API to extract the exact project name and account UUID, then hardcoded them into the configuration:
```bash
curl -s "https://api.cloudflare.com/client/v4/accounts/$CF_ACCOUNT_ID/pages/projects" \
  -H "Authorization: Bearer $CF_TOKEN" | jq '.result[].name'
```
The true project name went into `wrangler.toml` under `name`, and `account_id` was pinned to the exact 32-character hash. By enforcing strict namespaces at the manifest level, Wrangler was permanently locked to the correct production site.

-> *Verify: `cat wrangler.toml` shows `name = "exampapersamrita"` and the correct `account_id`, and the next deploy log prints that exact project name in its output.*


**38. Wrangler Auth Cache Poisoning (Code 10000)**

*The Pitfall (Deep Context):*
To debug the weird CI issues, we tried running Wrangler locally on macOS via `npx wrangler deploy`. The CLI threw a terrifying `Code 10000` authentication error, completely locking us out of our own Cloudflare deployment.

We attempted `wrangler login`, which opens the browser and completes the OAuth confirmation loop visibly. But the CLI kept rejecting subsequent commands anyway. The local `.wrangler` cache directory was clinging to a completely stale, poisoned OAuth token from an earlier account experiment, and it could not be naturally overwritten by a fresh login. New tokens went in; the old poisoned one kept being read first.

*How we faced it:*
We systematically purged the hidden local configuration state:
```bash
rm -rf node_modules/.cache/wrangler .wrangler
```
Then we forced a fresh OAuth injection over a clean slate with `wrangler login` again. With the poisoned cache physically deleted, the newly issued token was actually read, and deploys worked immediately. The lesson: OAuth state lives on disk, and "log in again" does nothing if the stale state file survives.

-> *Verify: `npx wrangler whoami` prints the correct account email and account ID with no `Code 10000` errors.*


**39. API Injection vs OAuth Refreshing**

*The Pitfall (Deep Context):*
Fixing the local deployment meant our CI runners now crashed instead. GitHub Actions environments are detached, headless Docker containers. They fundamentally cannot click "Allow" on a visual Cloudflare OAuth popup. When the workflow evaluated `wrangler deploy`, the CLI halted indefinitely waiting for a human to click a web link, stalling the entire Action until it hit the hard execution limit. A deploy that requires a human click inside a headless runner is not a deploy; it is a deadlock.

*How we faced it:*
We structurally bypassed the interactive loop. Instead of OAuth sessions, we generated a scoped API token in the Cloudflare dashboard (Pages: Edit permission), configured it as a GitHub Secret, and injected it into the environment:
```yaml
- name: Deploy
  run: npx wrangler pages deploy web
  env:
    CLOUDFLARE_API_TOKEN: ${{ secrets.CF_TOKEN }}
    CLOUDFLARE_ACCOUNT_ID: ${{ secrets.CF_ACCOUNT_ID }}
```
Wrangler detects the environment variable and executes fully headlessly without ever spawning a browser thread. This is also more secure than OAuth: the token is scoped, rotatable, and revocable.

-> *Verify: The automated CI log displays the Wrangler deploy success message referencing the correct project, with zero browser or login prompts.*


**40. Dirty Staging Environment Blocking**

*The Pitfall (Deep Context):*
The headless CI was authenticating, but the push instantly aborted claiming the branch was "dirty." This was baffling because we had just pulled from a clean `main` branch.

The cause: Wrangler generates a `.wrangler/` diagnostic state folder dynamically inside the runner during initialization. The runner's internal git environment immediately recognized the repository as containing untracked files. Wrangler refuses to publish from a "dirty" working tree by default, a guard meant to prevent incomplete debug builds from going live. Our own deploy tooling was generating the exact state that its safety check then rejected.

*How we faced it:*
We could not stop Wrangler from creating its diagnostic folders, so we injected an explicit override in the deploy step: `--commit-dirty=true`. This forces Wrangler to ignore local staging artifacts generated by the runner isolation itself and push the payload regardless:
```yaml
run: npx wrangler pages deploy web --commit-dirty=true
```
The alternative of adding `.wrangler/` to `.gitignore` helps locally but does not satisfy the runner's untracked-file check, so the explicit flag is the reliable fix.

-> *Verify: The deploy step completes with no "dirty" rejection, and the Pages production deployment timestamp advances in the Cloudflare dashboard.*


**41. Uploading Manifest Anomalies**

*The Pitfall (Deep Context):*
At the height of our desperation to push to production while Wrangler kept failing, we attempted a reckless maneuver: deploying to Cloudflare Pages via raw `curl` POSTs directly against the Cloudflare REST API, bypassing Wrangler entirely.

We were met with `{code: 8000096, message: manifest missing}`. Modern Edge endpoints do not accept raw static file uploads alone. They require a complex JSON manifest: a cryptographic hashing array mapping every single asset to its content hash, plus correctly formed multipart payloads. Our hand-rolled binary push logic was fundamentally incapable of generating valid manifests on the fly. We were trying to reimplement Wrangler inside a bash script, badly.

*How we faced it:*
We abandoned trying to outsmart the API with `curl` wrappers. We recognized that Wrangler contains the proprietary manifest compilation logic required to deploy Edge assets and functions correctly, and that no amount of bash cleverness would replicate it safely. We enforced strict Wrangler-based deployment as the only sanctioned path to production, and deleted the experimental curl push scripts so nobody could regress into them.

-> *Verify: `npx wrangler pages deploy web` completes with a printed deployment URL, and no custom upload scripts exist (`ls scripts/ | grep -i upload` returns nothing Pages-related).*
