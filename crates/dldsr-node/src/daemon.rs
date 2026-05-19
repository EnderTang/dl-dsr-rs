use crate::protocol::{now_millis, ProtocolActions, ProtocolCommand, ProtocolEvent, ProtocolState};
use crate::udp_transport::UdpTransport;
use anyhow::Result;
use dldsr_core::{NodeConfig, Packet, ProtocolMode};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::time::{interval, sleep, Duration};
use tracing::{info, warn};

pub struct Daemon {
    state: ProtocolState,
}

impl Daemon {
    pub fn new(config: NodeConfig) -> Self {
        Self {
            state: ProtocolState::new(config),
        }
    }

    pub async fn run(self) -> Result<()> {
        self.run_with_channels(None, None).await
    }

    pub async fn run_with_channels(
        mut self,
        mut commands: Option<mpsc::Receiver<ProtocolCommand>>,
        events: Option<mpsc::Sender<ProtocolEvent>>,
    ) -> Result<()> {
        let transport = UdpTransport::bind(self.state.config.bind_addr).await?;
        info!(
            node_id = self.state.config.node_id,
            bind = %self.state.config.bind_addr,
            mode = %self.state.config.protocol_mode,
            "node started"
        );

        let mut hello_tick = interval(Duration::from_millis(self.state.config.hello_interval_ms));
        let mut protocol_tick = interval(Duration::from_millis(100));
        let mut metrics_tick = interval(Duration::from_secs(5));
        let mut buf = vec![0_u8; 65_536];
        let mut control_buf = vec![0_u8; 4096];
        let mut control_socket =
            if let Some(control_bind_addr) = self.state.config.control_bind_addr {
                let socket = UdpSocket::bind(control_bind_addr).await?;
                info!(
                    node_id = self.state.config.node_id,
                    bind = %control_bind_addr,
                    "control socket started"
                );
                Some(socket)
            } else {
                None
            };

        if let Some(demo) = self.state.config.demo_send.clone() {
            let node_id = self.state.config.node_id;
            let dst_node_id = demo.dst_node_id;
            let payload = demo.payload.into_bytes();
            let delay = Duration::from_millis(demo.start_after_ms);
            let (tx, rx) = mpsc::channel(8);
            if commands.is_none() {
                commands = Some(rx);
                tokio::spawn(async move {
                    sleep(delay).await;
                    let _ = tx
                        .send(ProtocolCommand::DiscoverAndSend {
                            dst_node_id,
                            payload,
                        })
                        .await;
                    info!(node_id, dst_node_id, "demo sender triggered");
                });
            } else {
                warn!(
                    node_id,
                    "demo sender ignored because an external command channel is already attached"
                );
            }
        }

        loop {
            tokio::select! {
                _ = hello_tick.tick() => {
                    let actions = self.send_hellos();
                    self.apply_actions(actions, &transport, events.as_ref()).await;
                }
                _ = protocol_tick.tick() => {
                    let actions = self.state.handle_tick(now_millis());
                    self.apply_actions(actions, &transport, events.as_ref()).await;
                }
                _ = metrics_tick.tick() => {
                    self.log_metrics();
                    if let Err(err) = self.write_metrics_file().await {
                        warn!(?err, "metrics file write failed");
                    }
                }
                control = recv_control(&mut control_socket, &mut control_buf), if control_socket.is_some() => {
                    match control {
                        Some(Ok((len, sender))) => {
                            let actions = self.handle_control_message(&control_buf[..len], sender, control_socket.as_ref()).await;
                            self.apply_actions(actions, &transport, events.as_ref()).await;
                        }
                        Some(Err(err)) => warn!(?err, "control receive failed"),
                        None => {}
                    }
                }
                received = transport.recv(&mut buf) => {
                    match received {
                        Ok((packet, sender, len)) => {
                            self.state.record_rx(len);
                            let actions = self.state.handle_packet(packet, sender);
                            self.apply_actions(actions, &transport, events.as_ref()).await;
                        }
                        Err(err) => warn!(?err, "packet receive failed"),
                    }
                }
                command = recv_command(&mut commands), if commands.is_some() => {
                    if let Some(command) = command {
                        let actions = self.state.handle_command(command);
                        self.apply_actions(actions, &transport, events.as_ref()).await;
                    }
                }
                signal = tokio::signal::ctrl_c() => {
                    if let Err(err) = signal {
                        warn!(?err, "shutdown signal listener failed");
                    }
                    self.log_metrics();
                    if let Err(err) = self.write_metrics_file().await {
                        warn!(?err, "metrics file write failed during shutdown");
                    }
                    return Ok(());
                }
            }
        }
    }

