# 🌩️ Production Deployment Guide (Oracle Linux + Cloudflare)

This document contains the exact, copy-pasteable commands customized for your active Oracle Cloud VM (**IP: 140.245.237.213**).

## Phase 1: Push Files to the Oracle Server

Run these commands in your **local computer's terminal** (not the VM). Note: Replace `*08-20*.key` with the exact filename if it is slightly different.

1. **Protect your SSH key** (SSH will refuse to connect if the key permissions are too open):
```bash
chmod 400 ~/Downloads/*08-20*.key
```

2. **Upload the Server and Database** (This will take a minute or two since the DB is ~13MB):
```bash
cd ~/Project/amrita_downloader/amrita-dl

# We use 'opc' because you provisioned Oracle Linux 9
scp -i ~/Downloads/*08-20*.key \
  target/release/server \
  /run/media/anuruprkris/DATA/amrita-exam-papers-indexed/index.db \
  opc@140.245.237.213:/home/opc/
```

---

## Phase 2: Start the Server inside the VM

Now, log directly into the Oracle Virtual Machine:

1. **Connect via SSH:**
```bash
ssh -i ~/Downloads/*08-20*.key opc@140.245.237.213
```

2. **Make the API run forever (Systemd):**
Copy and paste this entire block into your VM terminal and press Enter. It creates a robust background daemon:
```bash
sudo tee /etc/systemd/system/amrita-server.service << 'EOF'
[Unit]
Description=Amrita API Server
After=network.target

[Service]
Type=simple
User=opc
WorkingDirectory=/home/opc
ExecStart=/home/opc/server
Environment="PORT=80"
Environment="INDEX_DB=/home/opc/index.db"
Environment="STORAGE_PUBLIC_URL=https://objectstorage.ap-hyderabad-1.oraclecloud.com/n/axrtbfdmqkku/b/oracle-amrita-bucket/o"
Environment="CORS_ALLOWED_ORIGINS=https://exampapersamrita.pages.dev"
Restart=always

[Install]
WantedBy=multi-user.target
EOF
```

3. **Start the API:**
```bash
sudo chmod +x /home/opc/server
sudo systemctl daemon-reload
sudo systemctl enable --now amrita-server
```

4. **Open the Oracle Linux OS Firewall:**
Oracle Linux firmly blocks external traffic out of the box. Run this to open port 80:
```bash
sudo firewall-cmd --zone=public --add-port=80/tcp --permanent
sudo firewall-cmd --reload
```

---

## Phase 3: Open the OCI Cloud Firewall

Now that your OS is accepting traffic, you must tell the Oracle Cloud Dashboard to let internet traffic reach the VM.

1. In the **Oracle Cloud Console**, go to your VM's details page.
2. Under "Primary VNIC", click on your Subnet string text (e.g., `Subnet-amrita-paper`).
3. Click on the **Security List** (e.g., `Default Security List for Vcn-amrita-paper`).
4. Click **Add Ingress Rules**.
   - **Source CIDR:** Type `0.0.0.0/0`
   - **Destination Port Range:** Type `80`
   - Click the **Add Ingress Rules** button.

---

## Phase 4: Route the Frontend to your IP

Since we aren't using a custom root domain right now, we will simply point the Cloudflare Pages reverse proxy directly to your Oracle VM's public IP address.

1. **Open a new local terminal (Disconnect from the VM).**
2. Point your API redirect proxy straight to the Oracle IP:
```bash
cd ~/Project/amrita_downloader/amrita-dl
echo "/api/* http://140.245.237.213/api/:splat 200" > web/_redirects
```
3. Deploy the UI to the Cloudflare network:
```bash
CLOUDFLARE_API_TOKEN="" npx wrangler pages deploy web --project-name exampapersamrita --commit-dirty=true
```

## Validation
Once completed, simply open your browser and visit:
**[https://exampapersamrita.pages.dev](https://exampapersamrita.pages.dev)**

The API is now natively hooked up!
