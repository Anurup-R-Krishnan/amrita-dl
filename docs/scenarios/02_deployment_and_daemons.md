# Deployment Orchestration & Execution

**7. SSH Protocol Warnings**
*The Pitfall (Deep Context):* Attempting to script our headless deployment logic across Oracle Cloud introduced us to incredibly frustrating OS-level quirks. We were routing stdout from SSH command sequences directly into localized parse functions (like pulling PIDs or status codes). Suddenly, our parsers fatally crashed.
When we inspected the raw network streams, we discovered that Oracle Linux's hardened OpenSSH daemon aggressively injected "Connection not using post-quantum exchanges" warnings directly into the standard output stream of every single remote command. If we executed `ssh opc@ip 'echo "hello"'`, the output was literally `Connection not using... \n hello`. This unsuppressable cryptographic protocol warning actively shattered our piped payloads, breaking JSON parsers, bash conditionals, and variable assignments unconditionally across our entire automated pipeline. 

*How we faced it:* We had to aggressively construct an explicit `grep -v 'post-quantum'` inversion matrix around all our automated SSH logic wrappers. Instead of trusting the standard output stream naturally, we brutally filtered out the protocol string noise entirely. We effectively built a sanitization layer over the OS network stack, preserving only the pure execution strings that our downstream architecture expected.
-> *Verify: SSH commands executed natively return null STDERR anomalies limiting pure stdout data lines solely.*


**8. Git Directory Assumptions**
*The Pitfall (Deep Context):* The transition from local testing onto the physical hardware of the VM triggered a complete failure cascade. The original legacy scripts, written by a previous engineer, made incredibly naive, hardcoded assumptions about the filesystem layout. They inherently believed the codebase was explicitly cloned and localized into `~/amrita-dl/`. 
However, in our new architecture, we fundamentally bypassed standard `git pull` operations. To prevent leaking untracked security keys and `node_modules` into production, we utilized an explicit `source.tar.gz` data-transfer model. When the zip extracted, it dumped perfectly flat against the root `/home/opc/` user boundary. The scripts wildly executed `cd ~/amrita-dl/` and immediately threw fatal `No such file or directory` drops. Because the expected sub-directories never existed on the Oracle Linux host, every daemon, update script, and compilation hook broke instantly.

*How we faced it:* We had to execute a surgical root-path rectification matrix across the repository. We manually overwrote the working directory logic natively, pointing target executions dynamically against exactly `~/` (or resolving the exact PWD) instead of relying on legacy nested structural folders.
-> *Verify: `ls ~/target/release/server` natively returns execution environments bypassing sub-folder strictions.*


**9. Missing Tarball Assets**
*The Pitfall (Deep Context):* Our strategy to bypass Git and rely on `source.tar.gz` bundling backfired during our first deployment. We packaged `src/`, `Cargo.toml`, and `Cargo.lock` into the bundle and pushed it. The compilation on the remote Oracle node ran flawlessly for roughly 40 minutes under our `-j 1` chokehold. Then, at the 99% mark, the Rust macro `include_str!("web/index.html")` executed and the compiler instantly terminated with a fatal execution abort.
In our haste to bundle the rust code, our naive `tar` command completely ignored the `web/` frontend nodes. The Rust compiler uses `include_str!` to physically read file contents *during compilation* and bake them directly into the binary's memory segment (for absolute zero-latency serving). The physical DOM file didn't exist in the bundle. 

*How we faced it:* We rigidly reconstructed the compression matrix sequence (`tar -czvf`) explicitly declaring directory inclusive parameters binding the `web/` node mapping securely. We forced the deployment scripts to never blindly compress, but to explicitly whitelist the exact required directories so the macro could flawlessly parse the physical text assets.
-> *Verify: `tar tf source.tar.gz | grep web/index.html` resolves dynamically avoiding static build drops.*