    fn send_hellos(&self) -> ProtocolActions {
        let mut actions = ProtocolActions::default();
        for neighbor_id in self.logical_neighbors() {
            if let Some(node) = self.state.config.topology.node(neighbor_id) {
                let packet = Packet::Hello {
                    src_node_id: self.state.config.node_id,
                    timestamp_millis: now_millis(),
                };
                actions.send(packet, node.endpoint);
                info!(
                    node_id = self.state.config.node_id,
                    neighbor_id, "HELLO sent"
                );
            }
        }
        actions
    }

    async fn apply_actions(
        &mut self,
        actions: ProtocolActions,
        transport: &UdpTransport,
        events: Option<&mpsc::Sender<ProtocolEvent>>,
    ) {
        for outbound in actions.outbound {
            match transport.send(&outbound.packet, outbound.dst).await {
                Ok(_) => self.state.record_tx(&outbound.packet),
                Err(err) => {
                    self.state.metrics.dropped_packets += 1;
                    warn!(?err, dst = %outbound.dst, "packet send failed");
                }
            }
        }
        if let Some(events) = events {
            for event in actions.events {
                if events.send(event).await.is_err() {
                    warn!("protocol event receiver dropped");
                }
            }
        }
    }

    fn logical_neighbors(&self) -> Vec<u32> {
        self.state
            .config
            .topology
            .node(self.state.config.node_id)
            .map(|node| node.neighbors.clone())
            .unwrap_or_default()
    }

    fn log_metrics(&self) {
        let metrics = self.state.metrics();
        info!(
            node_id = self.state.config.node_id,
            tx_bytes = metrics.tx_bytes,
            rx_bytes = metrics.rx_bytes,
            control_bytes = metrics.control_bytes,
            data_bytes = metrics.data_bytes,
            delivered_packets = metrics.delivered_packets,
            dropped_packets = metrics.dropped_packets,
            route_discovery_attempts = metrics.route_discovery_attempts,
            route_discovery_successes = metrics.route_discovery_successes,
            route_discovery_failures = metrics.route_discovery_failures,
            rreq_forwarded = metrics.rreq_forwarded,
            rreq_dropped_duplicate = metrics.rreq_dropped_duplicate,
            rreq_dropped_ttl = metrics.rreq_dropped_ttl,
            rrep_forwarded = metrics.rrep_forwarded,
            rrep_dropped_invalid = metrics.rrep_dropped_invalid,
            data_forwarded = metrics.data_forwarded,
            data_delivered = metrics.data_delivered,
            data_dropped = metrics.data_dropped,
            average_latency_ms = metrics.average_latency_ms(),
            "node metrics"
        );
    }

