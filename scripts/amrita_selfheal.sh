#!/usr/bin/env bash
# Runs every 2 minutes via amrita-selfheal.timer on the OCI VM.
# 1. Restarts amrita-server if its local health check fails.
# 2. Publishes the current cloudflared quick-tunnel hostname to the edge
#    proxy's KV-backed origin whenever it changes, so a tunnel restart
#    (new random *.trycloudflare.com URL) never requires a code change
#    or redeploy - this is the fix for the outage that started this.
# 3. Restarts the tunnel if it's up as a process but not actually reachable.
#
# Expects ORIGIN_UPDATE_SECRET and PAGES_URL in the environment
# (systemd unit loads them from /etc/amrita/origin.env).
set -uo pipefail

STATE_FILE="$HOME/.amrita-origin"
TAG="amrita-selfheal"

log() { logger -t "$TAG" -- "$1"; }

# --- 1. Local backend health ---
if ! curl -fsS -m 5 http://localhost:80/api/health >/dev/null 2>&1; then
  log "amrita-server health check failed, restarting"
  sudo systemctl restart amrita-server
fi

# --- 2. Discover current tunnel URL and publish if changed ---
if [ -z "${ORIGIN_UPDATE_SECRET:-}" ] || [ -z "${PAGES_URL:-}" ]; then
  log "ORIGIN_UPDATE_SECRET/PAGES_URL not set, skipping origin publish"
  exit 0
fi

CURRENT_URL=$(journalctl -u cloudflared-tunnel --since -24h -o cat 2>/dev/null \
  | grep -o 'https://[a-z0-9-]*\.trycloudflare\.com' | tail -1)

if [ -z "$CURRENT_URL" ]; then
  log "no tunnel URL found in recent logs"
else
  PREV_URL=""
  [ -f "$STATE_FILE" ] && PREV_URL=$(cat "$STATE_FILE")

  if [ "$CURRENT_URL" != "$PREV_URL" ]; then
    if curl -fsS -m 10 -X POST \
      -H "Authorization: Bearer $ORIGIN_UPDATE_SECRET" \
      --data "$CURRENT_URL" \
      "$PAGES_URL/api/_origin" >/dev/null 2>&1; then
      echo "$CURRENT_URL" > "$STATE_FILE"
      log "published new origin $CURRENT_URL"
    else
      log "failed to publish new origin $CURRENT_URL, will retry next tick"
    fi
  fi

  # --- 3. Is the tunnel actually reachable? ---
  if ! curl -fsS -m 8 "$CURRENT_URL/api/health" >/dev/null 2>&1; then
    log "tunnel URL $CURRENT_URL is unreachable, restarting cloudflared-tunnel"
    sudo systemctl restart cloudflared-tunnel
  fi
fi

exit 0
