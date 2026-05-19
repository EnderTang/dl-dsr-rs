#!/usr/bin/env sh
set -eu

NODES="${NODES:-4}"
MODE="${MODE:-dldsr}"
OUT="${OUT:-docker-compose.generated.yml}"

python3 scripts/gen_compose.py --nodes "$NODES" --mode "$MODE" --output "$OUT"
docker compose -f "$OUT" up --build

