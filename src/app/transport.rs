//! The transport, the block-name prompt and the notes the keyboard is holding.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use super::App;
use crate::core::render::RtCommand;
use crate::utils::time::Tempo;

/// How long a played note rings when the keyboard cannot report releases.
const HELD_NOTE_MS: u64 = 500;
/// Longest block name the prompt accepts, in characters.
const MAX_NAME_LEN: usize = 32;

/// A note waiting to be released.
pub(crate) struct HeldNote {
    track: u8,
    pitch: u8,
    deadline: Instant,
}

impl App {
    /// Start playing, or stop if already playing.
    pub(crate) fn toggle_play(&mut self) {
        if self.engine.is_playing() {
            self.stop();
        } else {
            self.send(RtCommand::Play);
            self.status = "playing".into();
        }
    }

    /// Stop and rewind.
    pub(crate) fn stop(&mut self) {
        self.send(RtCommand::Stop);
        self.status = "stopped".into();
    }

    /// Enable or disable looping.
    pub(crate) fn toggle_loop(&mut self) {
        let looping = !self.engine.shared().looping.load(Ordering::Relaxed);
        self.send(RtCommand::SetLooping { looping });
        self.status = format!("looping {}", if looping { "on" } else { "off" });
    }

    /// Change the tempo.
    pub(crate) fn nudge_bpm(&mut self, delta: f32) {
        self.song.set_bpm(self.song.bpm() + delta);
        self.send(RtCommand::SetBpm {
            bpm: self.song.bpm(),
        });
        self.status = format!("bpm {:.0}", self.song.bpm());
    }

    /// Transpose the play keyboard.
    pub(crate) fn nudge_octave(&mut self, delta: i32) {
        self.octave = (self.octave + delta).clamp(-4, 4);
        self.status = format!("keyboard octave {:+}", self.octave);
    }

    /// Open the block-name prompt, or commit the one already open.
    pub(crate) fn rename_block(&mut self) {
        if self.rename.is_some() {
            self.rename_commit();
            return;
        }
        let Some(name) = self
            .song
            .tracks
            .get(self.cursor.track)
            .and_then(|track| track.blocks.get(self.cursor.block))
            .map(|block| block.name.clone())
        else {
            self.status = "move to a block to rename it".into();
            return;
        };
        self.rename = Some(String::new());
        self.status = format!("renaming {name}: type a new name, enter to confirm, esc to cancel");
    }

    /// Replace the block's name with what the prompt holds.
    pub(crate) fn rename_commit(&mut self) {
        let Some(buffer) = self.rename.take() else {
            return;
        };
        let name = buffer.trim().to_string();
        if name.is_empty() {
            self.status = "rename left unchanged".into();
            return;
        }
        if let Some(block) = self
            .song
            .tracks
            .get_mut(self.cursor.track)
            .and_then(|track| track.blocks.get_mut(self.cursor.block))
        {
            block.name = name.clone();
        }
        self.status = format!("block renamed to {name}");
    }

    /// Abandon the block-name prompt.
    pub(crate) fn cancel_rename(&mut self) {
        if self.rename.take().is_some() {
            self.status = "rename cancelled".into();
        }
    }

    /// A character typed into the block-name prompt.
    pub(crate) fn type_character(&mut self, character: char) {
        let Some(buffer) = self.rename.as_mut() else {
            return;
        };
        if buffer.chars().count() >= MAX_NAME_LEN {
            return;
        }
        buffer.push(character);
        self.status = format!("rename: {buffer}");
    }

    /// A backspace in the block-name prompt.
    pub(crate) fn backspace(&mut self) {
        if let Some(buffer) = self.rename.as_mut() {
            buffer.pop();
            self.status = format!("rename: {buffer}");
        }
    }

    /// A note was played on the keyboard.
    pub(crate) fn play_note(&mut self, pitch: u8, velocity: u8) {
        let track = self.cursor.track as u8;
        self.send(RtCommand::NoteOn {
            track,
            pitch,
            velocity,
        });
        self.held.push(HeldNote {
            track,
            pitch,
            deadline: Instant::now() + Duration::from_millis(HELD_NOTE_MS),
        });
    }

