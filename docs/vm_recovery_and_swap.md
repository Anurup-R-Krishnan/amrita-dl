# VM Kernel Recovery & Custom Swap Setup

## The ap-hyderabad Ampere Exhaustion 
The original architectural motive for this project was to leverage a zero-cost, high-performance Oracle Cloud `VM.Standard.A1.Flex` (Ampere A1) instance with 4 ARM cores and 24GB of RAM to blitz through Rust compilation and serve the 19,600+ database records flawlessly. 

**The Reality:** The `ap-hyderabad` region was completely exhausted of Ampere capacity. We spent days fighting extreme capacity limits and out of pure desperation, we fell back to the only available shape: the `VM.Standard.E2.1.Micro` tier. This x86_64 legacy node came with a microscopic 1GB of Kingston/Samsung physical RAM and a single starved core. 

## Initial Kernel Panic
- **Trigger:** Using `cargo` natively on a fresh Oracle OS install on this microscopic node.
- **Symptom:** Because Linux completely freezes when compiling code with limited RAM, the Virtual Machine violently locked up. TCP packets timed out, SSH refused to resolve banner exchanges, and the OCI dashboard monitoring eventually reported a fatal "VM Terminated". The OOM-killer was instantly assassinating `rustc` before dropping the entire kernel.

## Solution: Brutal Allocation of an Artificial 4GB Swap
Instead of trusting the limited 1GB of physical RAM, we had to artificially trick the kernel by carving an exact 4GB chunk out of the hard drive to act as a fallback "overflow bucket" (Swap File) directly on the SSD block volume. When Rust loads a file that exceeds the 1GB physical limit, the kernel temporarily places dormant memory pages into the hard drive file instead of sending a `SIGKILL` (OOM) out-of-memory death flag. We successfully turned a 1GB Oracle IoT computer into a massive 5GB RAM machine by trading compilation speed for heavy disk I/O.

### The Swap Survival Sequence

**1. Carve exactly 4 Gigabytes out of the Oracle OS Disk Space**
```bash
sudo fallocate -l 4G /swapfile
```
-> *Verify: `ls -lh /swapfile` confirms 4.0G file size exists.*

**2. Secure it so non-root users cannot read the active memory blocks**
```bash
sudo chmod 600 /swapfile
```
-> *Verify: `ls -ld /swapfile` explicitly reports `-rw-------` permissions.*

**3. Format the blank space into a Memory Page File structure**
```bash
sudo mkswap /swapfile
```
-> *Verify: System outputs `Setting up swapspace version 1` confirming formatting.*

**4. Activate the memory blocks forcibly into the active kernel pool**
```bash
sudo swapon /swapfile
```
-> *Verify: `free -h` outputs `Swap: 4.0G` confirming the buffer is fully active.*

**5. Guarantee permanence forever upon VM reboot (fstab mount)**
```bash
echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
```
-> *Verify: `cat /etc/fstab | grep swap` successfully reads out the persistent volume mount.*