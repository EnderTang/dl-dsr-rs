# eBPF/XDP Prototype

This directory is a prototype scaffold. It is intentionally outside the Cargo workspace so normal Rust builds do not require kernel headers, clang, bpftool, or root privileges.

The idea is to parse the DL-DSR packet prefix in XDP and later redirect packets by label. The current daemon remains the source of truth for protocol behavior.

On Ubuntu 24.04:

```bash
sudo apt-get update
sudo apt-get install -y clang llvm libbpf-dev bpftool linux-headers-$(uname -r)
clang -O2 -g -target bpf -c ebpf/xdp_dldsr_kern.c -o target/xdp_dldsr_kern.o
```

