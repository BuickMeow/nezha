use std::fmt;

#[derive(Debug)]
pub enum MidiError {
    Io(std::io::Error),
    Parse(midly::Error),
}

impl fmt::Display for MidiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MidiError::Io(e) => write!(f, "IO error: {e}"),
            MidiError::Parse(e) => write!(f, "Parse error: {e}"),
        }
    }
}

impl std::error::Error for MidiError {}

impl From<std::io::Error> for MidiError {
    fn from(e: std::io::Error) -> Self {
        MidiError::Io(e)
    }
}

impl From<midly::Error> for MidiError {
    fn from(e: midly::Error) -> Self {
        MidiError::Parse(e)
    }
}
