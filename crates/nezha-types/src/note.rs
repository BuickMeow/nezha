#[derive(Clone, Debug, Default)]
pub struct Note {
    pub key: u8,
    pub start: f64,
    pub end: f64,
    pub start_tick: u32,
    pub end_tick: u32,
    pub velocity: u8,
    pub channel: u8,
    pub track: u16,
}
