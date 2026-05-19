use dldsr_core::BenchmarkReport;

use crate::ComparisonReport;

pub fn markdown(report: &BenchmarkReport) -> String {
    format!(
        r#"# DL-DSR Benchmark Summary

| metric | value |
| --- | ---: |
| mode | {} |
| hops | {} |
| payload_size | {} |
| packets | {} |
| total_tx_bytes | {} |
| total_rx_bytes | {} |
| path_header_bytes | {} |
| delivered_packets | {} |
| lost_packets | {} |
| delivery_ratio | {:.3} |
| average_latency_ms | {:.3} |
| p50_latency_ms | {:.3} |
| p95_latency_ms | {:.3} |
| estimated_energy_units | {:.1} |

Energy is a transparent proxy: tx_bytes * 1.0 + rx_bytes * 0.5.
"#,
        report.mode,
        report.hops,
        report.payload_size,
        report.packet_count,
        report.total_tx_bytes,
        report.total_rx_bytes,
        report.path_header_bytes,
        report.delivered_packets,
        report.lost_packets,
        report.delivery_ratio,
        report.average_latency_ms,
        report.p50_latency_ms,
        report.p95_latency_ms,
        report.estimated_energy_units
    )
}

pub fn comparison_markdown(report: &ComparisonReport) -> String {
    format!(
        r#"# DSR vs DL-DSR Benchmark Summary

| metric | DSR | DL-DSR |
| --- | ---: | ---: |
| hops | {hops} | {hops} |
| payload_size | {payload_size} | {payload_size} |
| packets | {packets} | {packets} |
| total_tx_bytes | {dsr_tx} | {dldsr_tx} |
| total_rx_bytes | {dsr_rx} | {dldsr_rx} |
| path_header_bytes | {dsr_header} | {dldsr_header} |
| delivered_packets | {dsr_delivered} | {dldsr_delivered} |
| delivery_ratio | {dsr_ratio:.3} | {dldsr_ratio:.3} |
| average_latency_ms | {dsr_avg:.3} | {dldsr_avg:.3} |
| p95_latency_ms | {dsr_p95:.3} | {dldsr_p95:.3} |
| estimated_energy_units | {dsr_energy:.1} | {dldsr_energy:.1} |

| savings | value |
| --- | ---: |
| path_header_savings_bytes | {header_savings} |
| total_tx_savings_bytes | {tx_savings} |
| estimated_energy_savings_units | {energy_savings:.1} |

Energy is a transparent proxy: tx_bytes * 1.0 + rx_bytes * 0.5.
"#,
        hops = report.dsr.hops,
        payload_size = report.dsr.payload_size,
        packets = report.dsr.packet_count,
        dsr_tx = report.dsr.total_tx_bytes,
        dldsr_tx = report.dldsr.total_tx_bytes,
        dsr_rx = report.dsr.total_rx_bytes,
        dldsr_rx = report.dldsr.total_rx_bytes,
        dsr_header = report.dsr.path_header_bytes,
        dldsr_header = report.dldsr.path_header_bytes,
        dsr_delivered = report.dsr.delivered_packets,
        dldsr_delivered = report.dldsr.delivered_packets,
        dsr_ratio = report.dsr.delivery_ratio,
        dldsr_ratio = report.dldsr.delivery_ratio,
        dsr_avg = report.dsr.average_latency_ms,
        dldsr_avg = report.dldsr.average_latency_ms,
        dsr_p95 = report.dsr.p95_latency_ms,
        dldsr_p95 = report.dldsr.p95_latency_ms,
        dsr_energy = report.dsr.estimated_energy_units,
        dldsr_energy = report.dldsr.estimated_energy_units,
        header_savings = report.path_header_savings_bytes,
        tx_savings = report.total_tx_savings_bytes,
        energy_savings = report.estimated_energy_savings_units
    )
}
