pub mod energy;
pub mod error;
pub mod label;
pub mod metrics;
pub mod packet;
pub mod route;
pub mod tables;
pub mod topology;

pub use energy::*;
pub use error::*;
pub use label::*;
pub use metrics::*;
pub use packet::*;
pub use route::*;
pub use tables::*;
pub use topology::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    fn addr(port: u16) -> SocketAddr {
        format!("127.0.0.1:{port}").parse().unwrap()
    }

    #[test]
    fn labels_are_unique_for_active_neighbors() {
        let mut table = NeighborTable::default();
        let a = table.assign_or_reuse_label(1, addr(7001), 1).unwrap();
        let b = table.assign_or_reuse_label(2, addr(7002), 1).unwrap();
        assert_ne!(a, b);
        assert_eq!(a, table.assign_or_reuse_label(1, addr(7001), 2).unwrap());
    }

    #[test]
    fn label_table_updates_from_hello_reply() {
        let mut labels = LabelTable::default();
        labels.update_from_hello_reply(7, 42, 100);
        assert_eq!(labels.label_assigned_by_neighbor(7), Some(42));
    }

    #[test]
    fn dsr_header_is_larger_than_dldsr_header() {
        assert!(
            path_header_bytes(ProtocolMode::Dsr, 8) > path_header_bytes(ProtocolMode::DlDsr, 8)
        );
    }

    #[test]
    fn route_cache_chooses_shortest_path() {
        let mut cache = RouteCache::default();
        cache.insert(RouteEntry::new_dsr(9, vec![0, 1, 2, 9], 1000));
        cache.insert(RouteEntry::new_dsr(9, vec![0, 3, 9], 1000));
        assert_eq!(cache.shortest(9, 1).unwrap().hop_count(), 3);
    }

    #[test]
    fn duplicate_rreq_detection_works() {
        let mut seen = SeenRequests::default();
        assert!(seen.mark_if_new(1, 99, 1, 60_000, 10_000));
        assert!(!seen.mark_if_new(1, 99, 2, 60_000, 10_000));
    }

    #[test]
    fn seen_requests_enforces_max_entries() {
        let mut seen = SeenRequests::default();
        assert!(seen.mark_if_new(1, 1, 10, 60_000, 2));
        assert!(seen.mark_if_new(1, 2, 20, 60_000, 2));
        assert!(seen.mark_if_new(1, 3, 30, 60_000, 2));

        assert_eq!(seen.len(), 2);
        assert!(seen.mark_if_new(1, 1, 40, 60_000, 2));
    }

    #[test]
    fn dldsr_next_hop_lookup_by_label_works() {
        let mut table = NeighborTable::default();
        let label = table.assign_or_reuse_label(2, addr(7002), 1).unwrap();
        assert_eq!(table.neighbor_by_label(label).unwrap().node_id, 2);
    }

    #[test]
    fn deterministic_smoke_compares_modes() {
        let dsr = simulate_chain(4, ProtocolMode::Dsr, 64, 10);
        let dldsr = simulate_chain(4, ProtocolMode::DlDsr, 64, 10);
        assert_eq!(dsr.delivered_packets, 10);
        assert_eq!(dldsr.delivered_packets, 10);
        assert!(dsr.path_header_bytes > dldsr.path_header_bytes);
    }
}
