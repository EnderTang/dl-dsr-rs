use clap::{Parser, Subcommand};
use dldsr_core::ProtocolMode;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(author, version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
    #[arg(long)]
    pub config: Option<PathBuf>,
    #[arg(long)]
    pub node_id: Option<u32>,
    #[arg(long)]
    pub bind: Option<SocketAddr>,
    #[arg(long)]
    pub topology: Option<PathBuf>,
    #[arg(long)]
    pub mode: Option<ProtocolMode>,
    #[arg(long)]
    pub send_dst: Option<u32>,
    #[arg(long, default_value = "hello from dl-dsr-rs")]
    pub payload: Option<String>,
    #[arg(long, default_value_t = 2_000)]
    pub send_after_ms: u64,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Send {
        #[arg(long)]
        control_addr: SocketAddr,
        #[arg(long)]
        dst: u32,
        #[arg(long)]
        payload: String,
    },
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
