use dldsr_core::{simulate_chain, ProtocolMode};

#[test]
fn four_node_chain_delivers_and_dldsr_has_smaller_header() {
    let dsr = simulate_chain(4, ProtocolMode::Dsr, 64, 25);
    let dldsr = simulate_chain(4, ProtocolMode::DlDsr, 64, 25);

    assert_eq!(dsr.delivered_packets, 25);
    assert_eq!(dldsr.delivered_packets, 25);
    assert!(dsr.path_header_bytes > dldsr.path_header_bytes);
    assert!(dsr.total_tx_bytes > dldsr.total_tx_bytes);
}

