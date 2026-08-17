# Amrita Exam Papers -- 100% Free Cloud Deployment Guide

This guide details how to host the Amrita Exam Papers Search Engine completely free ($0/month forever) with enterprise-grade performance, SSL certificates, global CDN, and automatic DDoS protection.

---

## Recommended Stack (100% Free Forever)

| Component | Service | Cost | Function |
| :--- | :--- | :--- | :--- |
| **Compute & Storage** | Oracle Cloud (OCI) Always Free | $0.00 / mo | 4 ARM Ampere Cores, 24 GB RAM, 200 GB Storage |
| **Domain & DNS** | Cloudflare Free Tier | $0.00 / mo | Free SSL, DNS routing, Edge CDN |
| **HTTPS Tunnel** | Cloudflare Tunnels (`cloudflared`) | $0.00 / mo | Zero open inbound ports, instant HTTPS |
| **Health Monitoring** | UptimeRobot / Better Stack | $0.00 / mo | Free 5-minute HTTP uptime monitor |

---

## Step 1: Generate Cloud Deployment Bundle

On your local development machine, run the automated bundle script:

```bash
./scripts/prepare_cloud_deploy.sh
```

This compiles the optimized release binary and bundles `index.db` and `amrita-exam-papers-indexed/` into `./dist` (automatically omitting the raw files to save 50%+ disk space).

---

## Step 2: Provision Oracle Cloud (OCI) Instance

1. Create a free account at [oracle.com/cloud/free](https://www.oracle.com/cloud/free/).
2. Navigate to **Compute > Instances > Create Instance**.
3. Select **Image**: Ubuntu 22.04 LTS (or Debian 12 ARM64).
4. Select **Shape**: `VM.Standard.A1.Flex` (Assign 2 to 4 OCPUs and 12-24 GB RAM under Always Free).
5. Add your SSH Public Key and click **Create**.

---

## Step 3: Server Setup & Deployment

1. SSH into your OCI instance:
   ```bash
   ssh ubuntu@<YOUR_OCI_PUBLIC_IP>
   ```

2. Install Docker and Docker Compose:
   ```bash
   sudo apt-get update
   sudo apt-get install -y docker.io docker-compose-v2
   sudo usermod -aG docker $USER
   newgrp docker
   ```

3. Transfer your `./dist` directory or git repo to the instance:
   ```bash
   scp -r ./dist ubuntu@<YOUR_OCI_PUBLIC_IP>:~/amrita-app
   ```

4. Launch the application:
   ```bash
   cd ~/amrita-app
   docker compose up -d --build
   ```

5. Verify server health locally on the VM:
   ```bash
   curl http://localhost:8080/api/health
   # Expected output: {"status":"ok","database":"connected","total_papers":19600}
   ```

---

## Step 4: Configure Cloudflare Tunnel (Zero Open Ports)

Using Cloudflare Tunnels exposes your app over HTTPS without opening port 80/443 on your OCI firewall.

1. In Cloudflare Dashboard, go to **Zero Trust > Networks > Tunnels**.
2. Click **Create a Tunnel**, name it `amrita-papers`.
3. Follow the single line install command provided for Linux ARM64:
   ```bash
   sudo cloudflared service install <YOUR_TUNNEL_TOKEN>
   ```
4. Route traffic to:
   - **Service Type**: HTTP
   - **URL**: `localhost:8080`
   - **Public Hostname**: `papers.yourdomain.com` (or your chosen domain).

Your application is now live worldwide over HTTPS with instant SSL!

---

## Step 5: Free Monitoring & Uptime Alerts

1. Sign up for a free account at [uptimerobot.com](https://uptimerobot.com) or [betterstack.com](https://betterstack.com).
2. Create an **HTTP(s) Monitor**:
   - **URL**: `https://papers.yourdomain.com/api/health`
   - **Interval**: Every 5 minutes
3. Enable email/SMS notifications for downtime alerts.
