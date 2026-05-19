use crate::cli::Cli;
use anyhow::{Context, Result};
use dldsr_core::{
    default_hello_interval_ms, default_max_rreq_hops, default_neighbor_ttl_ms,
    default_route_cache_ttl_ms, default_route_discovery_max_retries,
    default_route_discovery_timeout_ms, default_seen_request_max_entries,
    default_seen_request_ttl_ms, default_strict_protocol_mode, NodeConfig, ProtocolMode, Topology,
};
use std::fs;

pub fn load(cli: Cli) -> Result<NodeConfig> {
    if let Some(path) = cli.config {
        let text =
            fs::read_to_string(&path).with_context(|| format!("read config {}", path.display()))?;
        return toml::from_str(&text).with_context(|| format!("parse config {}", path.display()));
    }

    let topology_path = cli
        .topology
        .context("--topology is required when --config is omitted")?;
    let text = fs::read_to_string(&topology_path)
        .with_context(|| format!("read topology {}", topology_path.display()))?;
    let topology: Topology = toml::from_str(&text)
        .with_context(|| format!("parse topology {}", topology_path.display()))?;
    let node_id = cli
        .node_id
        .context("--node-id is required when --config is omitted")?;
    let bind_addr = cli
        .bind
        .or_else(|| topology.node(node_id).map(|node| node.endpoint))
        .context("bind address not supplied and node id not found in topology")?;

    Ok(NodeConfig {
        node_id,
        bind_addr,
        protocol_mode: cli.mode.unwrap_or(ProtocolMode::DlDsr),
        hello_interval_ms: default_hello_interval_ms(),
        route_cache_ttl_ms: default_route_cache_ttl_ms(),
        neighbor_ttl_ms: default_neighbor_ttl_ms(),
        log_level: "info".to_string(),
        topology,
        demo_send: cli.send_dst.map(|dst_node_id| dldsr_core::DemoSendConfig {
            dst_node_id,
            payload: cli
                .payload
                .unwrap_or_else(|| "hello from dl-dsr-rs".to_string()),
            start_after_ms: cli.send_after_ms,
        }),
        route_discovery_timeout_ms: default_route_discovery_timeout_ms(),
        route_discovery_max_retries: default_route_discovery_max_retries(),
        max_rreq_hops: default_max_rreq_hops(),
        seen_request_ttl_ms: default_seen_request_ttl_ms(),
        seen_request_max_entries: default_seen_request_max_entries(),
        metrics_output_path: None,
        control_bind_addr: None,
        strict_protocol_mode: default_strict_protocol_mode(),
    })
}
