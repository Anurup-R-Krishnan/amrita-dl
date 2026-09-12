#!/usr/bin/env bash
# Runs ON the OCI VM (piped in via `ssh ... 'bash -s' < scripts/oci_deploy_remote.sh`)
# after the calling workflow has already rsynced src/, Cargo.toml, Cargo.lock,
# and scripts/ into ~/amrita-dl. Builds, installs with rollback-on-failure,
# and health-gates before returning success.
set -euo pipefail

cd "$HOME/amrita-dl"
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_BUILD_JOBS=1

# --- Guard: this box has 498MB RAM. Refuse to even try without headroom. ---
avail_kb=$(df --output=avail / | tail -1)
if [ "$avail_kb" -lt 3145728 ]; then
  echo "ERROR: less than 3GB free on / (${avail_kb}KB) - aborting build" >&2
  exit 1
fi
if [ -z "$(swapon --show 2>/dev/null)" ]; then
  echo "ERROR: no swap configured - build would OOM on this box" >&2
  exit 1
fi

echo "--- cargo fetch ---"
cargo fetch --locked

echo "--- cargo build --release --bin server -j 1 ---"
nice -n 19 cargo build --release --locked --bin server -j 1

# --- Install the systemd unit only if it actually changed ---
if ! cmp -s scripts/amrita-server.service /etc/systemd/system/amrita-server.service; then
  echo "--- amrita-server.service changed, installing ---"
  sudo install -m 644 scripts/amrita-server.service /etc/systemd/system/amrita-server.service
  sudo systemctl daemon-reload
fi

echo "--- installing self-heal units ---"
sudo install -m 755 scripts/amrita_selfheal.sh /usr/local/bin/amrita_selfheal.sh
sudo restorecon -v /usr/local/bin/amrita_selfheal.sh || true
if ! cmp -s scripts/amrita-selfheal.service /etc/systemd/system/amrita-selfheal.service 2>/dev/null; then
  sudo install -m 644 scripts/amrita-selfheal.service /etc/systemd/system/amrita-selfheal.service
  sudo systemctl daemon-reload
fi
if ! cmp -s scripts/amrita-selfheal.timer /etc/systemd/system/amrita-selfheal.timer 2>/dev/null; then
  sudo install -m 644 scripts/amrita-selfheal.timer /etc/systemd/system/amrita-selfheal.timer
  sudo systemctl daemon-reload
fi
sudo systemctl enable --now amrita-selfheal.timer

# --- Keep a rollback copy, then swap the binary (stop-before-copy: avoids "Text file busy") ---
if [ -f /usr/local/bin/amrita-server ]; then
  sudo cp -f /usr/local/bin/amrita-server /usr/local/bin/amrita-server.prev
fi
sudo systemctl stop amrita-server
sudo install -m 755 -o root -g root target/release/server /usr/local/bin/amrita-server
sudo restorecon -v /usr/local/bin/amrita-server || true
sudo systemctl start amrita-server

echo "--- health-gating new binary ---"
ok=0
for i in $(seq 1 10); do
  if curl -fsS -m 5 http://localhost:80/api/health | grep -q '"status":"ok"'; then
    ok=1
    break
  fi
  sleep 2
done

if [ "$ok" != "1" ]; then
  echo "ERROR: new binary failed health check - rolling back" >&2
  sudo systemctl stop amrita-server
  if [ -f /usr/local/bin/amrita-server.prev ]; then
    sudo install -m 755 /usr/local/bin/amrita-server.prev /usr/local/bin/amrita-server
    sudo restorecon -v /usr/local/bin/amrita-server || true
  fi
  sudo systemctl start amrita-server
  exit 1
fi

echo "--- deploy OK ---"