    async fn handle_control_message(
        &mut self,
        bytes: &[u8],
        sender: std::net::SocketAddr,
        control_socket: Option<&UdpSocket>,
    ) -> ProtocolActions {
        match serde_json::from_slice::<ControlSendRequest>(bytes) {
            Ok(request) => {
                info!(
                    node_id = self.state.config.node_id,
                    dst_node_id = request.dst,
                    payload_len = request.payload.len(),
                    "control send request accepted"
                );
                if let Some(socket) = control_socket {
                    let _ = socket.send_to(br#"{"status":"accepted"}"#, sender).await;
                }
                self.state.handle_command(ProtocolCommand::DiscoverAndSend {
                    dst_node_id: request.dst,
                    payload: request.payload.into_bytes(),
                })
            }
            Err(err) => {
                warn!(
                    ?err,
                    node_id = self.state.config.node_id,
                    "control parse failed"
                );
                if let Some(socket) = control_socket {
                    let _ = socket
                        .send_to(
                            format!(r#"{{"status":"error","reason":"{}"}}"#, err).as_bytes(),
                            sender,
                        )
                        .await;
                }
                ProtocolActions::default()
            }
        }
    }

    async fn write_metrics_file(&self) -> Result<()> {
        let Some(path) = self.state.config.metrics_output_path.as_deref() else {
            return Ok(());
        };
        write_metrics_snapshot(path, &self.metrics_snapshot()).await
    }

    fn metrics_snapshot(&self) -> MetricsSnapshot {
        let metrics = self.state.metrics();
        MetricsSnapshot {
            node_id: self.state.config.node_id,
            protocol_mode: self.state.config.protocol_mode,
            tx_bytes: metrics.tx_bytes,
            rx_bytes: metrics.rx_bytes,
            control_bytes: metrics.control_bytes,
            data_bytes: metrics.data_bytes,
            delivered_packets: metrics.delivered_packets,
            dropped_packets: metrics.dropped_packets,
            route_discovery_attempts: metrics.route_discovery_attempts,
            route_discovery_successes: metrics.route_discovery_successes,
            route_discovery_failures: metrics.route_discovery_failures,
            rreq_forwarded: metrics.rreq_forwarded,
            rreq_dropped_duplicate: metrics.rreq_dropped_duplicate,
            rreq_dropped_ttl: metrics.rreq_dropped_ttl,
            rrep_forwarded: metrics.rrep_forwarded,
            rrep_dropped_invalid: metrics.rrep_dropped_invalid,
            data_forwarded: metrics.data_forwarded,
            data_delivered: metrics.data_delivered,
            data_dropped: metrics.data_dropped,
            average_latency_ms: metrics.average_latency_ms(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ControlSendRequest {
    dst: u32,
    payload: String,
}

#[derive(Debug, Serialize)]
struct MetricsSnapshot {
    node_id: u32,
    protocol_mode: ProtocolMode,
    tx_bytes: u64,
    rx_bytes: u64,
    control_bytes: u64,
    data_bytes: u64,
    delivered_packets: u64,
    dropped_packets: u64,
    route_discovery_attempts: u64,
    route_discovery_successes: u64,
    route_discovery_failures: u64,
    rreq_forwarded: u64,
    rreq_dropped_duplicate: u64,
    rreq_dropped_ttl: u64,
    rrep_forwarded: u64,
    rrep_dropped_invalid: u64,
    data_forwarded: u64,
    data_delivered: u64,
    data_dropped: u64,
    average_latency_ms: f64,
}

async fn write_metrics_snapshot(path: &Path, snapshot: &MetricsSnapshot) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp_path = path.with_extension("tmp");
    let bytes = serde_json::to_vec_pretty(snapshot)?;
    tokio::fs::write(&tmp_path, bytes).await?;
    tokio::fs::rename(&tmp_path, path).await?;
    Ok(())
}

async fn recv_command(
    commands: &mut Option<mpsc::Receiver<ProtocolCommand>>,
) -> Option<ProtocolCommand> {
    match commands {
        Some(commands) => commands.recv().await,
        None => None,
    }
}

async fn recv_control(
    socket: &mut Option<UdpSocket>,
    buf: &mut [u8],
) -> Option<std::io::Result<(usize, std::net::SocketAddr)>> {
    match socket {
        Some(socket) => Some(socket.recv_from(buf).await),
        None => None,
    }
}
