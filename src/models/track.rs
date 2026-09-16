use crate::models::clip::Clip;
use crate::models::instrument::Instrument;

pub struct Track {
    id: u32,
    name: String,
    clips: Vec<Clip>,
    volume: f32,
    pan: f32,
    muted: bool,
    armed: bool,
    instrument: Instrument,
    input_gain: f32,
    pan_enabled: bool,
    solo: bool,
}
