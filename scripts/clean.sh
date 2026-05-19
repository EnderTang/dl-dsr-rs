#!/usr/bin/env sh
set -eu

cargo clean
rm -f docker-compose.generated.yml
rm -rf target/bench

