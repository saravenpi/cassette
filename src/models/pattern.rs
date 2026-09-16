pub struct Pattern {
    id: u32,
    name: String,
    notes: Vec<Note>,
}

#[derive(Clone)]
pub struct Note {
    pitch: u8,
    velocity: u8,
    start: f32,
    duration: f32,
}
