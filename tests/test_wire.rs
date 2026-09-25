//! The webview-to-engine wire contract.
//!
//! `Event` renames variants to camelCase, but that alone does not rename the
//! fields inside struct variants: without `rename_all_fields` serde expects
//! `start_beats` where the frontend sends `startBeats`, and every multi-word
//! field is rejected as "missing field". These tests pin the JSON the frontend
//! actually sends.

use zboub69::events::Event;

/// Deserialize one payload, reporting the failure rather than panicking inside
/// the assertion so the message names the offending event.
fn parse(payload: &str) -> Event {
    serde_json::from_str(payload).unwrap_or_else(|error| panic!("{payload} should parse: {error}"))
}

#[test]
fn a_struct_variant_reads_camel_case_fields() {
    let event = parse(r#"{"createBlock":{"track":0,"startBeats":4.0,"lengthBeats":2.0}}"#);
    assert_eq!(
        event,
        Event::CreateBlock {
            track: 0,
            start_beats: 4.0,
            length_beats: 2.0,
        }
    );
}

#[test]
fn a_sample_import_reads_camel_case_fields() {
    let event =
        parse(r#"{"importSample":{"track":1,"path":"/samples/kick.wav","startBeats":8.0}}"#);
    assert_eq!(
        event,
        Event::ImportSample {
            track: 1,
            path: "/samples/kick.wav".into(),
            start_beats: 8.0,
        }
    );
}

#[test]
fn a_place_block_reads_camel_case_fields() {
    let event = parse(r#"{"placeBlock":{"track":0,"block":1,"targetTrack":2,"startBeats":16.0}}"#);
    assert_eq!(
        event,
        Event::PlaceBlock {
            track: 0,
            block: 1,
            target_track: 2,
            start_beats: 16.0,
        }
    );
}

#[test]
fn a_set_track_mix_reads_camel_case_fields() {
    let event =
        parse(r#"{"setTrackMix":{"track":0,"volume":0.8,"pan":-0.5,"muted":true,"solo":false}}"#);
    assert_eq!(
        event,
        Event::SetTrackMix {
            track: 0,
            volume: 0.8,
            pan: -0.5,
            muted: true,
            solo: false,
        }
    );
}

#[test]
fn a_unit_variant_is_a_bare_camel_case_string() {
    assert_eq!(parse(r#""togglePlay""#), Event::TogglePlay);
    assert_eq!(parse(r#""cycleTrackKind""#), Event::CycleTrackKind);
}

#[test]
fn an_event_survives_a_round_trip() {
    let event = Event::ImportSample {
        track: 0,
        path: "/samples/kick.wav".into(),
        start_beats: 1.5,
    };
    let json = serde_json::to_string(&event).expect("an event should serialize");
    assert_eq!(
        json,
        r#"{"importSample":{"track":0,"path":"/samples/kick.wav","startBeats":1.5}}"#
    );
    assert_eq!(parse(&json), event);
}
