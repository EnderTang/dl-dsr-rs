use crate::error::{DldsrError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolMode {
    Dsr,
    DlDsr,
}

impl std::str::FromStr for ProtocolMode {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "dsr" => Ok(Self::Dsr),
            "dldsr" | "dl-dsr" => Ok(Self::DlDsr),
            other => Err(format!("unknown protocol mode: {other}")),
        }
    }
}

impl std::fmt::Display for ProtocolMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dsr => write!(f, "dsr"),
            Self::DlDsr => write!(f, "dldsr"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Packet {
    Hello {
        src_node_id: u32,
        timestamp_millis: u64,
    },
    HelloReply {
        src_node_id: u32,
        dst_node_id: u32,
        assigned_label: u8,
        update_flag: bool,
        timestamp_millis: u64,
    },
    Rreq {
        src_node_id: u32,
        dst_node_id: u32,
        request_id: u64,
        previous_hop: u32,
        hop_count: u8,
        dsr_path: Vec<u32>,
        dldsr_label_path: Vec<u8>,
        trace_path: Vec<u32>,
        created_at_millis: u64,
    },
    Rrep {
        src_node_id: u32,
        dst_node_id: u32,
        request_id: u64,
        dsr_path: Vec<u32>,
        dldsr_label_path: Vec<u8>,
        trace_path: Vec<u32>,
        created_at_millis: u64,
    },
    Data {
        src_node_id: u32,
        dst_node_id: u32,
        sequence: u64,
        protocol_mode: ProtocolMode,
        route_cursor: usize,
        dsr_path: Vec<u32>,
        dldsr_label_path: Vec<u8>,
        payload: Vec<u8>,
        created_at_millis: u64,
    },
    Ack {
        src_node_id: u32,
        dst_node_id: u32,
        sequence: u64,
    },
    Error {
        src_node_id: u32,
        dst_node_id: u32,
        message: String,
    },
}

impl Packet {
    pub fn encode(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|err| DldsrError::Codec(err.to_string()))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(|err| DldsrError::Codec(err.to_string()))
    }

    pub fn encoded_len(&self) -> Result<usize> {
        Ok(self.encode()?.len())
    }
}

pub fn path_header_bytes(mode: ProtocolMode, hops: usize) -> usize {
    match mode {
        ProtocolMode::Dsr => hops * std::mem::size_of::<u32>(),
        ProtocolMode::DlDsr => hops * std::mem::size_of::<u8>(),
    }
}
