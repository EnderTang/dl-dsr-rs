# Architecture

```mermaid
flowchart LR
  Bench[dldsr-bench] --> Core[dldsr-core]
  Node[dldsr-node] --> Core
  Node --> UDP[Tokio UDP Socket]
  UDP --> N1[Logical Neighbor]
  Core --> Packets[Packet Encoding]
  Core --> Tables[Neighbor and Label Tables]
  Core --> Routes[Route Cache]
  Uring[dldsr-uring prototype] -. future fast I/O .-> UDP
  XDP[eBPF/XDP prototype] -. future kernel fast path .-> Packets
```

`dldsr-core` owns protocol data structures, metrics, topology parsing, and deterministic benchmark modeling. `dldsr-node` is the async UDP daemon. Docker Compose starts multiple node processes from one image, while each daemon only contacts logical neighbors from the topology.

