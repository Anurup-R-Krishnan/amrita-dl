# VM Kernel Recovery & Custom Swap Setup

Because we were unable to secure an Ampere A1 (24GB RAM) shape due to extreme capacity limits in `ap-hyderabad`, we fell back to the 1GB RAM `Micro` tier. Because Linux completely freezes when compiling code with limited RAM, we implemented an artificial high-speed buffer directly on the SSD block volume.

## Initial Kernel Panic
- **Trigger:** Using `cargo` natively on a fresh Oracle OS install.
- **Symptom:** Virtual Machine locked up, TCP packets timed out, SSH refused to resolve banner exchanges, and dashboard monitoring failed ("VM Terminated" eventually).

## Solution: Creating an Artificial 4GB Swap
Instead of trusting the limited 1GB of physical Kingston/Samsung RAM, we carved out exactly 4GB of the hard drive to act as a fallback "overflow bucket" (Swap File). When rust loads a file that exceeds the 1GB physical limit, the kernel temporarily places dormant memory pages into the hard drive file instead of sending a `SIGKILL` (OOM) out-of-memory death flag.

### The Shell Command Used:
```bash
# 1. Carve exactly 4 Gigabytes out of the Oracle OS Disk Space
sudo fallocate -l 4G /swapfile

# 2. Secure it so non-root users cannot read the active memory blocks
sudo chmod 600 /swapfile

# 3. Format the blank space into a Memory Page File structure
sudo mkswap /swapfile

# 4. Turn it on
sudo swapon /swapfile

# 5. Make it permanent forever upon VM reboot (fstab mount)
echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
```

Using this, a free-tier 1GB Oracle IoT computer successfully acts identical to a massive 5GB RAM Developer Machine simply by trading compilation speed for disk I/O.
