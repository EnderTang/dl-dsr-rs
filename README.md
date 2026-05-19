# dl-dsr-rs

Rust lab implementation of Dynamic Label-based Source Routing (DL-DSR) alongside a baseline DSR mode.

The daemon runs each node as an independent UDP process. Docker containers can all share one bridge network, while the protocol only sends Hello/HelloReply traffic to logical neighbors from the topology file.

## What Is Implemented

- Cargo workspace with core protocol types, node daemon, benchmark CLI, and io_uring prototype crate.
- DSR path overhead model using 32-bit node identifiers.
- DL-DSR path overhead model using 8-bit dynamic labels.
- Neighbor Table and Label Table with deterministic tests.
- UDP daemon with Hello/HelloReply, RREQ/RREP route discovery, route caching, and multi-hop Data forwarding.
- Runtime UDP control socket for asking a running node to send data.
- Live metrics counters with optional JSON file output.
- Docker Compose generator for 4/8/16/32-node chain topologies.
- Benchmark output as Markdown or JSON, including side-by-side DSR vs DL-DSR comparison.
- eBPF/XDP and io_uring prototype notes that do not block the main build.

## Build And Test

```bash
cargo fmt
cargo test
cargo clippy --workspace --all-targets
```

## Run A Local Node

```bash
cargo run -p dldsr-node -- --config examples/node0.toml
```

Or use explicit arguments:

```bash
cargo run -p dldsr-node -- \
  --node-id 0 \
  --bind 0.0.0.0:7000 \
  --topology docker/topology/chain-4.toml \
  --mode dldsr
```

## Benchmark

Single mode Markdown:

```bash
cargo run -p dldsr-bench -- --mode dldsr --hops 4 --payload-size 64 --packets 100
```

Side-by-side JSON:

```bash
cargo run -p dldsr-bench -- --compare --hops 32 --payload-size 512 --packets 1000 --json
```

The benchmark is a deterministic model for comparing source-route header load, estimated latency, and energy proxy. It is intentionally separate from the daemon so experiments are reproducible without requiring Docker timing stability.

## Docker Demo

Generate and run a 4-node chain:

```bash
python3 scripts/gen_compose.py --nodes 4 --mode dldsr --output docker-compose.generated.yml
docker compose -f docker-compose.generated.yml up --build
```

The generated compose file starts one `dldsr-node` container per logical node. Node 0 automatically triggers route discovery and sends one payload to the last node after startup, so the logs should show `RREQ originated`, `RREQ forwarded`, `RREP generated`, `route cached`, `DATA forwarded`, and `DATA delivered`.

Manual local demo with four terminals:

```bash
cargo run -p dldsr-node -- --config examples/node0.toml
cargo run -p dldsr-node -- --config examples/node1.toml
cargo run -p dldsr-node -- --config examples/node2.toml
cargo run -p dldsr-node -- --config examples/node3.toml
```

Then trigger node 0 at runtime:

```bash
cargo run -p dldsr-node -- send \
  --control-addr 127.0.0.1:9000 \
  --dst 3 \
  --payload "hello dldsr"
```

The helper sends JSON to node 0's configured `control_bind_addr`. The control path uses the same route discovery, route cache, and forwarding code as startup/demo sends.

Equivalent Python trigger:

```bash
python3 - <<'PY'
import socket
sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
sock.sendto(b'{"dst":3,"payload":"hello dldsr"}', ("127.0.0.1", 9000))
print(sock.recvfrom(1024)[0].decode())
PY
```

## Metrics Output

If `metrics_output_path` is set, the daemon periodically writes a JSON snapshot using a temporary file and rename:

```bash
cat /tmp/dldsr-node0-metrics.json | jq
```

## Current Protocol Completeness

Live daemon behavior:

- Hello/HelloReply dynamic label exchange.
- RREQ/RREP route discovery over configured logical neighbors.
- Source route cache and queued payload flush after route discovery.
- DSR forwarding by node-id path.
- DL-DSR forwarding by 8-bit labels resolved through the Neighbor Table.
- Runtime control sends through the same live data path.
- Delivery logs and test events.

Robustness now implemented:

- Route discovery timeout, retry, and failure events.
- RREQ max-hop guard.
- Expiring and bounded duplicate RREQ cache.
- RREP trace/sender adjacency validation.
- DATA sender, protocol mode, cursor, path, and label validation.
- Internal route error/data drop events.
- Live metrics counters and optional JSON output for bytes, delivery/drop, route discovery, RREQ/RREP, DATA forwarding, and average latency.
- Docker 4-node DL-DSR demo with live route discovery and delivery.

Future work:

- HTTP metrics endpoint or Prometheus exporter.
- Add ACK-based delivery confirmation and packet retransmission.
- Implement full route maintenance and upstream Packet::Error propagation.
- Add cryptographic authentication for control and protocol packets.
- Add mobility and real wireless PHY/MAC simulation.

## Known Limits

- The live daemon implements Hello/HelloReply, RREQ/RREP route discovery, route caching, and multi-hop Data forwarding for DSR and DL-DSR.
- Packet::Error is still future work; route failures are currently internal events/logs/metrics.
- There is no cryptographic authentication.
- There is no mobility model or real wireless PHY/MAC simulation.
- Docker uses a bridge network, and logical topology enforcement is done by the daemon configuration.
- eBPF/XDP and io_uring are prototype scaffolds for future fast-path work.
