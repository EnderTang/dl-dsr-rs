#!/usr/bin/env sh
set -eu

set -- dldsr-node \
  --node-id "${NODE_ID:-0}" \
  --bind "${BIND_ADDR:-0.0.0.0:7000}" \
  --topology "${TOPOLOGY:-/app/docker/topology/chain-4.toml}" \
  --mode "${MODE:-dldsr}"

if [ -n "${SEND_DST:-}" ]; then
  set -- "$@" --send-dst "$SEND_DST" --payload "${PAYLOAD:-hello from docker}" --send-after-ms "${SEND_AFTER_MS:-2500}"
fi

exec "$@"
