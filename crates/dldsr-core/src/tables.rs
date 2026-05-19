use crate::error::{DldsrError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeighborEntry {
    pub node_id: u32,
    pub label_assigned_by_me_to_neighbor: u8,
    pub last_seen_millis: u64,
    pub endpoint: SocketAddr,
    pub alive: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct NeighborTable {
    entries: HashMap<u32, NeighborEntry>,
}

impl NeighborTable {
    pub fn assign_or_reuse_label(
        &mut self,
        node_id: u32,
        endpoint: SocketAddr,
        now: u64,
    ) -> Result<u8> {
        if let Some(entry) = self.entries.get_mut(&node_id) {
            entry.endpoint = endpoint;
            entry.last_seen_millis = now;
            entry.alive = true;
            return Ok(entry.label_assigned_by_me_to_neighbor);
        }

        let label = (1..=u8::MAX)
            .find(|candidate| {
                !self.entries.values().any(|entry| {
                    entry.alive && entry.label_assigned_by_me_to_neighbor == *candidate
                })
            })
            .ok_or(DldsrError::LabelSpaceExhausted)?;

        self.entries.insert(
            node_id,
            NeighborEntry {
                node_id,
                label_assigned_by_me_to_neighbor: label,
                last_seen_millis: now,
                endpoint,
                alive: true,
            },
        );
        Ok(label)
    }

    pub fn neighbor_by_label(&self, label: u8) -> Option<&NeighborEntry> {
        self.entries
            .values()
            .find(|entry| entry.alive && entry.label_assigned_by_me_to_neighbor == label)
    }

    pub fn get(&self, node_id: u32) -> Option<&NeighborEntry> {
        self.entries.get(&node_id)
    }

    pub fn entries(&self) -> impl Iterator<Item = &NeighborEntry> {
        self.entries.values()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelEntry {
    pub neighbor_node_id: u32,
    pub label_assigned_by_neighbor_to_me: u8,
    pub last_seen_millis: u64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LabelTable {
    entries: HashMap<u32, LabelEntry>,
}

impl LabelTable {
    pub fn update_from_hello_reply(&mut self, neighbor_node_id: u32, label: u8, now: u64) {
        self.entries.insert(
            neighbor_node_id,
            LabelEntry {
                neighbor_node_id,
                label_assigned_by_neighbor_to_me: label,
                last_seen_millis: now,
            },
        );
    }

    pub fn label_assigned_by_neighbor(&self, neighbor_node_id: u32) -> Option<u8> {
        self.entries
            .get(&neighbor_node_id)
            .map(|entry| entry.label_assigned_by_neighbor_to_me)
    }
}
