use crate::energy::EnergyEstimate;
use crate::packet::{path_header_bytes, ProtocolMode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeMetrics {
    pub tx_packets: u64,
    pub rx_packets: u64,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
    pub control_bytes: u64,
    pub data_bytes: u64,
    pub delivered_packets: u64,
    pub dropped_packets: u64,
    pub latency_samples_ms: Vec<f64>,
    pub route_discovery_attempts: u64,
    pub route_discovery_successes: u64,
    pub route_discovery_failures: u64,
    pub rreq_forwarded: u64,
    pub rreq_dropped_duplicate: u64,
    pub rreq_dropped_ttl: u64,
    pub rrep_forwarded: u64,
    pub rrep_dropped_invalid: u64,
    pub data_forwarded: u64,
    pub data_delivered: u64,
    pub data_dropped: u64,
}

impl NodeMetrics {
    pub fn record_tx(&mut self, bytes: usize, control: bool) {
        self.tx_packets += 1;
        self.tx_bytes += bytes as u64;
        if control {
            self.control_bytes += bytes as u64;
        } else {
            self.data_bytes += bytes as u64;
        }
    }

    pub fn record_rx(&mut self, bytes: usize) {
        self.rx_packets += 1;
        self.rx_bytes += bytes as u64;
    }

    pub fn energy(&self) -> EnergyEstimate {
        EnergyEstimate::from_bytes(self.tx_bytes, self.rx_bytes)
    }

    pub fn average_latency_ms(&self) -> f64 {
        if self.latency_samples_ms.is_empty() {
            0.0
        } else {
            self.latency_samples_ms.iter().sum::<f64>() / self.latency_samples_ms.len() as f64
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub mode: ProtocolMode,
    pub hops: usize,
    pub payload_size: usize,
    pub packet_count: u64,
    pub total_tx_bytes: u64,
    pub total_rx_bytes: u64,
    pub control_bytes: u64,
    pub data_bytes: u64,
    pub path_header_bytes: u64,
    pub delivered_packets: u64,
    pub lost_packets: u64,
    pub delivery_ratio: f64,
    pub average_latency_ms: f64,
    pub p50_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub estimated_energy_units: f64,
}

pub fn simulate_chain(
    hops: usize,
    mode: ProtocolMode,
    payload_size: usize,
    packet_count: u64,
) -> BenchmarkReport {
    let path_bytes = path_header_bytes(mode, hops) as u64;
    let serialized_overhead = 32_u64;
    let per_packet = payload_size as u64 + path_bytes + serialized_overhead;
    let link_transmissions = hops.saturating_sub(1).max(1) as u64;
    let total_tx_bytes = per_packet * packet_count * link_transmissions;
    let total_rx_bytes = per_packet * packet_count * link_transmissions;
    let latencies: Vec<f64> = (0..packet_count)
        .map(|i| hops as f64 * 0.8 + payload_size as f64 * 0.002 + (i % 5) as f64 * 0.05)
        .collect();
    let energy = EnergyEstimate::from_bytes(total_tx_bytes, total_rx_bytes);

    BenchmarkReport {
        mode,
        hops,
        payload_size,
        packet_count,
        total_tx_bytes,
        total_rx_bytes,
        control_bytes: 0,
        data_bytes: total_tx_bytes,
        path_header_bytes: path_bytes * packet_count,
        delivered_packets: packet_count,
        lost_packets: 0,
        delivery_ratio: 1.0,
        average_latency_ms: mean(&latencies),
        p50_latency_ms: percentile(latencies.clone(), 50.0),
        p95_latency_ms: percentile(latencies, 95.0),
        estimated_energy_units: energy.total_energy_units,
    }
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn percentile(mut values: Vec<f64>, percentile: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((percentile / 100.0) * (values.len().saturating_sub(1)) as f64).round() as usize;
    values[idx]
}
