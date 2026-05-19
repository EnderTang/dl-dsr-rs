#!/usr/bin/env sh
set -eu

mkdir -p target/bench
for hops in 4 8 16 32; do
  for payload in 64 512; do
    cargo run -q -p dldsr-bench -- --compare --hops "$hops" --payload-size "$payload" --packets "${PACKETS:-1000}" \
      > "target/bench/chain-${hops}-${payload}.md"
    cargo run -q -p dldsr-bench -- --compare --hops "$hops" --payload-size "$payload" --packets "${PACKETS:-1000}" --json \
      > "target/bench/chain-${hops}-${payload}.json"
  done
done

echo "wrote target/bench/*.md and target/bench/*.json"

