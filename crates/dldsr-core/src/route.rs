use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteEntry {
    pub dst_node_id: u32,
    pub dsr_path: Vec<u32>,
    pub dldsr_label_path: Vec<u8>,
    pub expires_at_millis: u64,
}

impl RouteEntry {
    pub fn new_dsr(dst_node_id: u32, dsr_path: Vec<u32>, expires_at_millis: u64) -> Self {
        Self {
            dst_node_id,
            dsr_path,
            dldsr_label_path: Vec::new(),
            expires_at_millis,
        }
    }

    pub fn new_dldsr(dst_node_id: u32, dldsr_label_path: Vec<u8>, expires_at_millis: u64) -> Self {
        Self {
            dst_node_id,
            dsr_path: Vec::new(),
            dldsr_label_path,
            expires_at_millis,
        }
    }

    pub fn hop_count(&self) -> usize {
        if self.dsr_path.is_empty() {
            self.dldsr_label_path.len()
        } else {
            self.dsr_path.len()
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RouteCache {
    routes: HashMap<u32, Vec<RouteEntry>>,
}

impl RouteCache {
    pub fn insert(&mut self, route: RouteEntry) {
        self.routes
            .entry(route.dst_node_id)
            .or_default()
            .push(route);
    }

    pub fn shortest(&self, dst_node_id: u32, now: u64) -> Option<&RouteEntry> {
        self.routes.get(&dst_node_id).and_then(|routes| {
            routes
                .iter()
                .filter(|route| route.expires_at_millis >= now)
                .min_by_key(|route| route.hop_count())
        })
    }
}

#[derive(Debug, Default, Clone)]
pub struct SeenRequests {
    seen: HashMap<(u32, u64), u64>,
}

impl SeenRequests {
    pub fn mark_if_new(
        &mut self,
        src_node_id: u32,
        request_id: u64,
        now: u64,
        ttl_ms: u64,
        max_entries: usize,
    ) -> bool {
        self.prune(now, ttl_ms);
        let is_new = self.seen.insert((src_node_id, request_id), now).is_none();
        self.enforce_max_entries(max_entries);
        is_new
    }

    pub fn prune(&mut self, now: u64, ttl_ms: u64) {
        self.seen
            .retain(|_, seen_at| now.saturating_sub(*seen_at) <= ttl_ms);
    }

    pub fn len(&self) -> usize {
        self.seen.len()
    }

    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }

    fn enforce_max_entries(&mut self, max_entries: usize) {
        if max_entries == 0 {
            self.seen.clear();
            return;
        }
        if self.seen.len() <= max_entries {
            return;
        }

        let mut entries: Vec<((u32, u64), u64)> = self
            .seen
            .iter()
            .map(|(key, seen_at)| (*key, *seen_at))
            .collect();
        entries.sort_by_key(|(key, seen_at)| (*seen_at, key.0, key.1));

        let remove_count = entries.len() - max_entries;
        for (key, _) in entries.into_iter().take(remove_count) {
            self.seen.remove(&key);
        }
    }
}
