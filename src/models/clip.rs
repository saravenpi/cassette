pub struct Clip {
    id: u32,
    name: String,
    slot: u32,
    loop_enabled: bool,
    start: f32,
    duration: f32,
    muted: bool,
    volume: f32,
    pan: f32,
}
