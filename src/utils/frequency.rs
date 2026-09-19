//! Pitch helpers.

/// Convert a MIDI note number to its frequency in Hz.
///
/// Uses 12-tone equal temperament with A4 = 440 Hz (note 69). The argument is
/// an `f32` so callers can fold in per-instrument octave offsets before calling.
pub fn note_to_freq(note: f32) -> f32 {
    440.0 * 2.0_f32.powf((note - 69.0) / 12.0)
}

/// MIDI note name in scientific pitch notation, e.g. `C4` or `F#3`.
pub fn note_name(note: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let octave = note as i32 / 12 - 1;
    format!("{}{}", NAMES[note as usize % 12], octave)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a4_is_440() {
        assert!((note_to_freq(69.0) - 440.0).abs() < 1e-3);
    }

    #[test]
    fn an_octave_doubles_frequency() {
        assert!((note_to_freq(81.0) - 880.0).abs() < 1e-2);
    }

    #[test]
    fn names_are_readable() {
        assert_eq!(note_name(60), "C4");
        assert_eq!(note_name(66), "F#4");
    }
}