**10. Headless Execution Drops**
*The Pitfall (Deep Context):* The sheer agony of compiling Rust on a 1-core machine came to a head when we attempted our first successful builds. We watched the terminal slowly parse over 45 minutes. But then, disaster struck. We minimized the terminal window to do something else. 
At exactly the 5-minute mark of screen inactivity, the Oracle cloud networking infrastructure—which aggressively limits idle connections—severed our TCP KeepAlive endpoints. When the SSH tunnel unexpectedly snapped, the Linux kernel transmitted a fatal `SIGHUP` (Hangup) signal to our bash instance. This cascaded down the process tree, instantly assassinating our 45-minute compilation job at 99%. An entire hour was lost purely to a network timeout feature.

*How we faced it:* We realized we could never trust an active SSH session for long-running execution. We forcibly injected `nohup` (No Hang Up) prefix bindings across all compilation commands natively. By redirecting output streams into `build.log` and executing implicitly in the detached background via `&`, we permanently decoupled the compilation matrix from our client network stability.
-> *Verify: `cat ~/build.pid` returns daemon execution pointers ensuring background processing holds persistently.*


**11. Asynchronous Service Startup**
*The Pitfall (Deep Context):* Moving to detached, 45-minute background compilation introduced a massive sequential orchestration hurdle: how does the system know when the compilation finishes so it can restart the web daemon? We couldn't manually stare at the `tail -f build.log` for half an hour just to type `systemctl restart amrita-server`. Our continuous deployment pipeline broke completely as it attempted to reboot the daemon before the binary even existed, leading to the server being dead for 45 minutes on every update.

*How we faced it:* We coded a dedicated bash-based asynchronous polling loop. The detached script executed a `while loop` checking explicitly for the exact file modification time of `target/release/server`. The very millisecond the file resolved gracefully and completely, the script autonomously executed the downstream Systemd daemon hooks. The system was now self-aware enough to launch its own updates without human intervention.
-> *Verify: `systemctl is-active amrita-server` reads active bypassing manual interactions effectively tracking post-compile signals.*


**12. Systemd Execution Restrictions (203/EXEC)**
*The Pitfall (Deep Context):* We felt triumphant when the automated compile finished. The polling loop successfully triggered `systemctl restart amrita-server`. We curled the localhost, expecting a 200 OK. Instead, we got connection refused. 
Checking the systemctl logs revealed an immediate, catastrophic `(code=exited, status=203/EXEC)`. The binary existed, but the system refused to run it. Oracle Linux utilizes extremely aggressive SELinux permissions. Systemd runs as a root-level daemon, and we had instructed it to execute a binary residing inside `/home/opc/target/release/server`. SELinux actively and violently bans root-based daemons from touching binaries located inside unprivileged user home networks to prevent privilege escalation attacks.

*How we faced it:* We fundamentally scrambled our deployment orchestration. We abandoned trying to execute out of the build directory. We aggressively injected a post-compile migration hook, moving the resultant binaries directly onto secure, kernel-level mappings shifting them to `/usr/local/bin/amrita-server`. By operating strictly outside of home directory boundaries, SELinux recognized it as a valid system executable and allowed execution seamlessly.
-> *Verify: `systemctl status amrita-server` logs execution running successfully overriding `EXEC/203` status anomalies.*


**13. Daemon Reload Caching**
*The Pitfall (Deep Context):* As we were desperately fixing the `203/EXEC` pathing issues, we modified our `/etc/systemd/system/amrita-server.service` file to point to `/usr/local/bin`. We excitedly ran `systemctl restart amrita-server` again. It failed again, explicitly complaining that `/home/opc/target/release/server` didn't exist.
We stared at the screen. The text file clearly pointed to `/usr/local/...`. We realized that Linux Systemd utilizes extreme internal memory caching models. It does not natively re-read its own `.service` files from disk when restarting a daemon; it relies entirely on its pre-loaded memory state, fundamentally ignoring our frantic hotfixes.

*How we faced it:* We learned the hard way that you must manually command systemd to flush its caches. We forcibly injected `systemctl daemon-reload` into our automated update pathways. This command brutally forces the system layer to re-parse all physical definition text files on disk, ensuring that our pathing updates were actually acknowledged before execution.
-> *Verify: `systemctl daemon-reload && systemctl restart amrita-server` resolves mapping to the correct binary natively.*