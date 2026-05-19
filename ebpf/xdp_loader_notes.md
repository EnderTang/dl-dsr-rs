# XDP Loader Notes

Future loader options:

- Use `libbpf-rs` or Aya from a separate crate.
- Attach to a Docker bridge or veth device on a lab host.
- Keep userspace topology and label state synchronized through a BPF map.

This is not enabled in the main daemon because the project should run without kernel-specific setup.

