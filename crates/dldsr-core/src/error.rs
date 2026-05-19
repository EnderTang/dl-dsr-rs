use thiserror::Error;

#[derive(Debug, Error)]
pub enum DldsrError {
    #[error("no labels are available for active neighbor table")]
    LabelSpaceExhausted,
    #[error("route not found for destination {0}")]
    RouteNotFound(u32),
    #[error("label {0} does not map to a known neighbor")]
    UnknownLabel(u8),
    #[error("packet codec failed: {0}")]
    Codec(String),
}

pub type Result<T> = std::result::Result<T, DldsrError>;
