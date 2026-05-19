use crate::packet::ProtocolMode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topology {
    pub nodes: Vec<TopologyNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyNode {
    pub node_id: u32,
    pub endpoint: SocketAddr,
    pub neighbors: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub node_id: u32,
    pub bind_addr: SocketAddr,
    pub protocol_mode: ProtocolMode,
    #[serde(default = "default_hello_interval_ms")]
    pub hello_interval_ms: u64,
    #[serde(default = "default_route_cache_ttl_ms")]
    pub route_cache_ttl_ms: u64,
    #[serde(default = "default_neighbor_ttl_ms")]
    pub neighbor_ttl_ms: u64,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    pub topology: Topology,
    #[serde(default)]
    pub demo_send: Option<DemoSendConfig>,
    #[serde(default = "default_route_discovery_timeout_ms")]
    pub route_discovery_timeout_ms: u64,
    #[serde(default = "default_route_discovery_max_retries")]
    pub route_discovery_max_retries: u32,
    #[serde(default = "default_max_rreq_hops")]
    pub max_rreq_hops: u8,
    #[serde(default = "default_seen_request_ttl_ms")]
    pub seen_request_ttl_ms: u64,
    #[serde(default = "default_seen_request_max_entries")]
    pub seen_request_max_entries: usize,
    #[serde(default)]
    pub metrics_output_path: Option<PathBuf>,
    #[serde(default)]
    pub control_bind_addr: Option<SocketAddr>,
    #[serde(default = "default_strict_protocol_mode")]
    pub strict_protocol_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemoSendConfig {
    pub dst_node_id: u32,
    pub payload: String,
    pub start_after_ms: u64,
}

pub fn default_hello_interval_ms() -> u64 {
    800
}

pub fn default_route_cache_ttl_ms() -> u64 {
    30_000
}

pub fn default_neighbor_ttl_ms() -> u64 {
    5_000
}

pub fn default_log_level() -> String {
    "info".to_string()
}

pub fn default_route_discovery_timeout_ms() -> u64 {
    2_000
}

pub fn default_route_discovery_max_retries() -> u32 {
    3
}

pub fn default_max_rreq_hops() -> u8 {
    64
}

pub fn default_seen_request_ttl_ms() -> u64 {
    60_000
}

pub fn default_seen_request_max_entries() -> usize {
    10_000
}

pub fn default_strict_protocol_mode() -> bool {
    true
}

impl Topology {
    pub fn chain(nodes: usize, base_port: u16) -> Self {
        let nodes = (0..nodes)
            .map(|idx| {
                let mut neighbors = Vec::new();
                if idx > 0 {
                    neighbors.push((idx - 1) as u32);
                }
                if idx + 1 < nodes {
                    neighbors.push((idx + 1) as u32);
                }
                TopologyNode {
                    node_id: idx as u32,
                    endpoint: format!("127.0.0.1:{}", base_port + idx as u16)
                        .parse()
                        .unwrap(),
                    neighbors,
                }
            })
            .collect();
        Self { nodes }
    }

    pub fn node(&self, node_id: u32) -> Option<&TopologyNode> {
        self.nodes.iter().find(|node| node.node_id == node_id)
    }

    pub fn endpoints(&self) -> HashMap<u32, SocketAddr> {
        self.nodes
            .iter()
            .map(|node| (node.node_id, node.endpoint))
            .collect()
    }
}
