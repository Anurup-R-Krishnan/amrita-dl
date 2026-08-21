# Production Deployment (Oracle Linux + Cloudflare)

Target VM IP: `68.233.111.2`. User: `opc`.

## 1. Bootstrapping

**Install toolchains & Rust**
```bash
ssh -i ~/Downloads/ssh-key-2026-08-20.key opc@68.233.111.2
sudo dnf install -y gcc clang llvm-devel glibc-devel
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```
*Verify: `gcc --version` and `cargo --version` execute successfully.*

## 2. Compilation and Daemonization

**Compile constrained background build**
```bash
tar xzf source.tar.gz
nohup cargo build --release --bin server -j 1 > ~/build.log 2>&1 &
```
*Verify: `tail ~/build.log` outputs "Finished release".*

**Install binary to path and Daemonize**
```bash
sudo cp ~/target/release/server /usr/local/bin/amrita-server
sudo chmod 755 /usr/local/bin/amrita-server
sudo cp scripts/amrita-server.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now amrita-server
```
*Verify: `systemctl status amrita-server` reads "active (running)".*
*Verify: `ss -tlnp | grep :80` confirms binds to port 80.*

## 3. Cloudflare Tunnel Persistence

**Install cloudflared and initialize service**
```bash
curl -fsSL 'https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64' -o /tmp/cloudflared
sudo mv /tmp/cloudflared /usr/local/bin/cloudflared
sudo chmod +x /usr/local/bin/cloudflared

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
sudo systemctl enable --now cloudflared-tunnel
```
*Verify: `systemctl status cloudflared-tunnel` reads "active (running)".*
*Verify: `grep -o 'https://[a-z0-9-]*\.trycloudflare\.com' ~/tunnel.log` returns live HTTPS URL.*