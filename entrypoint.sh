#!/usr/bin/env bash
set -e

if [ -n "$CLOUDFLARE_TUNNEL_TOKEN" ]; then
  echo "[*] Đang khởi chạy Cloudflare Tunnel..."
  cloudflared tunnel --no-autoupdate run --token "$CLOUDFLARE_TUNNEL_TOKEN" &
fi

chmod +x ./zenne-bot
exec ./zenne-bot "$@"