    /// Release every held note at `pitch`. The cursor may have moved since the
    /// key went down, so the note-off goes to the track that took the note-on.
    pub(crate) fn release_note(&mut self, pitch: u8) {
        let mut index = 0;
        while index < self.held.len() {
            if self.held[index].pitch == pitch {
                let note = self.held.swap_remove(index);
                self.send(RtCommand::NoteOff {
                    track: note.track,
                    pitch,
                });
            } else {
                index += 1;
            }
        }
    }

    /// Release notes the keyboard never reported a release for.
    pub(crate) fn release_expired_notes(&mut self) {
        let now = Instant::now();
        let mut index = 0;
        while index < self.held.len() {
            if self.held[index].deadline <= now {
                let note = self.held.swap_remove(index);
                self.send(RtCommand::NoteOff {
                    track: note.track,
                    pitch: note.pitch,
                });
            } else {
                index += 1;
            }
        }
    }

    /// Frames in one beat at the current tempo and the engine's rate.
    pub(crate) fn frames_per_beat(&self) -> u64 {
        Tempo::new(self.song.bpm()).frames_per_beat(self.engine.sample_rate())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::Event;

    fn headless_app() -> App {
        App::new_silent().expect("silent app should build")
    }

    #[test]
    fn a_note_is_released_on_the_track_that_played_it() {
        let mut app = headless_app();
        app.handle(Event::NoteOn {
            pitch: 60,
            velocity: 100,
        });
        app.handle(Event::MoveTrack(1));
        app.handle(Event::NoteOff { pitch: 60 });
        assert!(
            app.held.is_empty(),
            "moving the cursor must not strand the held note"
        );
    }

    #[test]
    fn a_held_note_expires_on_its_own() {
        let mut app = headless_app();
        app.handle(Event::NoteOn {
            pitch: 64,
            velocity: 90,
        });
        app.held[0].deadline = Instant::now();
        app.tick();
        assert!(app.held.is_empty());
    }

    #[test]
    fn renaming_a_block_replaces_its_name() {
        let mut app = headless_app();
        app.handle(Event::AddBlock);
        app.cursor.block = 0;
        app.handle(Event::RenameBlock);
        assert!(app.rename.is_some());
        for character in "Verse".chars() {
            app.handle(Event::TextInput(character));
        }
        app.handle(Event::RenameBlock);
        assert_eq!(app.song.tracks[0].blocks[0].name, "Verse");
        assert!(app.rename.is_none());
    }

    #[test]
    fn the_name_prompt_forgets_what_it_did_not_keep() {
        let mut app = headless_app();
        app.handle(Event::AddBlock);
        app.cursor.block = 0;
        app.handle(Event::RenameBlock);
        app.handle(Event::TextInput('X'));
        app.handle(Event::TextBackspace);
        assert_eq!(app.rename.as_deref(), Some(""));
        for _ in 0..MAX_NAME_LEN + 4 {
            app.handle(Event::TextInput('A'));
        }
        assert_eq!(app.rename.as_deref().map(str::len), Some(MAX_NAME_LEN));
    }

    #[test]
    fn cancelling_a_rename_keeps_the_old_name() {
        let mut app = headless_app();
        app.handle(Event::AddBlock);
        app.handle(Event::RenameBlock);
        app.handle(Event::TextInput('X'));
        app.handle(Event::CancelRename);
        assert_eq!(app.song.tracks[0].blocks[0].name, "1");
    }

    #[test]
    fn the_octave_never_leaves_the_keyboard_range() {
        let mut app = headless_app();
        for _ in 0..9 {
            app.handle(Event::OctaveDelta(1));
        }
        assert_eq!(app.octave, 4);
        assert_eq!(app.ui_state().octave, 4);
    }

    #[test]
    fn the_tempo_moves_and_reaches_the_engine() {
        let mut app = headless_app();
        app.handle(Event::BpmDelta(8.0));
        assert_eq!(app.song.bpm(), 128.0);
        assert!(app.ui_state().status.contains("128"));
    }

    #[test]
    fn play_then_stop_reaches_the_engine() {
        let mut app = headless_app();
        app.handle(Event::TogglePlay);
        assert!(app.status.contains("playing"), "status: {}", app.status);
        app.handle(Event::Stop);
        assert!(app.status.contains("stopped"), "status: {}", app.status);
    }
}
