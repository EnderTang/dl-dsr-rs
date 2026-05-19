use dldsr_core::{
    LabelTable, NeighborTable, NodeConfig, NodeMetrics, Packet, ProtocolMode, RouteCache,
    RouteEntry, SeenRequests,
};
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub enum ProtocolCommand {
    DiscoverAndSend { dst_node_id: u32, payload: Vec<u8> },
}

#[derive(Debug, Clone)]
pub enum ProtocolEvent {
    RouteCached {
        node_id: u32,
        dst_node_id: u32,
        request_id: u64,
    },
    DataDelivered {
        node_id: u32,
        src_node_id: u32,
        dst_node_id: u32,
        sequence: u64,
        payload: Vec<u8>,
    },
    RouteDiscoveryFailed {
        node_id: u32,
        dst_node_id: u32,
        attempts: u32,
    },
    DataDropped {
        node_id: u32,
        dst_node_id: u32,
        reason: String,
    },
    RouteError {
        node_id: u32,
        dst_node_id: u32,
        reason: String,
    },
}

#[derive(Debug, Clone)]
pub struct OutboundPacket {
    pub packet: Packet,
    pub dst: SocketAddr,
}

#[derive(Debug, Default)]
pub struct ProtocolActions {
    pub outbound: Vec<OutboundPacket>,
    pub events: Vec<ProtocolEvent>,
}

impl ProtocolActions {
    pub fn send(&mut self, packet: Packet, dst: SocketAddr) {
        self.outbound.push(OutboundPacket { packet, dst });
    }
}

#[derive(Debug)]
struct PendingData {
    dst_node_id: u32,
    payload: Vec<u8>,
}

#[derive(Debug, Clone)]
struct DiscoveryState {
    request_id: u64,
    last_sent_millis: u64,
    retries_done: u32,
}

#[derive(Debug)]
pub struct ProtocolState {
    pub config: NodeConfig,
    pub neighbors: NeighborTable,
    pub labels: LabelTable,
    pub routes: RouteCache,
    pub metrics: NodeMetrics,
    seen_requests: SeenRequests,
    pending_data: VecDeque<PendingData>,
    discoveries: HashMap<u32, DiscoveryState>,
    next_request_id: u64,
    next_sequence: u64,
}

impl ProtocolState {
    pub fn new(config: NodeConfig) -> Self {
        Self {
            config,
            neighbors: NeighborTable::default(),
            labels: LabelTable::default(),
            routes: RouteCache::default(),
            metrics: NodeMetrics::default(),
            seen_requests: SeenRequests::default(),
            pending_data: VecDeque::new(),
            discoveries: HashMap::new(),
            next_request_id: 1,
            next_sequence: 1,
        }
    }

    pub fn metrics(&self) -> &NodeMetrics {
        &self.metrics
    }

    pub fn record_rx(&mut self, bytes: usize) {
        self.metrics.record_rx(bytes);
    }

    pub fn record_tx(&mut self, packet: &Packet) {
        if let Ok(bytes) = packet.encoded_len() {
            self.metrics.record_tx(bytes, is_control_packet(packet));
        }
    }

    pub fn handle_tick(&mut self, now: u64) -> ProtocolActions {
        self.seen_requests
            .prune(now, self.config.seen_request_ttl_ms);
        self.retry_expired_discoveries(now)
    }

    pub fn handle_command(&mut self, command: ProtocolCommand) -> ProtocolActions {
        match command {
            ProtocolCommand::DiscoverAndSend {
                dst_node_id,
                payload,
            } => self.discover_and_send(dst_node_id, payload),
        }
    }

