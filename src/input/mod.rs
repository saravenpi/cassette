//! Input adapters. This is the only layer that talks to hardware.
//!
//! The MIDI and OSC adapters translate their protocols into [`Event`]s, the same
//! vocabulary the webview speaks, so new devices need no changes in `core`.

use crate::events::Event;

/// MIDI input adapter that translates messages into DAW events.
pub struct MidiInputAdapter {
    channel_filter: Option<u8>,
}

/// A single MIDI message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MidiMessage {
    pub channel: u8,
    /// Status nibble: `0x9` note on, `0x8` note off.
    pub status: u8,
    pub note: u8,
    pub velocity: u8,
}

impl MidiInputAdapter {
    /// Create an adapter that listens to every channel.
    pub fn new() -> Self {
        Self {
            channel_filter: None,
        }
    }

    /// Create an adapter restricted to one MIDI channel.
    pub fn with_channel_filter(channel: u8) -> Self {
        Self {
            channel_filter: Some(channel),
        }
    }

    /// Translate a MIDI message, or `None` if it is filtered or unsupported.
    pub fn translate(&self, message: &MidiMessage) -> Option<Event> {
        if let Some(filter) = self.channel_filter
            && message.channel != filter
        {
            return None;
        }
        let mut pitch = message.note;
        let mut velocity = message.velocity;

        match message.status {
            0x9 => {
                // Note-on with zero velocity is a note-off by spec.
                if velocity == 0 {
                    Some(Event::NoteOff { pitch })
                } else {
                    pitch &= 0x7f;
                    velocity &= 0x7f;
                    Some(Event::NoteOn { pitch, velocity })
                }
            }
            0x8 => Some(Event::NoteOff { pitch }),
            _ => None,
        }
    }
}

impl Default for MidiInputAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// OSC input adapter that translates control messages into DAW events.
pub struct OscInputAdapter {
    registered: Vec<String>,
}

/// A single OSC message.
#[derive(Debug, Clone, PartialEq)]
pub enum OscMessage {
    /// A continuous controller value.
    Control(String, f32),
    /// A trigger carrying one or more arguments.
    Trigger { path: String, args: Vec<f32> },
}

impl OscInputAdapter {
    /// Create an adapter with no registered paths.
    pub fn new() -> Self {
        Self {
            registered: Vec::new(),
        }
    }

    /// Register interest in an OSC path.
    pub fn register_callback(&mut self, path: String) {
        self.registered.push(path);
    }

    /// Whether a path has been registered.
    pub fn is_registered(&self, path: &str) -> bool {
        self.registered.iter().any(|p| p == path)
    }

    /// Route an OSC message into an [`Event::OscControl`].
    pub fn route(&self, message: &OscMessage) -> Option<Event> {
        let (address, value) = match message {
            OscMessage::Control(address, value) => (address.clone(), *value),
            OscMessage::Trigger { path, args } => (path.clone(), *args.first()?),
        };
        Some(Event::OscControl {
            address,
            value: value.clamp(0.0, 1.0),
        })
    }
}

impl Default for OscInputAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midi_note_on_zero_velocity_is_a_note_off() {
        let adapter = MidiInputAdapter::new();
        let message = MidiMessage {
            channel: 1,
            status: 0x9,
            note: 60,
            velocity: 0,
        };
        assert_eq!(
            adapter.translate(&message),
            Some(Event::NoteOff { pitch: 60 })
        );
    }

    #[test]
    fn midi_unsupported_status_is_ignored() {
        let adapter = MidiInputAdapter::new();
        let message = MidiMessage {
            channel: 1,
            status: 0xb,
            note: 60,
            velocity: 10,
        };
        assert!(adapter.translate(&message).is_none());
    }

    #[test]
    fn osc_values_are_clamped() {
        let adapter = OscInputAdapter::new();
        let event = adapter
            .route(&OscMessage::Control("/x".into(), 9.0))
            .expect("routed");
        assert_eq!(
            event,
            Event::OscControl {
                address: "/x".into(),
                value: 1.0
            }
        );
    }

    #[test]
    fn osc_trigger_without_args_is_ignored() {
        let adapter = OscInputAdapter::new();
        assert!(
            adapter
                .route(&OscMessage::Trigger {
                    path: "/x".into(),
                    args: vec![]
                })
                .is_none()
        );
    }
}
