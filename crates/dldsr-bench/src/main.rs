mod report;

use anyhow::Result;
use clap::Parser;
use dldsr_core::{simulate_chain, BenchmarkReport, ProtocolMode};
use serde::Serialize;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "dldsr")]
    mode: ProtocolMode,
    #[arg(long, default_value_t = 4)]
    hops: usize,
    #[arg(long, default_value_t = 64)]
    payload_size: usize,
    #[arg(long, default_value_t = 100)]
    packets: u64,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    compare: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.compare {
        let comparison = ComparisonReport::new(args.hops, args.payload_size, args.packets);
        if args.json {
            println!("{}", serde_json::to_string_pretty(&comparison)?);
        } else {
            println!("{}", report::comparison_markdown(&comparison));
        }
    } else {
        let report = simulate_chain(args.hops, args.mode, args.payload_size, args.packets);
        if args.json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            println!("{}", report::markdown(&report));
        }
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct ComparisonReport {
    pub dsr: BenchmarkReport,
    pub dldsr: BenchmarkReport,
    pub path_header_savings_bytes: u64,
    pub total_tx_savings_bytes: u64,
    pub estimated_energy_savings_units: f64,
}

impl ComparisonReport {
    fn new(hops: usize, payload_size: usize, packet_count: u64) -> Self {
        let dsr = simulate_chain(hops, ProtocolMode::Dsr, payload_size, packet_count);
        let dldsr = simulate_chain(hops, ProtocolMode::DlDsr, payload_size, packet_count);
        Self {
            path_header_savings_bytes: dsr
                .path_header_bytes
                .saturating_sub(dldsr.path_header_bytes),
            total_tx_savings_bytes: dsr.total_tx_bytes.saturating_sub(dldsr.total_tx_bytes),
            estimated_energy_savings_units: dsr.estimated_energy_units
                - dldsr.estimated_energy_units,
            dsr,
            dldsr,
        }
    }
}
