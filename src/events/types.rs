#[derive(Debug, Clone)]
pub enum Event {
    KeyPress(String),
    Quit,
    Play { frame_offset: u64 },
    Stop { frame_offset: u64 },
    Record { frame_offset: u64 },
    TransportTick,
    NoteOn(u8, u8, u8),
    NoteOff(u8, u8, u8),
}
