# Resume Bullets

- Built a Rust-based distributed routing engine implementing DSR and dynamic-label source routing over UDP.
- Reduced source-routing path overhead by encoding multi-hop routes with 8-bit dynamic labels instead of 32-bit node identifiers.
- Implemented Docker-based multi-node chain topologies and benchmarked 4/8/16/32-hop routes with 64B and 512B payloads.
- Prototyped an eBPF/XDP fast path for label-based packet parsing and forwarding.
- Added an experimental io_uring crate to explore batched Linux I/O for future high-throughput packet processing.

