//! Key bindings.
//!
//! Lower-case letters and the number row form a two-octave piano. Upper-case
//! letters and punctuation are commands, so playing and editing never collide.
//!
//! The same keymap also drives the block-name prompt: in text mode every
//! printable character becomes [`Event::TextInput`] and commands are suspended.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::events::Event;

/// Translates key presses into [`Event`]s.
#[derive(Debug, Clone)]
pub struct Keymap {
    /// Transposition of the play keyboard, in octaves.
    octave: i32,
    /// Whether keystrokes feed the block-name prompt instead of the piano.
    text_mode: bool,
}

impl Default for Keymap {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Keymap {
    /// Create a keymap with the given keyboard octave offset.
    pub fn new(octave: i32) -> Self {
        Self {
            octave,
            text_mode: false,
        }
    }

    /// Current keyboard octave offset.
    pub fn octave(&self) -> i32 {
        self.octave
    }

    /// Set the keyboard octave offset, clamped to a playable range.
    pub fn set_octave(&mut self, octave: i32) {
        self.octave = octave.clamp(-4, 4);
    }

    /// Route keystrokes to the block-name prompt (`true`) or the piano.
    pub fn set_text_mode(&mut self, text_mode: bool) {
        self.text_mode = text_mode;
    }

    /// Whether keystrokes currently feed the block-name prompt.
    pub fn text_mode(&self) -> bool {
        self.text_mode
    }

    /// Map a key event. Returns `None` for unmapped keys and key releases of
    /// non-piano keys.
    pub fn map(&self, key: KeyEvent) -> Option<Event> {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('q'))
        {
            return Some(Event::Quit);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return None;
        }
        if self.text_mode {
            return self.map_text(key);
        }
        if key.kind == KeyEventKind::Release {
            if let KeyCode::Char(c) = key.code
                && let Some(pitch) = self.pitch_for(c)
            {
                return Some(Event::NoteOff { pitch });
            }
            return None;
        }

        // The piano is only playable on press (and auto-repeat).
        if let KeyCode::Char(c) = key.code
            && let Some(pitch) = self.pitch_for(c)
        {
            return Some(Event::NoteOn {
                pitch,
                velocity: 100,
            });
        }

        match key.code {
            KeyCode::Char(' ') => Some(Event::TogglePlay),
            KeyCode::Enter => Some(Event::RenameBlock),
            KeyCode::Left => Some(Event::MoveBlock(-1)),
            KeyCode::Right => Some(Event::MoveBlock(1)),
            KeyCode::Up => Some(Event::MoveTrack(-1)),
            KeyCode::Down => Some(Event::MoveTrack(1)),
            KeyCode::Char('A') => Some(Event::AddBlock),
            KeyCode::Char('D') => Some(Event::DeleteBlock),
            KeyCode::Char('K') => Some(Event::CycleTrackKind),
            KeyCode::Char(',') => Some(Event::VolumeDelta(-0.05)),
            KeyCode::Char('.') => Some(Event::VolumeDelta(0.05)),
            KeyCode::Char('<') => Some(Event::BpmDelta(-1.0)),
            KeyCode::Char('>') => Some(Event::BpmDelta(1.0)),
            KeyCode::Char('M') => Some(Event::ToggleMute),
            KeyCode::Char('S') => Some(Event::ToggleSolo),
            KeyCode::Char('L') => Some(Event::ToggleLoop),
            KeyCode::Char('P') => Some(Event::PanDelta(-0.1)),
            KeyCode::Char('N') => Some(Event::PanDelta(0.1)),
            KeyCode::Char('Q') => Some(Event::Quit),
            KeyCode::Char('?') => Some(Event::ToggleHelp),
            KeyCode::PageUp => Some(Event::OctaveDelta(1)),
            KeyCode::PageDown => Some(Event::OctaveDelta(-1)),
            _ => None,
        }
    }

    /// Map a key while the block-name prompt owns the keyboard.
    fn map_text(&self, key: KeyEvent) -> Option<Event> {
        if key.kind == KeyEventKind::Release {
            return None;
        }
        match key.code {
            KeyCode::Char(c) => Some(Event::TextInput(c)),
            KeyCode::Backspace => Some(Event::TextBackspace),
            KeyCode::Enter => Some(Event::RenameBlock),
            KeyCode::Esc => Some(Event::CancelRename),
            _ => None,
        }
    }

