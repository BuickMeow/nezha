pub mod error;
pub mod midi;
pub mod parser;
pub mod time;

pub use error::MidiError;
pub use midi::{LoadProgress, MidiFile, Note};
pub use parser::MidiParser;
pub use time::{TempoSegment, is_black_key};
