# Production Deployment Guide (Oracle Linux + Cloudflare)

This document contains the exact, copy-pasteable commands customized for your active Oracle Cloud VM (IP: 68.233.111.2).

## Phase 1: Oracle Environment Bootstrapping

Oracle Free Tier instances require specific GCC toolchains to compile bundled `rusqlite`.

```bash
# Log into the Oracle Virtual Machine
ssh -i ~/Downloads/ssh-key-2026-08-20.key opc@68.233.111.2

# Install Required C Toolchains for Bindgen
sudo dnf install -y gcc clang llvm-devel glibc-devel

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

## Phase 2: Compilation and Systemd Migration

Due to kernel panic restraints on 1GB RAM instances, compilation runs in a background thread utilizing constrained limits (`codegen-units=16`, `-j 1`).

```bash
tar xzf source.tar.gz
# Compile silently in background
nohup cargo build --release --bin server -j 1 > ~/build.log 2>&1 &
```

Once `target/release/server` executes successfully, SELinux prevents executing binaries directly inside `/home/opc/` for background daemons.

```bash
# Escalate binary to system execution paths
sudo cp ~/target/release/server /usr/local/bin/amrita-server
sudo chmod 755 /usr/local/bin/amrita-server

# Escalate systemd configuration
sudo cp scripts/amrita-server.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable amrita-server
sudo systemctl start amrita-server
```

## Phase 3: Cloudflare Tunnel Persistence

To bypass aggressive OCI Firewalls and Cloudflare origin TLS requirements, local port 80 is bridged directly via a named encrypted tunnel.

```bash
# Download native Cloudflared daemon
curl -fsSL 'https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64' -o /tmp/cloudflared
sudo mv /tmp/cloudflared /usr/local/bin/cloudflared
sudo chmod +x /usr/local/bin/cloudflared

# Establish daemon persistence
sudo tee /etc/systemd/system/cloudflared-tunnel.service > /dev/null << 'EOF'
[Unit]
Description=Cloudflare Quick Tunnel to amrita-server
After=network.target amrita-server.service

[Service]
Type=simple
User=opc
ExecStart=/usr/local/bin/cloudflared tunnel --url http://localhost:80 --logfile /home/opc/tunnel.log
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable cloudflared-tunnel
sudo systemctl start cloudflared-tunnel
```