use dldsr_core::{NodeConfig, ProtocolMode, Topology};
use dldsr_node::daemon::Daemon;
use dldsr_node::protocol::{ProtocolCommand, ProtocolEvent};
use serde_json::Value;
use std::net::{SocketAddr, UdpSocket};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn live_dldsr_route_discovery_and_data_delivery() {
    run_live_chain(ProtocolMode::DlDsr).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn live_dsr_route_discovery_and_data_delivery() {
    run_live_chain(ProtocolMode::Dsr).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn runtime_control_trigger_sends_data() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("dldsr_node=trace,info")
        .with_test_writer()
        .try_init();

    let addrs = reserve_local_addrs(4);
    let control_addr = reserve_local_addrs(1)[0];
    let topology = Topology {
        nodes: vec![
            node(0, addrs[0], vec![1]),
            node(1, addrs[1], vec![0, 2]),
            node(2, addrs[2], vec![1, 3]),
            node(3, addrs[3], vec![2]),
        ],
    };

    let (event_tx, mut event_rx) = mpsc::channel(64);
    let mut handles = Vec::new();
    for node_id in 0..4 {
        let mut cfg = config(
            node_id,
            addrs[node_id as usize],
            ProtocolMode::DlDsr,
            topology.clone(),
        );
        if node_id == 0 {
            cfg.control_bind_addr = Some(control_addr);
        }
        let events = event_tx.clone();
        handles.push(tokio::spawn(async move {
            Daemon::new(cfg).run_with_channels(None, Some(events)).await
        }));
    }
    drop(event_tx);

    tokio::time::sleep(Duration::from_millis(900)).await;
    let socket = tokio::net::UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("bind control client");
    socket
        .send_to(br#"{"dst":3,"payload":"runtime hello"}"#, control_addr)
        .await
        .expect("send control request");

    let delivered = timeout(Duration::from_secs(5), async {
        loop {
            match event_rx.recv().await {
                Some(ProtocolEvent::DataDelivered { payload, .. })
                    if payload == b"runtime hello" =>
                {
                    break true;
                }
                Some(_) => {}
                None => break false,
            }
        }
    })
    .await
    .expect("runtime control delivery timed out");

    for handle in handles {
        handle.abort();
    }

    assert!(delivered, "control request should use live send path");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn metrics_output_path_writes_valid_json() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();

    let addrs = reserve_local_addrs(1);
    let metrics_path = unique_temp_path("dldsr-node-metrics.json");
    let topology = Topology {
        nodes: vec![node(0, addrs[0], Vec::new())],
    };
    let mut cfg = config(0, addrs[0], ProtocolMode::DlDsr, topology);
    cfg.metrics_output_path = Some(metrics_path.clone());

    let handle = tokio::spawn(async move { Daemon::new(cfg).run().await });
    tokio::time::sleep(Duration::from_millis(5_500)).await;
    handle.abort();

    let text = std::fs::read_to_string(&metrics_path).expect("metrics file should exist");
    let json: Value = serde_json::from_str(&text).expect("metrics JSON should parse");
    assert_eq!(json["node_id"], 0);
    assert_eq!(json["protocol_mode"], "DlDsr");
    assert!(json["tx_bytes"].is_u64());
    let _ = std::fs::remove_file(metrics_path);
}

async fn run_live_chain(mode: ProtocolMode) {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("dldsr_node=trace,info")
        .with_test_writer()
        .try_init();

    let addrs = reserve_local_addrs(4);
    let topology = Topology {
        nodes: vec![
            node(0, addrs[0], vec![1]),
            node(1, addrs[1], vec![0, 2]),
            node(2, addrs[2], vec![1, 3]),
            node(3, addrs[3], vec![2]),
        ],
    };

    let (event_tx, mut event_rx) = mpsc::channel(64);
    let mut command_txs = Vec::new();
    let mut handles = Vec::new();

    for node_id in 0..4 {
        let (command_tx, command_rx) = mpsc::channel(8);
        command_txs.push(command_tx);
        let daemon = Daemon::new(config(
            node_id,
            addrs[node_id as usize],
            mode,
            topology.clone(),
        ));
        let events = event_tx.clone();
        handles.push(tokio::spawn(async move {
            daemon
                .run_with_channels(Some(command_rx), Some(events))
                .await
        }));
    }
    drop(event_tx);

    // Hello/HelloReply is periodic and UDP-based. This short wait lets both
    // directions in each logical link learn labels before route discovery.
    tokio::time::sleep(Duration::from_millis(900)).await;
    for handle in &handles {
        assert!(!handle.is_finished(), "daemon task exited during startup");
    }

    command_txs[0]
        .send(ProtocolCommand::DiscoverAndSend {
            dst_node_id: 3,
            payload: b"live hello".to_vec(),
        })
        .await
        .expect("node0 command channel should be open");

    let delivered = timeout(Duration::from_secs(5), async {
        loop {
            match event_rx.recv().await {
                Some(ProtocolEvent::DataDelivered {
                    node_id,
                    src_node_id,
                    dst_node_id,
                    payload,
                    ..
                }) if node_id == 3
                    && src_node_id == 0
                    && dst_node_id == 3
                    && payload == b"live hello" =>
                {
                    break true;
                }
                Some(_) => {}
                None => break false,
            }
        }
    })
    .await
    .expect("live route discovery timed out");

    for handle in handles {
        handle.abort();
    }

    assert!(delivered, "node3 should receive the live UDP data packet");
}

fn config(
    node_id: u32,
    bind_addr: SocketAddr,
    mode: ProtocolMode,
    topology: Topology,
) -> NodeConfig {
    NodeConfig {
        node_id,
        bind_addr,
        protocol_mode: mode,
        hello_interval_ms: 200,
        route_cache_ttl_ms: 30_000,
        neighbor_ttl_ms: 5_000,
        log_level: "info".to_string(),
        topology,
        demo_send: None,
        route_discovery_timeout_ms: 2_000,
        route_discovery_max_retries: 3,
        max_rreq_hops: 64,
        seen_request_ttl_ms: 60_000,
        seen_request_max_entries: 10_000,
        metrics_output_path: None,
        control_bind_addr: None,
        strict_protocol_mode: true,
    }
}

fn node(node_id: u32, endpoint: SocketAddr, neighbors: Vec<u32>) -> dldsr_core::TopologyNode {
    dldsr_core::TopologyNode {
        node_id,
        endpoint,
        neighbors,
    }
}

fn reserve_local_addrs(count: usize) -> Vec<SocketAddr> {
    let sockets: Vec<UdpSocket> = (0..count)
        .map(|_| UdpSocket::bind("127.0.0.1:0").expect("reserve UDP port"))
        .collect();
    let addrs = sockets
        .iter()
        .map(|socket| socket.local_addr().expect("read local addr"))
        .collect();
    drop(sockets);
    addrs
}

fn unique_temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos(),
        name
    ));
    path
}