    /// Semitone offset for a lower-case piano key, if it is one.
    ///
    /// The layout mirrors a tracker: `z` row is the lower octave, `q` row the
    /// upper one.
    fn pitch_for(&self, c: char) -> Option<u8> {
        let offset = match c {
            'z' => 0,
            's' => 1,
            'x' => 2,
            'd' => 3,
            'c' => 4,
            'v' => 5,
            'g' => 6,
            'b' => 7,
            'h' => 8,
            'n' => 9,
            'j' => 10,
            'm' => 11,
            'q' => 12,
            '2' => 13,
            'w' => 14,
            '3' => 15,
            'e' => 16,
            'r' => 17,
            '5' => 18,
            't' => 19,
            '6' => 20,
            'y' => 21,
            '7' => 22,
            'u' => 23,
            _ => return None,
        };
        // Middle C is 60.
        let note = 60 + self.octave * 12 + offset;
        u8::try_from(note.clamp(0, 127)).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn space_toggles_playback() {
        let keymap = Keymap::default();
        assert_eq!(
            keymap.map(press(KeyCode::Char(' '))),
            Some(Event::TogglePlay)
        );
    }

    #[test]
    fn lowercase_letters_play_notes() {
        let keymap = Keymap::default();
        assert_eq!(
            keymap.map(press(KeyCode::Char('z'))),
            Some(Event::NoteOn {
                pitch: 60,
                velocity: 100
            })
        );
        assert_eq!(
            keymap.map(press(KeyCode::Char('m'))),
            Some(Event::NoteOn {
                pitch: 71,
                velocity: 100
            })
        );
    }

    #[test]
    fn octave_shift_transposes_the_keyboard() {
        let mut keymap = Keymap::default();
        keymap.set_octave(1);
        assert_eq!(
            keymap.map(press(KeyCode::Char('z'))),
            Some(Event::NoteOn {
                pitch: 72,
                velocity: 100
            })
        );
    }

    #[test]
    fn uppercase_letters_are_commands_not_notes() {
        let keymap = Keymap::default();
        assert_eq!(
            keymap.map(press(KeyCode::Char('M'))),
            Some(Event::ToggleMute)
        );
        assert_eq!(
            keymap.map(press(KeyCode::Char('S'))),
            Some(Event::ToggleSolo)
        );
    }

    #[test]
    fn block_commands_are_bound() {
        let keymap = Keymap::default();
        assert_eq!(keymap.map(press(KeyCode::Char('A'))), Some(Event::AddBlock));
        assert_eq!(
            keymap.map(press(KeyCode::Char('D'))),
            Some(Event::DeleteBlock)
        );
        assert_eq!(
            keymap.map(press(KeyCode::Char('K'))),
            Some(Event::CycleTrackKind)
        );
        assert_eq!(keymap.map(press(KeyCode::Enter)), Some(Event::RenameBlock));
        assert_eq!(keymap.map(press(KeyCode::Up)), Some(Event::MoveTrack(-1)));
        assert_eq!(keymap.map(press(KeyCode::Right)), Some(Event::MoveBlock(1)));
    }

    #[test]
    fn text_mode_swallows_commands_and_types() {
        let mut keymap = Keymap::default();
        keymap.set_text_mode(true);
        assert_eq!(
            keymap.map(press(KeyCode::Char('q'))),
            Some(Event::TextInput('q'))
        );
        assert_eq!(
            keymap.map(press(KeyCode::Char('Z'))),
            Some(Event::TextInput('Z'))
        );
        assert_eq!(
            keymap.map(press(KeyCode::Backspace)),
            Some(Event::TextBackspace)
        );
        assert_eq!(keymap.map(press(KeyCode::Enter)), Some(Event::RenameBlock));
        assert_eq!(keymap.map(press(KeyCode::Esc)), Some(Event::CancelRename));
        assert_eq!(keymap.map(press(KeyCode::Up)), None);
    }

    #[test]
    fn control_c_quits() {
        let keymap = Keymap::default();
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(keymap.map(key), Some(Event::Quit));
    }

    #[test]
    fn key_releases_only_produce_note_offs() {
        let keymap = Keymap::default();
        let mut key = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE);
        key.kind = KeyEventKind::Release;
        assert_eq!(keymap.map(key), Some(Event::NoteOff { pitch: 60 }));

        let mut space = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
        space.kind = KeyEventKind::Release;
        assert_eq!(keymap.map(space), None);
    }
}