    pub fn handle_packet(&mut self, packet: Packet, sender: SocketAddr) -> ProtocolActions {
        match packet {
            Packet::Hello {
                src_node_id,
                timestamp_millis,
            } => self.handle_hello(src_node_id, timestamp_millis, sender),
            Packet::HelloReply {
                src_node_id,
                dst_node_id,
                assigned_label,
                timestamp_millis,
                ..
            } => {
                self.handle_hello_reply(src_node_id, dst_node_id, assigned_label, timestamp_millis)
            }
            Packet::Rreq {
                src_node_id,
                dst_node_id,
                request_id,
                previous_hop,
                hop_count,
                dsr_path,
                dldsr_label_path,
                trace_path,
                created_at_millis,
            } => self.handle_rreq(
                src_node_id,
                dst_node_id,
                request_id,
                previous_hop,
                hop_count,
                dsr_path,
                dldsr_label_path,
                trace_path,
                created_at_millis,
            ),
            Packet::Rrep {
                src_node_id,
                dst_node_id,
                request_id,
                dsr_path,
                dldsr_label_path,
                trace_path,
                created_at_millis,
            } => self.handle_rrep(
                sender,
                src_node_id,
                dst_node_id,
                request_id,
                dsr_path,
                dldsr_label_path,
                trace_path,
                created_at_millis,
            ),
            Packet::Data {
                src_node_id,
                dst_node_id,
                sequence,
                protocol_mode,
                route_cursor,
                dsr_path,
                dldsr_label_path,
                payload,
                created_at_millis,
            } => self.handle_data(
                sender,
                src_node_id,
                dst_node_id,
                sequence,
                protocol_mode,
                route_cursor,
                dsr_path,
                dldsr_label_path,
                payload,
                created_at_millis,
            ),
            packet => {
                debug!(node_id = self.config.node_id, ?packet, "packet ignored");
                ProtocolActions::default()
            }
        }
    }

    fn handle_hello(
        &mut self,
        src_node_id: u32,
        _timestamp_millis: u64,
        sender: SocketAddr,
    ) -> ProtocolActions {
        let mut actions = ProtocolActions::default();
        if !self.is_logical_neighbor(src_node_id) {
            warn!(
                node_id = self.config.node_id,
                src_node_id, "HELLO ignored from non-neighbor"
            );
            return actions;
        }

        let Ok(label) = self
            .neighbors
            .assign_or_reuse_label(src_node_id, sender, now_millis())
        else {
            warn!(
                node_id = self.config.node_id,
                src_node_id, "HELLO label allocation failed"
            );
            return actions;
        };

        info!(
            node_id = self.config.node_id,
            src_node_id, label, "HELLO received"
        );
        actions.send(
            Packet::HelloReply {
                src_node_id: self.config.node_id,
                dst_node_id: src_node_id,
                assigned_label: label,
                update_flag: true,
                timestamp_millis: now_millis(),
            },
            sender,
        );
        info!(
            node_id = self.config.node_id,
            dst_node_id = src_node_id,
            label,
            "HELLO_REPLY sent"
        );
        actions
    }

    fn handle_hello_reply(
        &mut self,
        src_node_id: u32,
        dst_node_id: u32,
        assigned_label: u8,
        timestamp_millis: u64,
    ) -> ProtocolActions {
        if dst_node_id == self.config.node_id && self.is_logical_neighbor(src_node_id) {
            self.labels
                .update_from_hello_reply(src_node_id, assigned_label, timestamp_millis);
            info!(
                node_id = self.config.node_id,
                src_node_id, assigned_label, "HELLO_REPLY received"
            );
        }
        ProtocolActions::default()
    }

    fn discover_and_send(&mut self, dst_node_id: u32, payload: Vec<u8>) -> ProtocolActions {
        let now = now_millis();
        if let Some(route) = self.routes.shortest(dst_node_id, now).cloned() {
            return self.origin_data(dst_node_id, payload, route);
        }

        self.pending_data.push_back(PendingData {
            dst_node_id,
            payload,
        });
        if self.discoveries.contains_key(&dst_node_id) {
            return ProtocolActions::default();
        }
        self.start_discovery(dst_node_id, 0)
    }

    fn start_discovery(&mut self, dst_node_id: u32, retries_done: u32) -> ProtocolActions {
        let mut actions = ProtocolActions::default();
        let request_id = self.next_request_id;
        self.next_request_id += 1;
        let now = now_millis();
        self.seen_requests.mark_if_new(
            self.config.node_id,
            request_id,
            now,
            self.config.seen_request_ttl_ms,
            self.config.seen_request_max_entries,
        );
        self.discoveries.insert(
            dst_node_id,
            DiscoveryState {
                request_id,
                last_sent_millis: now,
                retries_done,
            },
        );
        self.metrics.route_discovery_attempts += 1;

        info!(
            node_id = self.config.node_id,
            dst_node_id, request_id, retries_done, "RREQ originated"
        );

        for neighbor_id in self.logical_neighbors() {
            if let Some(endpoint) = self.endpoint(neighbor_id) {
                actions.send(
                    Packet::Rreq {
                        src_node_id: self.config.node_id,
                        dst_node_id,
                        request_id,
                        previous_hop: self.config.node_id,
                        hop_count: 0,
                        dsr_path: vec![self.config.node_id],
                        dldsr_label_path: Vec::new(),
                        trace_path: vec![self.config.node_id],
                        created_at_millis: now,
                    },
                    endpoint,
                );
            }
        }
        actions
    }

