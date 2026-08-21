# Deployment Orchestration & Execution

**7. SSH Protocol Warnings**

*The Pitfall (Deep Context):*
While scripting headless deployment logic against the Oracle VM, we piped stdout from SSH command sequences directly into local parse functions that extracted PIDs and status codes. The parsers suddenly crashed on output that should have been trivially clean.

When we inspected the raw network streams, we found the culprit: Oracle Linux's hardened OpenSSH daemon injects a "Connection not using post-quantum exchanges" warning directly into the standard output stream of every remote command. Running `ssh opc@ip 'echo "hello"'` returned `Connection not using... \n hello` instead of just `hello`. This warning is printed to stdout, not stderr, so standard redirection tricks like `2>/dev/null` do nothing. It shattered JSON parsers, bash conditionals, and variable assignments across our entire automated pipeline.

*How we faced it:*
We wrapped every automated SSH call in an explicit filter: `ssh opc@ip 'cmd' | grep -v 'post-quantum'`. Rather than trusting the stdout stream, we built a sanitization layer that stripped the protocol noise before any downstream parsing. For structured extraction we also switched to explicit delimiters, matching only lines containing the exact marker our scripts emitted. The lesson: never assume remote stdout is clean on hardened distros; filter at the boundary.

-> *Verify: `ssh opc@ip 'echo OK' | grep -v post-quantum` returns exactly one line containing `OK`.*


**8. Git Directory Assumptions**

*The Pitfall (Deep Context):*
Moving from local testing onto the physical VM triggered a complete failure cascade. The original deployment scripts hardcoded the assumption that the codebase lived in `~/amrita-dl/`.

But our new architecture bypassed `git pull` entirely. To prevent leaking untracked security keys and `node_modules` into production, we shipped code as a `source.tar.gz` bundle instead. When that archive extracted, it dumped flat against `/home/opc/` with no subdirectory. The scripts ran `cd ~/amrita-dl/` and immediately threw `No such file or directory`. Because the expected nested folders never existed on the Oracle host, every daemon definition, update script, and compilation hook broke instantly.

*How we faced it:*
We executed a surgical path rectification across the repository. We overwrote the working-directory logic to point execution at exactly `$HOME` (or resolved PWD) instead of assuming legacy nested folders. Every script now derives its paths from variables set at the top of the file rather than inline literals, so a future relocation is a one-line change.

-> *Verify: `ls ~/target/release/server` returns the binary path without any `No such file or directory` error.*


**9. Missing Tarball Assets**

*The Pitfall (Deep Context):*
Our strategy of bypassing git with `source.tar.gz` bundles backfired during the first real deployment. We packaged `src/`, `Cargo.toml`, and `Cargo.lock` and pushed it. Compilation on the remote Oracle node ran cleanly for roughly 40 minutes under `-j 1`. Then, near the end, the Rust macro `include_str!("web/index.html")` fired inside the compiler and the build aborted fatally.

In our haste to bundle the Rust code, our naive `tar` command had completely ignored the `web/` frontend directory. `include_str!` physically reads file contents during compilation and bakes them directly into the binary's memory segment for zero-latency serving. If the file does not exist at compile time, the compiler refuses to produce anything. Forty minutes of compute burned because a single HTML file was missing from the tarball.

*How we faced it:*
We reconstructed the compression sequence explicitly: `tar -czvf source.tar.gz src Cargo.toml Cargo.lock web`. We forced the deployment scripts to never blindly compress. Instead they whitelist the exact required directories so the macro can always find its physical text assets. We also added a pre-flight check on the local machine that lists the tarball contents and greps for `web/index.html` before upload.

-> *Verify: `tar -tzf source.tar.gz | grep web/index.html` resolves to exactly one match before any upload begins.*


**10. Headless Execution Drops**

*The Pitfall (Deep Context):*
The sheer agony of compiling Rust on a 1-core machine peaked when we finally got a successful build running. We watched the terminal slowly churn for 45 minutes. Then disaster struck: we minimized the terminal window to do something else.

At roughly the 5-minute mark of connection inactivity, Oracle Cloud's networking infrastructure severed our idle TCP session. When the SSH tunnel snapped unexpectedly, the Linux kernel transmitted a fatal `SIGHUP` (Hangup) signal to our bash instance. That signal cascaded down the process tree, instantly killing the 45-minute compilation job near completion. An entire hour was lost purely to a network timeout feature.

