#[derive(Debug, thiserror::Error)]
pub enum MidiError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(#[from] midly::Error),
}
