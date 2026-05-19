use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EnergyEstimate {
    pub tx_energy_units: f64,
    pub rx_energy_units: f64,
    pub total_energy_units: f64,
}

impl EnergyEstimate {
    pub fn from_bytes(tx_bytes: u64, rx_bytes: u64) -> Self {
        let tx_energy_units = tx_bytes as f64 * 1.0;
        let rx_energy_units = rx_bytes as f64 * 0.5;
        Self {
            tx_energy_units,
            rx_energy_units,
            total_energy_units: tx_energy_units + rx_energy_units,
        }
    }
}
