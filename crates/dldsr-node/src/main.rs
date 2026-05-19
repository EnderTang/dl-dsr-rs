use anyhow::Result;
use dldsr_node::cli::{Cli, Command};
use dldsr_node::config;
use dldsr_node::daemon::Daemon;
use serde_json::json;
use tokio::net::UdpSocket;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse_args();
    if let Some(Command::Send {
        control_addr,
        dst,
        payload,
    }) = cli.command
    {
        let socket = UdpSocket::bind("127.0.0.1:0").await?;
        let request = json!({ "dst": dst, "payload": payload }).to_string();
        socket.send_to(request.as_bytes(), control_addr).await?;
        let mut buf = [0_u8; 1024];
        if let Ok(Ok((len, _))) = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            socket.recv_from(&mut buf),
        )
        .await
        {
            println!("{}", String::from_utf8_lossy(&buf[..len]));
        }
        return Ok(());
    }

    let config = config::load(cli)?;
    tracing_subscriber::fmt()
        .with_env_filter(config.log_level.clone())
        .with_target(false)
        .init();
    Daemon::new(config).run().await
}
