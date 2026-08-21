# Deployment Orchestration & Execution

**7. SSH Protocol Warnings**
- *Roadblock:* "Connection not using post-quantum exchanges" log pollution disrupted background bash payload piping natively.
- *Fix:* Aggregated SSH logic filtering `grep -v 'post-quantum'` enforcing strictly parsing outputs without log corruption.
-> *Verify: SSH commands executed natively return null STDERR anomalies limiting pure stdout data lines solely.*

**8. Git Directory Assumptions**
- *Roadblock:* Legacy environments assumed repo existed under `~/amrita-dl/`, leading directly to structural mapping `fn not found` runtime failures.
- *Fix:* Re-mapped variables pointing execution spaces directly against Oracle user space origins `~`.
-> *Verify: `ls ~/target/release/server` natively returns execution environments bypassing sub-folder strictions.*

**9. Missing Tarball Assets**
- *Roadblock:* UI directory `web/` was explicitly orphaned skipping initial transfers generating `include_str!` macro termination mid-compile securely.
- *Fix:* Re-bundled `source.tar.gz` capturing the layout strictly inclusive.
-> *Verify: `tar tf source.tar.gz | grep web/index.html` resolves dynamically avoiding static build drops.*

**10. Headless Execution Drops**
- *Roadblock:* Executing raw build commands aborted compilation abruptly when remote terminal TCP endpoints encountered closure.
- *Fix:* Implemented `nohup` detachment bridging PID mappings explicitly allowing silent resolution states.
-> *Verify: `cat ~/build.pid` returns daemon execution pointers ensuring background processing holds persistently.*

**11. Asynchronous Service Startup**
- *Roadblock:* Could not afford blocking human intervention testing for build completions taking roughly 45 minutes manually.
- *Fix:* Authored detached polling script scanning for structural file existences unconditionally launching downstream daemonization. 
-> *Verify: `systemctl is-active amrita-server` reads active bypassing manual interactions effectively tracking post-compile signals.*

**12. Systemd Execution Restrictions (203/EXEC)**
- *Roadblock:* SELinux actively blocks raw execution environments spanning user boundaries yielding `Permission Denied` specifically via Systemd.
- *Fix:* Relocated executable forcibly migrating runtime components bridging `/usr/local/bin` mappings directly.
-> *Verify: `systemctl status amrita-server` logs execution running successfully overriding `EXEC/203` status anomalies.*

**13. Daemon Reload Caching**
- *Roadblock:* Replacing variables in active SystemD services failed propagation keeping nodes broken despite code availability.
- *Fix:* Forced daemon-reload execution structures tracking new modifications.
-> *Verify: `systemctl daemon-reload && systemctl restart amrita-server` resolves mapping to the correct binary natively.*