*How we faced it:*
We accepted that no active SSH session could ever be trusted for long-running execution. We prefixed all compilation commands with `nohup`, redirected output into `build.log`, and detached into the background with `&`. This permanently decoupled the compile job from client network stability:
```bash
nohup cargo build --release -j 1 >> build.log 2>&1 &
echo $! > build.pid
```
Even if SSH dropped mid-build, the process kept running server-side and logged progress to disk. Reconnecting and running `tail -f build.log` showed live status regardless of how many times our laptop disconnected.

-> *Verify: `cat ~/build.pid` returns a PID and `ps -p $(cat ~/build.pid)` confirms the process is alive after disconnecting and reconnecting SSH.*


**11. Asynchronous Service Startup**

*The Pitfall (Deep Context):*
Moving to detached 45-minute background compilation introduced a massive sequential orchestration hurdle: how does the system know when compilation finishes so it can restart the web daemon? We could not manually stare at `tail -f build.log` for half an hour just to type `systemctl restart amrita-server`.

Our naive first attempt simply chained the restart after the build command inside the same background script. But because the script was itself detached, any early-exit condition or partial compile failure still triggered the restart, pointing Systemd at a stale or missing binary. The daemon went dead for 45 minutes on every update while the next build ran, then came back up serving old code or nothing at all.

*How we faced it:*
We wrote a dedicated bash polling loop. The detached watcher checks the modification timestamp of `target/release/server` every few seconds. Only when the mtime advances past the moment the build began does it trigger the downstream Systemd restart:
```bash
START=$(date +%s)
while true; do
  MTIME=$(stat -c %Y ~/target/release/server 2>/dev/null || echo 0)
  if [ "$MTIME" -gt "$START" ]; then break; fi
  sleep 5
done
sudo systemctl restart amrita-server
```
The system became self-aware enough to launch its own updates without human intervention, and only ever restarts on proof of a fresh binary.

-> *Verify: `systemctl is-active amrita-server` reads `active` immediately after the mtime watcher triggers, without manual interaction.*


**12. Systemd Execution Restrictions (203/EXEC)**

*The Pitfall (Deep Context):*
We felt triumphant when the automated compile finished and the polling loop successfully triggered `systemctl restart amrita-server`. We curled localhost expecting a 200 OK. Instead we got connection refused.

Checking journalctl revealed `(code=exited, status=203/EXEC)`. The binary existed on disk, but the system refused to execute it. Oracle Linux runs SELinux aggressively. Systemd executes services under strict policy, and we had instructed it to run a binary living inside `/home/opc/target/release/server`. SELinux actively blocks root-level daemons from executing binaries located inside unprivileged user home directories, a hardening measure designed to prevent privilege escalation attacks through user-writable paths. Our binary was caught by this policy despite being owned by root.

*How we faced it:*
We scrambled our deployment orchestration. We abandoned executing out of the build directory entirely. A post-compile migration hook moves the fresh binary to `/usr/local/bin/amrita-server` and chmods it executable. Operating outside home directory boundaries means SELinux recognizes it as a valid system executable and permits execution cleanly:
```bash
sudo mv target/release/server /usr/local/bin/amrita-server
sudo chmod +x /usr/local/bin/amrita-server
```
This is also why Scenario 8's path rectification matters: both fixes converge on keeping runtime artifacts out of `/home/opc`.

-> *Verify: `systemctl status amrita-server` shows `active (running)` with no `203/EXEC` status in journalctl.*


**13. Daemon Reload Caching**

*The Pitfall (Deep Context):*
While desperately fixing the `203/EXEC` pathing issues from Scenario 12, we edited `/etc/systemd/system/amrita-server.service` to point to `/usr/local/bin`. We excitedly ran `systemctl restart amrita-server`. It failed again with the same complaint about `/home/opc/target/release/server` not existing.

We stared at the screen. The unit file clearly pointed to `/usr/local/...`. The realization: Systemd does not re-read `.service` files from disk on restart. It relies entirely on its pre-loaded in-memory state, silently ignoring our frantic hotfixes on disk. Restarting a daemon only re-executes what Systemd already believes the config says.

*How we faced it:*
We learned the hard way to flush Systemd's cache manually. We injected `systemctl daemon-reload` into our automated update pathway before every restart. This forces the system layer to re-parse all unit definition files on disk so path updates are actually acknowledged before execution:
```bash
sudo systemctl daemon-reload
sudo systemctl restart amrita-server
```
Any script that touches a `.service` file must pair the edit with a reload, or the edit silently never happens.

-> *Verify: `systemctl daemon-reload && systemctl restart amrita-server` starts the binary from `/usr/local/bin` as confirmed by `readlink /proc/$(pidof amrita-server)/exe`.*