    fn retry_expired_discoveries(&mut self, now: u64) -> ProtocolActions {
        let mut actions = ProtocolActions::default();
        let expired: Vec<(u32, DiscoveryState)> = self
            .discoveries
            .iter()
            .filter(|(_, state)| {
                now.saturating_sub(state.last_sent_millis) >= self.config.route_discovery_timeout_ms
            })
            .map(|(dst, state)| (*dst, state.clone()))
            .collect();

        for (dst_node_id, state) in expired {
            if state.retries_done >= self.config.route_discovery_max_retries {
                self.discoveries.remove(&dst_node_id);
                let dropped = self.drop_pending_for_dst(dst_node_id);
                self.metrics.route_discovery_failures += 1;
                self.metrics.dropped_packets += dropped as u64;
                self.metrics.data_dropped += dropped as u64;
                warn!(
                    node_id = self.config.node_id,
                    dst_node_id,
                    request_id = state.request_id,
                    attempts = state.retries_done + 1,
                    dropped,
                    "route discovery failed"
                );
                actions.events.push(ProtocolEvent::RouteDiscoveryFailed {
                    node_id: self.config.node_id,
                    dst_node_id,
                    attempts: state.retries_done + 1,
                });
                if dropped > 0 {
                    actions.events.push(ProtocolEvent::DataDropped {
                        node_id: self.config.node_id,
                        dst_node_id,
                        reason: "route discovery failed".to_string(),
                    });
                }
            } else {
                actions.outbound.extend(
                    self.start_discovery(dst_node_id, state.retries_done + 1)
                        .outbound,
                );
            }
        }
        actions
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_rreq(
        &mut self,
        src_node_id: u32,
        dst_node_id: u32,
        request_id: u64,
        previous_hop: u32,
        hop_count: u8,
        mut dsr_path: Vec<u32>,
        mut dldsr_label_path: Vec<u8>,
        mut trace_path: Vec<u32>,
        created_at_millis: u64,
    ) -> ProtocolActions {
        let mut actions = ProtocolActions::default();
        if !self.is_logical_neighbor(previous_hop) {
            warn!(
                node_id = self.config.node_id,
                previous_hop, request_id, "RREQ ignored from non-neighbor"
            );
            return actions;
        }
        if hop_count >= self.config.max_rreq_hops {
            self.metrics.rreq_dropped_ttl += 1;
            warn!(
                node_id = self.config.node_id,
                src_node_id,
                request_id,
                hop_count,
                max_rreq_hops = self.config.max_rreq_hops,
                "RREQ dropped by TTL"
            );
            return actions;
        }
        if !self.seen_requests.mark_if_new(
            src_node_id,
            request_id,
            now_millis(),
            self.config.seen_request_ttl_ms,
            self.config.seen_request_max_entries,
        ) {
            self.metrics.rreq_dropped_duplicate += 1;
            info!(
                node_id = self.config.node_id,
                src_node_id, request_id, "duplicate RREQ dropped"
            );
            return actions;
        }

        if dsr_path.last().copied() != Some(self.config.node_id) {
            dsr_path.push(self.config.node_id);
        }
        if trace_path.last().copied() != Some(self.config.node_id) {
            trace_path.push(self.config.node_id);
        }
        if self.config.protocol_mode == ProtocolMode::DlDsr {
            let Some(label) = self.labels.label_assigned_by_neighbor(previous_hop) else {
                self.metrics.dropped_packets += 1;
                warn!(
                    node_id = self.config.node_id,
                    previous_hop, request_id, "RREQ missing reverse label from previous hop"
                );
                return actions;
            };
            dldsr_label_path.push(label);
        }

        if self.config.node_id == dst_node_id {
            info!(
                node_id = self.config.node_id,
                src_node_id,
                request_id,
                hop_count = hop_count + 1,
                "RREP generated"
            );
            let rrep = Packet::Rrep {
                src_node_id,
                dst_node_id,
                request_id,
                dsr_path,
                dldsr_label_path,
                trace_path: trace_path.clone(),
                created_at_millis,
            };
            if let Some(reverse_next) = previous_in_trace(&trace_path, self.config.node_id) {
                if let Some(endpoint) = self.endpoint(reverse_next) {
                    actions.send(rrep, endpoint);
                }
            }
            return actions;
        }

        for neighbor_id in self.logical_neighbors() {
            if neighbor_id == previous_hop {
                continue;
            }
            if let Some(endpoint) = self.endpoint(neighbor_id) {
                self.metrics.rreq_forwarded += 1;
                actions.send(
                    Packet::Rreq {
                        src_node_id,
                        dst_node_id,
                        request_id,
                        previous_hop: self.config.node_id,
                        hop_count: hop_count + 1,
                        dsr_path: dsr_path.clone(),
                        dldsr_label_path: dldsr_label_path.clone(),
                        trace_path: trace_path.clone(),
                        created_at_millis,
                    },
                    endpoint,
                );
                info!(
                    node_id = self.config.node_id,
                    neighbor_id, request_id, "RREQ forwarded"
                );
            }
        }
        actions
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_rrep(
        &mut self,
        sender: SocketAddr,
        src_node_id: u32,
        dst_node_id: u32,
        request_id: u64,
        dsr_path: Vec<u32>,
        dldsr_label_path: Vec<u8>,
        trace_path: Vec<u32>,
        created_at_millis: u64,
    ) -> ProtocolActions {
        let mut actions = ProtocolActions::default();
        if let Err(reason) = self.validate_rrep(sender, &trace_path) {
            self.metrics.rrep_dropped_invalid += 1;
            warn!(
                node_id = self.config.node_id,
                request_id,
                %reason,
                "RREP dropped"
            );
            actions.events.push(ProtocolEvent::RouteError {
                node_id: self.config.node_id,
                dst_node_id,
                reason,
            });
            return actions;
        }

        if self.config.node_id == src_node_id {
            let route = match self.config.protocol_mode {
                ProtocolMode::Dsr => RouteEntry::new_dsr(
                    dst_node_id,
                    dsr_path.clone(),
                    now_millis() + self.config.route_cache_ttl_ms,
                ),
                ProtocolMode::DlDsr => RouteEntry {
                    dst_node_id,
                    dsr_path: dsr_path.clone(),
                    dldsr_label_path: dldsr_label_path.clone(),
                    expires_at_millis: now_millis() + self.config.route_cache_ttl_ms,
                },
            };
            let hop_count = route.hop_count();
            self.routes.insert(route.clone());
            self.discoveries.remove(&dst_node_id);
            self.metrics.route_discovery_successes += 1;
            info!(
                node_id = self.config.node_id,
                dst_node_id,
                request_id,
                hop_count,
                mode = %self.config.protocol_mode,
                "route cached"
            );
            actions.events.push(ProtocolEvent::RouteCached {
                node_id: self.config.node_id,
                dst_node_id,
                request_id,
            });
            actions
                .outbound
                .extend(self.flush_pending(dst_node_id, route));
            return actions;
        }

        if let Some(reverse_next) = previous_in_trace(&trace_path, self.config.node_id) {
            if let Some(endpoint) = self.endpoint(reverse_next) {
                self.metrics.rrep_forwarded += 1;
                actions.send(
                    Packet::Rrep {
                        src_node_id,
                        dst_node_id,
                        request_id,
                        dsr_path,
                        dldsr_label_path,
                        trace_path,
                        created_at_millis,
                    },
                    endpoint,
                );
                info!(
                    node_id = self.config.node_id,
                    reverse_next, request_id, "RREP forwarded"
                );
            }
        } else {
            warn!(
                node_id = self.config.node_id,
                request_id, "RREP reverse path lookup failed"
            );
        }
        actions
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_data(
        &mut self,
        sender: SocketAddr,
        src_node_id: u32,
        dst_node_id: u32,
        sequence: u64,
        protocol_mode: ProtocolMode,
        route_cursor: usize,
        dsr_path: Vec<u32>,
        dldsr_label_path: Vec<u8>,
        payload: Vec<u8>,
        created_at_millis: u64,
    ) -> ProtocolActions {
        let mut actions = ProtocolActions::default();
        if self.config.strict_protocol_mode && protocol_mode != self.config.protocol_mode {
            return self.drop_data(
                dst_node_id,
                format!(
                    "protocol mode mismatch: packet={protocol_mode}, node={}",
                    self.config.protocol_mode
                ),
            );
        }
        let Some(sender_node_id) = self.node_id_by_endpoint(sender) else {
            return self.drop_data(
                dst_node_id,
                format!("DATA sender endpoint {sender} is unknown"),
            );
        };
        if !self.is_logical_neighbor(sender_node_id) {
            return self.drop_data(
                dst_node_id,
                format!("DATA sender node {sender_node_id} is not a logical neighbor"),
            );
        }

        if dst_node_id == self.config.node_id {
            if let Err(reason) = self.validate_data_delivery(
                sender_node_id,
                protocol_mode,
                route_cursor,
                &dsr_path,
                &dldsr_label_path,
            ) {
                return self.drop_data(dst_node_id, reason);
            }
            let latency_ms = now_millis().saturating_sub(created_at_millis);
            self.metrics.delivered_packets += 1;
            self.metrics.data_delivered += 1;
            self.metrics.latency_samples_ms.push(latency_ms as f64);
            info!(
                node_id = self.config.node_id,
                src_node_id,
                dst_node_id,
                sequence,
                payload_len = payload.len(),
                latency_ms,
                mode = %protocol_mode,
                "DATA delivered"
            );
            actions.events.push(ProtocolEvent::DataDelivered {
                node_id: self.config.node_id,
                src_node_id,
                dst_node_id,
                sequence,
                payload,
            });
            return actions;
        }

        match protocol_mode {
            ProtocolMode::Dsr => {
                if dsr_path.get(route_cursor).copied() != Some(self.config.node_id) {
                    warn!(
                        node_id = self.config.node_id,
                        sequence, route_cursor, "DATA DSR cursor mismatch"
                    );
                    return self.drop_data(dst_node_id, "DATA DSR cursor mismatch".to_string());
                }
                let Some(next_node_id) = dsr_path.get(route_cursor + 1).copied() else {
                    warn!(
                        node_id = self.config.node_id,
                        sequence, "DATA DSR route exhausted"
                    );
                    return self.drop_data(dst_node_id, "DATA DSR route exhausted".to_string());
                };
                if !self.is_logical_neighbor(next_node_id) {
                    warn!(
                        node_id = self.config.node_id,
                        next_node_id, sequence, "DATA DSR next hop is not a logical neighbor"
                    );
                    return self.drop_data(
                        dst_node_id,
                        format!("DATA DSR next hop {next_node_id} is not a logical neighbor"),
                    );
                }
                if let Some(endpoint) = self.endpoint(next_node_id) {
                    self.metrics.data_forwarded += 1;
                    actions.send(
                        Packet::Data {
                            src_node_id,
                            dst_node_id,
                            sequence,
                            protocol_mode,
                            route_cursor: route_cursor + 1,
                            dsr_path,
                            dldsr_label_path,
                            payload,
                            created_at_millis,
                        },
                        endpoint,
                    );
                    info!(
                        node_id = self.config.node_id,
                        next_node_id, sequence, "DATA forwarded"
                    );
                }
            }
            ProtocolMode::DlDsr => {
                let Some(next_label) = dldsr_label_path.get(route_cursor).copied() else {
                    warn!(
                        node_id = self.config.node_id,
                        sequence, "DATA DL-DSR label path exhausted"
                    );
                    return self
                        .drop_data(dst_node_id, "DATA DL-DSR label path exhausted".to_string());
                };
                let Some(entry) = self.neighbors.neighbor_by_label(next_label) else {
                    warn!(
                        node_id = self.config.node_id,
                        next_label, sequence, "DATA DL-DSR label lookup failed"
                    );
                    return self.drop_data(
                        dst_node_id,
                        format!("DATA DL-DSR label {next_label} lookup failed"),
                    );
                };
                if !self.is_logical_neighbor(entry.node_id) {
                    warn!(
                        node_id = self.config.node_id,
                        next_node_id = entry.node_id,
                        sequence,
                        "DATA DL-DSR next hop is not a logical neighbor"
                    );
                    return self.drop_data(
                        dst_node_id,
                        format!(
                            "DATA DL-DSR next hop {} is not a logical neighbor",
                            entry.node_id
                        ),
                    );
                }
                self.metrics.data_forwarded += 1;
                actions.send(
                    Packet::Data {
                        src_node_id,
                        dst_node_id,
                        sequence,
                        protocol_mode,
                        route_cursor: route_cursor + 1,
                        dsr_path,
                        dldsr_label_path,
                        payload,
                        created_at_millis,
                    },
                    entry.endpoint,
                );
                info!(
                    node_id = self.config.node_id,
                    next_node_id = entry.node_id,
                    next_label,
                    sequence,
                    "DATA forwarded"
                );
            }
        }
        actions
    }

    fn flush_pending(&mut self, dst_node_id: u32, route: RouteEntry) -> Vec<OutboundPacket> {
        let mut remaining = VecDeque::new();
        let mut outbound = Vec::new();
        while let Some(pending) = self.pending_data.pop_front() {
            if pending.dst_node_id == dst_node_id {
                outbound.extend(
                    self.origin_data(dst_node_id, pending.payload, route.clone())
                        .outbound,
                );
            } else {
                remaining.push_back(pending);
            }
        }
        self.pending_data = remaining;
        outbound
    }

    fn drop_pending_for_dst(&mut self, dst_node_id: u32) -> usize {
        let mut remaining = VecDeque::new();
        let mut dropped = 0;
        while let Some(pending) = self.pending_data.pop_front() {
            if pending.dst_node_id == dst_node_id {
                dropped += 1;
            } else {
                remaining.push_back(pending);
            }
        }
        self.pending_data = remaining;
        dropped
    }

    fn drop_data(&mut self, dst_node_id: u32, reason: String) -> ProtocolActions {
        self.metrics.dropped_packets += 1;
        self.metrics.data_dropped += 1;
        warn!(
            node_id = self.config.node_id,
            dst_node_id,
            %reason,
            "DATA dropped"
        );
        let mut actions = ProtocolActions::default();
        actions.events.push(ProtocolEvent::DataDropped {
            node_id: self.config.node_id,
            dst_node_id,
            reason: reason.clone(),
        });
        actions.events.push(ProtocolEvent::RouteError {
            node_id: self.config.node_id,
            dst_node_id,
            reason,
        });
        actions
    }

    fn origin_data(
        &mut self,
        dst_node_id: u32,
        payload: Vec<u8>,
        route: RouteEntry,
    ) -> ProtocolActions {
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        let mut actions = ProtocolActions::default();
        let lookup_packet = Packet::Data {
            src_node_id: self.config.node_id,
            dst_node_id,
            sequence,
            protocol_mode: self.config.protocol_mode,
            route_cursor: 0,
            dsr_path: route.dsr_path.clone(),
            dldsr_label_path: route.dldsr_label_path.clone(),
            payload,
            created_at_millis: now_millis(),
        };
        if let Some(endpoint) = self.next_data_endpoint(&lookup_packet) {
            let Packet::Data {
                src_node_id,
                dst_node_id,
                sequence,
                protocol_mode,
                dsr_path,
                dldsr_label_path,
                payload,
                created_at_millis,
                ..
            } = lookup_packet
            else {
                unreachable!("lookup packet is always Data");
            };
            let packet = Packet::Data {
                src_node_id,
                dst_node_id,
                sequence,
                protocol_mode,
                route_cursor: 1,
                dsr_path,
                dldsr_label_path,
                payload,
                created_at_millis,
            };
            actions.send(packet, endpoint);
            info!(
                node_id = self.config.node_id,
                dst_node_id, sequence, "DATA originated"
            );
        } else {
            return self.drop_data(
                dst_node_id,
                "DATA origin next-hop lookup failed".to_string(),
            );
        }
        actions
    }

    fn next_data_endpoint(&self, packet: &Packet) -> Option<SocketAddr> {
        match packet {
            Packet::Data {
                protocol_mode: ProtocolMode::Dsr,
                route_cursor,
                dsr_path,
                ..
            } => {
                let next = dsr_path.get(route_cursor + 1).copied()?;
                self.endpoint(next)
            }
            Packet::Data {
                protocol_mode: ProtocolMode::DlDsr,
                route_cursor,
                dldsr_label_path,
                ..
            } => {
                let label = dldsr_label_path.get(*route_cursor).copied()?;
                self.neighbors
                    .neighbor_by_label(label)
                    .map(|entry| entry.endpoint)
            }
            _ => None,
        }
    }

    fn is_logical_neighbor(&self, node_id: u32) -> bool {
        self.logical_neighbors().contains(&node_id)
    }

    fn logical_neighbors(&self) -> Vec<u32> {
        self.config
            .topology
            .node(self.config.node_id)
            .map(|node| node.neighbors.clone())
            .unwrap_or_default()
    }

    fn endpoint(&self, node_id: u32) -> Option<SocketAddr> {
        self.config.topology.node(node_id).map(|node| node.endpoint)
    }

    fn node_id_by_endpoint(&self, endpoint: SocketAddr) -> Option<u32> {
        self.config
            .topology
            .nodes
            .iter()
            .find(|node| node.endpoint == endpoint)
            .map(|node| node.node_id)
    }

    fn are_adjacent(&self, left: u32, right: u32) -> bool {
        self.config
            .topology
            .node(left)
            .map(|node| node.neighbors.contains(&right))
            .unwrap_or(false)
    }

    fn validate_rrep(&self, sender: SocketAddr, trace_path: &[u32]) -> Result<(), String> {
        let sender_node_id = self
            .node_id_by_endpoint(sender)
            .ok_or_else(|| format!("RREP sender endpoint {sender} is unknown"))?;
        if !self.is_logical_neighbor(sender_node_id) {
            return Err(format!(
                "RREP sender node {sender_node_id} is not a logical neighbor"
            ));
        }
        let position = trace_path
            .iter()
            .position(|node_id| *node_id == self.config.node_id)
            .ok_or_else(|| "RREP trace_path does not include current node".to_string())?;
        let expected_sender = trace_path
            .get(position + 1)
            .copied()
            .ok_or_else(|| "RREP current node has no sender-side trace hop".to_string())?;
        if sender_node_id != expected_sender {
            return Err(format!(
                "RREP sender node {sender_node_id} does not match trace hop {expected_sender}"
            ));
        }
        if !self.are_adjacent(self.config.node_id, expected_sender) {
            return Err(format!(
                "RREP sender trace hop {expected_sender} is not adjacent to current node"
            ));
        }
        if let Some(reverse_next) = position
            .checked_sub(1)
            .and_then(|idx| trace_path.get(idx).copied())
        {
            if !self.are_adjacent(self.config.node_id, reverse_next) {
                return Err(format!(
                    "RREP reverse next hop {reverse_next} is not adjacent to current node"
                ));
            }
        }
        Ok(())
    }

    fn validate_data_delivery(
        &self,
        sender_node_id: u32,
        protocol_mode: ProtocolMode,
        route_cursor: usize,
        dsr_path: &[u32],
        dldsr_label_path: &[u8],
    ) -> Result<(), String> {
        match protocol_mode {
            ProtocolMode::Dsr => {
                if dsr_path.last().copied() != Some(self.config.node_id) {
                    return Err("DATA DSR path does not end at destination".to_string());
                }
                let expected_position = dsr_path
                    .iter()
                    .position(|node_id| *node_id == self.config.node_id)
                    .ok_or_else(|| "DATA DSR path does not include destination".to_string())?;
                if route_cursor != expected_position {
                    return Err(format!(
                        "DATA DSR delivery cursor {route_cursor} does not match destination position {expected_position}"
                    ));
                }
                let previous = expected_position
                    .checked_sub(1)
                    .and_then(|idx| dsr_path.get(idx).copied())
                    .ok_or_else(|| "DATA DSR destination has no previous hop".to_string())?;
                if previous != sender_node_id {
                    return Err(format!(
                        "DATA DSR sender {sender_node_id} does not match previous hop {previous}"
                    ));
                }
            }
            ProtocolMode::DlDsr => {
                if route_cursor != dldsr_label_path.len() {
                    return Err(format!(
                        "DATA DL-DSR delivery cursor {route_cursor} does not equal label path length {}",
                        dldsr_label_path.len()
                    ));
                }
            }
        }
        Ok(())
    }
}

fn previous_in_trace(trace_path: &[u32], current: u32) -> Option<u32> {
    let position = trace_path.iter().position(|node_id| *node_id == current)?;
    position
        .checked_sub(1)
        .and_then(|previous| trace_path.get(previous).copied())
}

pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn is_control_packet(packet: &Packet) -> bool {
    matches!(
        packet,
        Packet::Hello { .. }
            | Packet::HelloReply { .. }
            | Packet::Rreq { .. }
            | Packet::Rrep { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use dldsr_core::{Topology, TopologyNode};

    fn addr(port: u16) -> SocketAddr {
        format!("127.0.0.1:{port}").parse().unwrap()
    }

    fn chain_config(node_id: u32, mode: ProtocolMode) -> NodeConfig {
        NodeConfig {
            node_id,
            bind_addr: addr(7000 + node_id as u16),
            protocol_mode: mode,
            hello_interval_ms: 200,
            route_cache_ttl_ms: 30_000,
            neighbor_ttl_ms: 5_000,
            log_level: "info".to_string(),
            topology: Topology {
                nodes: vec![
                    TopologyNode {
                        node_id: 0,
                        endpoint: addr(7000),
                        neighbors: vec![1],
                    },
                    TopologyNode {
                        node_id: 1,
                        endpoint: addr(7001),
                        neighbors: vec![0, 2],
                    },
                    TopologyNode {
                        node_id: 2,
                        endpoint: addr(7002),
                        neighbors: vec![1, 3],
                    },
                    TopologyNode {
                        node_id: 3,
                        endpoint: addr(7003),
                        neighbors: vec![2],
                    },
                ],
            },
            demo_send: None,
            route_discovery_timeout_ms: 10,
            route_discovery_max_retries: 1,
            max_rreq_hops: 4,
            seen_request_ttl_ms: 60_000,
            seen_request_max_entries: 10_000,
            metrics_output_path: None,
            control_bind_addr: None,
            strict_protocol_mode: true,
        }
    }

    fn rreq(hop_count: u8) -> Packet {
        Packet::Rreq {
            src_node_id: 0,
            dst_node_id: 3,
            request_id: 77,
            previous_hop: 0,
            hop_count,
            dsr_path: vec![0],
            dldsr_label_path: Vec::new(),
            trace_path: vec![0],
            created_at_millis: now_millis(),
        }
    }

    #[test]
    fn duplicate_rreq_is_dropped_and_counted() {
        let mut state = ProtocolState::new(chain_config(1, ProtocolMode::Dsr));
        let first = state.handle_packet(rreq(0), addr(7000));
        let second = state.handle_packet(rreq(0), addr(7000));

        assert_eq!(first.outbound.len(), 1);
        assert!(second.outbound.is_empty());
        assert_eq!(state.metrics.rreq_dropped_duplicate, 1);
    }

    #[test]
    fn rreq_ttl_drop_is_counted() {
        let mut state = ProtocolState::new(chain_config(1, ProtocolMode::Dsr));
        let actions = state.handle_packet(rreq(4), addr(7000));

        assert!(actions.outbound.is_empty());
        assert_eq!(state.metrics.rreq_dropped_ttl, 1);
    }

    #[test]
    fn missing_dldsr_label_drops_data() {
        let mut state = ProtocolState::new(chain_config(1, ProtocolMode::DlDsr));
        let actions = state.handle_packet(
            Packet::Data {
                src_node_id: 0,
                dst_node_id: 3,
                sequence: 1,
                protocol_mode: ProtocolMode::DlDsr,
                route_cursor: 0,
                dsr_path: vec![0, 1, 2, 3],
                dldsr_label_path: vec![99],
                payload: b"bad label".to_vec(),
                created_at_millis: now_millis(),
            },
            addr(7000),
        );

        assert!(actions.outbound.is_empty());
        assert!(matches!(
            actions.events.first(),
            Some(ProtocolEvent::DataDropped { .. })
        ));
        assert_eq!(state.metrics.data_dropped, 1);
    }

    #[test]
    fn invalid_rrep_trace_is_dropped() {
        let mut state = ProtocolState::new(chain_config(1, ProtocolMode::Dsr));
        let actions = state.handle_packet(
            Packet::Rrep {
                src_node_id: 0,
                dst_node_id: 3,
                request_id: 7,
                dsr_path: vec![0, 1, 2, 3],
                dldsr_label_path: Vec::new(),
                trace_path: vec![0, 3],
                created_at_millis: now_millis(),
            },
            addr(7002),
        );

        assert!(actions.outbound.is_empty());
        assert_eq!(state.metrics.rrep_dropped_invalid, 1);
    }

    #[test]
    fn route_discovery_failure_drops_pending_payload() {
        let mut state = ProtocolState::new(chain_config(0, ProtocolMode::Dsr));
        let start = state.handle_command(ProtocolCommand::DiscoverAndSend {
            dst_node_id: 99,
            payload: b"unreachable".to_vec(),
        });
        assert_eq!(start.outbound.len(), 1);

        let retry = state.handle_tick(now_millis() + 20);
        assert_eq!(retry.outbound.len(), 1);

        let failed = state.handle_tick(now_millis() + 40);
        assert!(failed
            .events
            .iter()
            .any(|event| matches!(event, ProtocolEvent::RouteDiscoveryFailed { .. })));
        assert_eq!(state.metrics.route_discovery_failures, 1);
        assert_eq!(state.metrics.data_dropped, 1);
    }
}
