//! Phase 4.3 — MIDI and OSC input adapters.

use zboub69::events::Event;
use zboub69::input::{MidiInputAdapter, MidiMessage, OscInputAdapter, OscMessage};

#[test]
fn midi_note_on_translates_to_a_note_on_event() {
    let adapter = MidiInputAdapter::new();
    let message = MidiMessage {
        channel: 1,
        status: 0x9,
        note: 64,
        velocity: 80,
    };
    assert_eq!(
        adapter.translate(&message),
        Some(Event::NoteOn {
            pitch: 64,
            velocity: 80
        })
    );
}

#[test]
fn midi_note_off_translates_to_a_note_off_event() {
    let adapter = MidiInputAdapter::new();
    let message = MidiMessage {
        channel: 2,
        status: 0x8,
        note: 67,
        velocity: 50,
    };
    assert_eq!(
        adapter.translate(&message),
        Some(Event::NoteOff { pitch: 67 })
    );
}

#[test]
fn midi_note_on_with_zero_velocity_is_a_note_off() {
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
fn midi_channel_filtering_drops_other_channels() {
    let adapter = MidiInputAdapter::with_channel_filter(1);
    let message = MidiMessage {
        channel: 2,
        status: 0x9,
        note: 60,
        velocity: 100,
    };
    assert!(adapter.translate(&message).is_none());
}

#[test]
fn midi_unsupported_status_is_ignored() {
    let adapter = MidiInputAdapter::new();
    let message = MidiMessage {
        channel: 1,
        status: 0xb,
        note: 60,
        velocity: 20,
    };
    assert!(adapter.translate(&message).is_none());
}

#[test]
fn osc_control_message_routes_to_a_control_event() {
    let adapter = OscInputAdapter::new();
    let message = OscMessage::Control("/tempo".into(), 0.5);
    assert_eq!(
        adapter.route(&message),
        Some(Event::OscControl {
            address: "/tempo".into(),
            value: 0.5
        })
    );
}

#[test]
fn osc_trigger_uses_its_first_argument() {
    let adapter = OscInputAdapter::new();
    let message = OscMessage::Trigger {
        path: "/play/start".into(),
        args: vec![0.25, 0.75],
    };
    assert_eq!(
        adapter.route(&message),
        Some(Event::OscControl {
            address: "/play/start".into(),
            value: 0.25
        })
    );
}

#[test]
fn osc_values_are_clamped_into_unit_range() {
    let adapter = OscInputAdapter::new();
    let over = OscMessage::Control("/x".into(), 2.5);
    assert_eq!(
        adapter.route(&over),
        Some(Event::OscControl {
            address: "/x".into(),
            value: 1.0
        })
    );
    let under = OscMessage::Control("/x".into(), -3.0);
    assert_eq!(
        adapter.route(&under),
        Some(Event::OscControl {
            address: "/x".into(),
            value: 0.0
        })
    );
}

#[test]
fn osc_trigger_without_arguments_is_ignored() {
    let adapter = OscInputAdapter::new();
    let message = OscMessage::Trigger {
        path: "/x".into(),
        args: vec![],
    };
    assert!(adapter.route(&message).is_none());
}

#[test]
fn osc_paths_can_be_registered() {
    let mut adapter = OscInputAdapter::new();
    adapter.register_callback("/knob/1".into());
    assert!(adapter.is_registered("/knob/1"));
    assert!(!adapter.is_registered("/knob/2"));
}
