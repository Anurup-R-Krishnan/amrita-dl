# Deployment Orchestration & Execution

**7. SSH Protocol Warnings**
*The Pitfall:* During our headless, detached compilations, our silent background bash payloads were acting up. Digging into the streams, we realized that Oracle's native SSH protocols were polluting standard output with eternal "Connection not using post-quantum exchanges" warnings. This raw string pollution actively corrupted our piped payloads and shell executions, breaking our deployment automation silently.
*How we faced it:* We had to aggressively construct an explicit `grep -v 'post-quantum'` inversion matrix around our SSH logic wrappers. We brutally filtered out the protocol noise entirely from standard output, preserving only the pure data execution strings that our downstream architecture actually expected.
-> *Verify: SSH commands executed natively return null STDERR anomalies limiting pure stdout data lines solely.*

**8. Git Directory Assumptions**
*The Pitfall:* Everything failed when we transitioned to the VM. The original legacy scripts simply hardcoded structural assumptions—they believed the codebase was explicitly cloned into `~/amrita-dl/`. 
Because we fundamentally bypassed Git syncing using the explicit `source.tar.gz` data-transfer model, the zip extracted entirely flat against the root `/home/opc/` user boundary. Every single execution script threw chaotic `File Not Found` crashes because the expected sub-directories never existed on the Oracle Linux host.
*How we faced it:* We ran exhaustive re-mapping configurations, manually overwriting the working directory logic natively pointing target executions dynamically against exactly `~/` base origins instead of nested folders.
-> *Verify: `ls ~/target/release/server` natively returns execution environments bypassing sub-folder strictions.*

**9. Missing Tarball Assets**
*The Pitfall:* When we decided to bypass Git completely (to avoid `node_module` leaks and untracked artifacts) we bundled the source code. The compilation ran perfectly until the 80% mark, where it hit `include_str!("web/index.html")`. The server compiler terminated abruptly. Our hasty compression command ignored the `web/` frontend nodes entirely. The Rust macro failed mechanically because the required DOM string didn't physically exist in the bundle.
*How we faced it:* We rigidly reconstructed the compression matrix (`tar -czvf`) explicitly declaring directory inclusive parameters binding the `web/` node mapping securely. Once uploaded, the macro flawlessly resolved the DOM.
-> *Verify: `tar tf source.tar.gz | grep web/index.html` resolves dynamically avoiding static build drops.*

**10. Headless Execution Drops**
*The Pitfall:* The native builds on 1 core were taking 25 minutes to complete. We mistakenly ran raw `cargo build` commands over SSH. At precisely the 5-minute mark of screen inactivity, the Oracle cloud networking infrastructure aggressively severed TCP KeepAlive endpoints. When SSH broke, Linux transmitted consecutive `SIGHUP` terminations to our bash instance, aborting our 25-minute compilation completely.
*How we faced it:* We forcibly injected `nohup` (No Hang Up) prefix bindings across all compilation commands natively. By mapping output streams into `build.log` and executing implicitly in the background `&`, we permanently decoupled the compilation matrix from network instability limits.
-> *Verify: `cat ~/build.pid` returns daemon execution pointers ensuring background processing holds persistently.*

**11. Asynchronous Service Startup**
*The Pitfall:* Automating a massive 25-minute detached background compile threw up a massive sequential hurdle: how does the system know when to start the web daemon? We couldn't manually stare at a terminal for half an hour just to type `systemctl start`. The continuous deployment integration broke completely as it attempted to reboot non-existent binary payloads.
*How we faced it:* We coded a dedicated bash-based asynchronous polling loop. The detached script checked explicitly for the exact file creation of `target/release/server` continuously. The very second the file resolved gracefully, it autonomously triggered the downstream Systemd daemon hooks without manual operator delays.
-> *Verify: `systemctl is-active amrita-server` reads active bypassing manual interactions effectively tracking post-compile signals.*

**12. Systemd Execution Restrictions (203/EXEC)**
*The Pitfall:* After finally wrestling the compilation to 100%, we hooked the output absolute path (`/home/opc/target/release/server`) into our systemd configuration. Launching the OS daemon triggered an immediate, catastrophic (code=exited, status=203/EXEC) response. Oracle Linux's native SELinux permissions actively and violently banned the root-based Systemd daemon from touching user-space execution bins. 
*How we faced it:* We fundamentally scrapped user-space deployment. We aggressively migrated the resultant binaries directly onto secure, kernel-level mappings shifting to `/usr/local/bin/amrita-server`. By operating strictly outside of home directory boundaries, SELinux enforcement restrictions mapped to execution permissions properly.
-> *Verify: `systemctl status amrita-server` logs execution running successfully overriding `EXEC/203` status anomalies.*

**13. Daemon Reload Caching**
*The Pitfall:* Our frustration mounted as we changed the `ExecStart` mapping fixing the SELinux 203 paths. Re-running `systemctl start amrita-server` failed yet again exactly identifying the old broken `opc` paths. Linux Systemd utilizes extreme internal caching models overriding active external text file changes fundamentally dropping all code modifications gracefully explicitly.
*How we faced it:* The `systemctl daemon-reload` command was forcibly commanded validating internal caching clears reloading strictly mapping the current state path logic universally tracking the exact code locations efficiently.
-> *Verify: `systemctl daemon-reload && systemctl restart amrita-server` resolves mapping to the correct binary natively.*