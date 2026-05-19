# Benchmark Design

The benchmark uses a deterministic chain model so DSR and DL-DSR can be compared without noise from container scheduling or UDP timing.

Metrics:

- Network load: total transmitted and received bytes across all logical links.
- Path overhead: route header bytes, using 4 bytes per DSR hop and 1 byte per DL-DSR hop.
- End-to-end latency: deterministic estimate based on hop count, payload size, and a small sequence jitter.
- Energy proxy: `tx_bytes * 1.0 + rx_bytes * 0.5`.
- Delivery success: deterministic delivery count and delivery ratio.

Expected behavior:

- DL-DSR should have lower path header bytes for the same hop count.
- Savings grow with longer paths and more packets.
- Payload-heavy traffic reduces the percentage impact of header compression, but absolute header savings remain.

