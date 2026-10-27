//! Insert effects on a track's chain: adding, removing, reordering, bypassing
//! and setting parameters.
//!
//! Every edit lands on the track's `effects` first, then the whole chain is
//! pushed to the audio thread as one fixed-size [`EffectChain`].

use super::App;
use crate::models::{Effect, EffectChain, EffectKind, MAX_EFFECTS};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TrackShift {
    Inserted(usize),
    Deleted(usize),
    Moved { from: usize, to: usize },
}

impl App {
    pub(crate) fn set_effect_sidechain(
        &mut self,
        track: usize,
        index: usize,
        source: Option<usize>,
    ) {
        let found = self
            .song
            .tracks
            .get(track)
            .is_some_and(|lane| index < lane.effects.len());
        if !found {
            self.status = "no effect there".into();
            return;
        }
        let effect_kind = self.song.tracks[track].effects[index].kind;
        if !effect_kind.takes_sidechain() {
            self.status = format!("{} does not take a sidechain", effect_kind.name());
            return;
        }
        match source {
            Some(source_track) => {
                if source_track == track {
                    self.status = "an effect cannot listen to its own track".into();
                    return;
                }
                if source_track >= self.song.tracks.len() {
                    self.status = "no track there to listen to".into();
                    return;
                }
                self.song.tracks[track].effects[index].sidechain = Some(source_track as u8);
                let source_name = self.song.tracks[source_track].name.clone();
                self.push_effects(track);
                self.status = format!("{} listens to {}", effect_kind.name(), source_name);
            }
            None => {
                self.song.tracks[track].effects[index].sidechain = None;
                self.push_effects(track);
                self.status = format!("{} listens to its own input", effect_kind.name());
            }
        }
    }

    pub(crate) fn remap_sidechains(&mut self, shift: TrackShift) {
        for (track_idx, track) in self.song.tracks.iter_mut().enumerate() {
            for effect in &mut track.effects {
                if let Some(source) = effect.sidechain {
                    let s = source as usize;
                    let next = match shift {
                        TrackShift::Deleted(deleted) => {
                            if s == deleted {
                                None
                            } else if s > deleted {
                                Some((s - 1) as u8)
                            } else {
                                Some(s as u8)
                            }
                        }
                        TrackShift::Inserted(inserted) => {
                            if s >= inserted {
                                Some((s + 1) as u8)
                            } else {
                                Some(s as u8)
                            }
                        }
                        TrackShift::Moved { from, to } => {
                            if s == from {
                                Some(to as u8)
                            } else if from < to && s > from && s <= to {
                                Some((s - 1) as u8)
                            } else if to < from && s >= to && s < from {
                                Some((s + 1) as u8)
                            } else {
                                Some(s as u8)
                            }
                        }
                    };
                    if next == Some(track_idx as u8) {
                        effect.sidechain = None;
                    } else {
                        effect.sidechain = next;
                    }
                }
            }
        }
    }

    /// Insert a fresh effect at `index`, clamped to the chain, or at the end.
    pub(crate) fn add_effect(&mut self, track: usize, kind: EffectKind, index: Option<usize>) {
        let Some(lane_len) = self.song.tracks.get(track).map(|lane| lane.effects.len()) else {
            self.status = "no track there to add an effect to".into();
            return;
        };
        if lane_len >= MAX_EFFECTS {
            self.status = format!("track {} already holds {MAX_EFFECTS} effects", track + 1);
            return;
        }
        let id = self.song.next_effect_id();
        let mut effect = Effect::new(kind);
        effect.id = id;
        let at = index.map_or(lane_len, |index| index.min(lane_len));
        self.song.tracks[track].effects.insert(at, effect);
        self.select_track(track);
        self.push_effects(track);
        self.status = format!("{} added to track {}", kind.name(), track + 1);
    }

    pub(crate) fn paste_effect(
        &mut self,
        track: usize,
        index: Option<usize>,
        kind: EffectKind,
        params: &[f32],
        enabled: bool,
        sidechain: Option<usize>,
    ) {
        let Some(lane_len) = self.song.tracks.get(track).map(|lane| lane.effects.len()) else {
            self.status = "no track there to paste an effect to".into();
            return;
        };
        if lane_len >= MAX_EFFECTS {
            self.status = format!("track {} already holds {MAX_EFFECTS} effects", track + 1);
            return;
        }
        let id = self.song.next_effect_id();
        let mut effect = Effect::new(kind);
        effect.id = id;
        effect.enabled = enabled;
        let spec = kind.spec();
        for (i, p) in effect.params[..spec.len()].iter_mut().enumerate() {
            if let Some(&val) = params.get(i) {
                *p = spec[i].clamp(val);
            }
        }
        if let Some(s) = sidechain
            && s != track
            && s < self.song.tracks.len()
            && kind.takes_sidechain()
        {
            effect.sidechain = Some(s as u8);
        }
        let at = index.map_or(lane_len, |idx| (idx + 1).min(lane_len));
        self.song.tracks[track].effects.insert(at, effect);
        self.select_track(track);
        self.push_effects(track);
        self.status = format!("{} pasted to track {}", kind.name(), track + 1);
    }

    pub(crate) fn duplicate_effect(&mut self, track: usize, index: usize) {
        let Some(lane_len) = self.song.tracks.get(track).map(|lane| lane.effects.len()) else {
            return;
        };
        if index >= lane_len {
            return;
        }
        if lane_len >= MAX_EFFECTS {
            self.status = format!("track {} already holds {MAX_EFFECTS} effects", track + 1);
            return;
        }
        let mut copy = self.song.tracks[track].effects[index];
        copy.id = self.song.next_effect_id();
        let at = index + 1;
        self.song.tracks[track].effects.insert(at, copy);
        self.push_effects(track);
        self.status = format!(
            "duplicated {}",
            self.song.tracks[track].effects[at].kind.name()
        );
    }

    /// Take one effect out of a track's chain.
    pub(crate) fn remove_effect(&mut self, track: usize, index: usize) {
        if self.effect_mut(track, index).is_none() {
            return;
        }
        let removed = self.song.tracks[track].effects.remove(index);
        self.push_effects(track);
        self.status = format!("{} removed", removed.kind.name());
    }

    /// Move one effect to slot `to`, clamped to the chain.
    pub(crate) fn move_effect(&mut self, track: usize, from: usize, to: usize) {
        if self.effect_mut(track, from).is_none() {
            return;
        }
        let effects = &mut self.song.tracks[track].effects;
        let to = to.min(effects.len() - 1);
        let effect = effects.remove(from);
        effects.insert(to, effect);
        self.push_effects(track);
        self.status = format!("{} moved to slot {}", effect.kind.name(), to + 1);
    }

    /// Switch one effect on, or bypass it.
    pub(crate) fn set_effect_enabled(&mut self, track: usize, index: usize, enabled: bool) {
        let Some(effect) = self.effect_mut(track, index) else {
            return;
        };
        effect.enabled = enabled;
        let name = effect.kind.name();
        self.push_effects(track);
        self.status = format!("{name} {}", if enabled { "on" } else { "off" });
    }

    /// Set one parameter, clamped to its spec and rounded when it is a choice.
    pub(crate) fn set_effect_param(
        &mut self,
        track: usize,
        index: usize,
        param: usize,
        value: f32,
    ) {
        if !value.is_finite() {
            self.status = "an effect parameter needs a finite number".into();
            return;
        }
        let Some(effect) = self.effect_mut(track, index) else {
            return;
        };
        let Some(spec) = effect.kind.spec().get(param) else {
            self.status = format!("{} has no parameter {}", effect.kind.name(), param + 1);
            return;
        };
        let value = spec.clamp(value);
        effect.params[param] = value;
        let status = format!("{} {} {value:.2}", effect.kind.name(), spec.name);
        self.push_effects(track);
        self.status = status;
    }

    /// Push one track's whole chain to the audio thread.
    pub(crate) fn push_effects(&mut self, track: usize) {
        let Some(lane) = self.song.tracks.get(track) else {
            return;
        };
        let chain = EffectChain::from_effects(&lane.effects);
        if !self.engine.set_effects(track, &chain) {
            self.resync_pending = true;
        }
    }

    /// The effect an edit names, or `None` with the reason in the status.
    fn effect_mut(&mut self, track: usize, index: usize) -> Option<&mut Effect> {
        let found = self
            .song
            .tracks
            .get(track)
            .is_some_and(|lane| index < lane.effects.len());
        if !found {
            self.status = "no effect there".into();
            return None;
        }
        Some(&mut self.song.tracks[track].effects[index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::events::Event;
    use crate::models::{Effect, EffectKind, MAX_EFFECTS};

    fn headless_app() -> App {
        App::new_silent().expect("silent app should build")
    }

    fn add(app: &mut App, kind: EffectKind, index: Option<usize>) {
        app.submit(Event::AddEffect {
            track: 1,
            kind,
            index,
        });
    }

    fn kinds(app: &App) -> Vec<EffectKind> {
        app.song.tracks[1].effects.iter().map(|e| e.kind).collect()
    }

    fn param(app: &mut App, param: usize, value: f32) {
        app.submit(Event::SetEffectParam {
            track: 1,
            index: 0,
            param,
            value,
        });
    }

    #[test]
    fn adding_inserts_at_the_slot_and_selects_the_track() {
        let mut app = headless_app();
        add(&mut app, EffectKind::Reverb, None);
        add(&mut app, EffectKind::Eq, Some(0));
        add(&mut app, EffectKind::Delay, Some(99));
        assert_eq!(
            kinds(&app),
            vec![EffectKind::Eq, EffectKind::Reverb, EffectKind::Delay]
        );
        assert_eq!(app.cursor.track, 1);
        assert_eq!(app.ui_state().tracks[1].effects.len(), 3);
        assert!(!app.resync_pending);
        assert!(app.status.contains("Delay added"), "{}", app.status);
    }

    #[test]
    fn adding_allows_duplicates_and_refuses_full_chain_and_missing_track() {
        let mut app = headless_app();
        add(&mut app, EffectKind::Reverb, None);
        add(&mut app, EffectKind::Reverb, None);
        assert_eq!(kinds(&app), vec![EffectKind::Reverb, EffectKind::Reverb]);
        assert_ne!(
            app.song.tracks[1].effects[0].id,
            app.song.tracks[1].effects[1].id
        );
        assert_ne!(app.song.tracks[1].effects[0].id, 0);

        for _ in 0..MAX_EFFECTS - 2 {
            add(&mut app, EffectKind::Eq, None);
        }
        assert_eq!(kinds(&app).len(), MAX_EFFECTS);
        add(&mut app, EffectKind::Delay, None);
        assert!(app.status.contains("already holds"), "{}", app.status);

        app.submit(Event::AddEffect {
            track: 9,
            kind: EffectKind::Eq,
            index: None,
        });
        assert!(app.status.contains("no track"), "{}", app.status);
    }

    #[test]
    fn sidechain_can_be_set_cleared_and_refuses_invalid_sources() {
        let mut app = headless_app();
        app.song.tracks[0].name = "Kick".into();
        add(&mut app, EffectKind::Compressor, None);

        app.submit(Event::SetEffectSidechain {
            track: 1,
            index: 0,
            source: Some(0),
        });
        assert_eq!(app.song.tracks[1].effects[0].sidechain, Some(0));
        assert_eq!(app.status, "Compressor listens to Kick");

        app.submit(Event::SetEffectSidechain {
            track: 1,
            index: 0,
            source: None,
        });
        assert_eq!(app.song.tracks[1].effects[0].sidechain, None);
        assert_eq!(app.status, "Compressor listens to its own input");

        app.submit(Event::SetEffectSidechain {
            track: 1,
            index: 0,
            source: Some(1),
        });
        assert_eq!(app.song.tracks[1].effects[0].sidechain, None);
        assert_eq!(app.status, "an effect cannot listen to its own track");

        app.submit(Event::SetEffectSidechain {
            track: 1,
            index: 0,
            source: Some(99),
        });
        assert_eq!(app.song.tracks[1].effects[0].sidechain, None);
        assert_eq!(app.status, "no track there to listen to");

        add(&mut app, EffectKind::Reverb, None);
        app.submit(Event::SetEffectSidechain {
            track: 1,
            index: 1,
            source: Some(0),
        });
        assert_eq!(app.song.tracks[1].effects[1].sidechain, None);
        assert_eq!(app.status, "Reverb does not take a sidechain");
    }

    #[test]
    fn sidechain_remap_shifts_and_clears() {
        let mut app = headless_app();
        add(&mut app, EffectKind::Compressor, None);
        app.song.tracks[1].effects[0].sidechain = Some(3);

        app.remap_sidechains(TrackShift::Inserted(2));
        assert_eq!(app.song.tracks[1].effects[0].sidechain, Some(4));

        app.remap_sidechains(TrackShift::Deleted(4));
        assert_eq!(app.song.tracks[1].effects[0].sidechain, None);

        app.song.tracks[1].effects[0].sidechain = Some(3);
        app.remap_sidechains(TrackShift::Deleted(1));
        assert_eq!(app.song.tracks[1].effects[0].sidechain, Some(2));
    }

    #[test]
    fn sanitize_assigns_unique_nonzero_effect_ids() {
        let mut app = headless_app();
        let mut e1 = Effect::new(EffectKind::Reverb);
        let mut e2 = Effect::new(EffectKind::Delay);
        e1.id = 0;
        e2.id = 0;
        app.song.tracks[0].effects = vec![e1, e2];
        let problems = app.song.sanitize().expect("song should sanitize");
        assert!(problems.iter().any(|p| p.contains("unique effect ids")));
        assert_ne!(app.song.tracks[0].effects[0].id, 0);
        assert_ne!(app.song.tracks[0].effects[1].id, 0);
        assert_ne!(
            app.song.tracks[0].effects[0].id,
            app.song.tracks[0].effects[1].id
        );
    }

    #[test]
    fn removing_and_moving_keep_the_rest_in_order() {
        let mut app = headless_app();
        for kind in [EffectKind::Eq, EffectKind::Filter, EffectKind::Reverb] {
            add(&mut app, kind, None);
        }
        app.submit(Event::MoveEffect {
            track: 1,
            from: 0,
            to: 99,
        });
        assert_eq!(
            kinds(&app),
            vec![EffectKind::Filter, EffectKind::Reverb, EffectKind::Eq]
        );
        app.submit(Event::RemoveEffect { track: 1, index: 1 });
        assert_eq!(kinds(&app), vec![EffectKind::Filter, EffectKind::Eq]);
        app.submit(Event::RemoveEffect { track: 1, index: 5 });
        assert_eq!(app.status, "no effect there");
    }

    #[test]
    fn an_effect_can_be_bypassed() {
        let mut app = headless_app();
        add(&mut app, EffectKind::Saturator, None);
        app.submit(Event::SetEffectEnabled {
            track: 1,
            index: 0,
            enabled: false,
        });
        assert!(!app.song.tracks[1].effects[0].enabled);
        assert_eq!(app.status, "Saturator off");
    }

    #[test]
    fn a_param_is_clamped_rounded_and_checked() {
        let mut app = headless_app();
        add(&mut app, EffectKind::Filter, None);
        param(&mut app, 1, 1e9);
        param(&mut app, 0, 1.4);
        assert_eq!(app.song.tracks[1].effects[0].params[1], 20_000.0);
        assert_eq!(app.song.tracks[1].effects[0].params[0], 1.0);
        let before = app.song.tracks[1].effects[0];
        param(&mut app, 3, 0.5);
        assert!(app.status.contains("no parameter"), "{}", app.status);
        param(&mut app, 2, f32::NAN);
        assert!(app.status.contains("finite"), "{}", app.status);
        assert_eq!(app.song.tracks[1].effects[0], before);
    }

    #[test]
    fn a_dial_drag_is_one_undo_step() {
        let mut app = headless_app();
        add(&mut app, EffectKind::Reverb, None);
        for value in [0.6, 0.7, 0.8] {
            param(&mut app, 0, value);
        }
        assert_eq!(app.song.tracks[1].effects[0].params[0], 0.8);
        app.submit(Event::Undo);
        assert_eq!(app.song.tracks[1].effects[0].params[0], 0.5);
        app.submit(Event::Undo);
        assert!(app.song.tracks[1].effects.is_empty(), "undo of the add");
        app.submit(Event::Redo);
        assert_eq!(kinds(&app), vec![EffectKind::Reverb]);
    }

    #[test]
    fn paste_and_duplicate_effects_preserve_values() {
        let mut app = headless_app();
        app.handle(Event::PasteEffect {
            track: 0,
            index: None,
            kind: EffectKind::Filter,
            params: vec![1.0, 1200.0, 0.7],
            enabled: false,
            sidechain: None,
        });
        assert_eq!(app.song.tracks[0].effects.len(), 1);
        let eff = &app.song.tracks[0].effects[0];
        assert_eq!(eff.kind, EffectKind::Filter);
        assert!(!eff.enabled);
        assert_eq!(eff.params[1], 1200.0);

        app.handle(Event::DuplicateEffect { track: 0, index: 0 });
        assert_eq!(app.song.tracks[0].effects.len(), 2);
        assert_ne!(
            app.song.tracks[0].effects[0].id,
            app.song.tracks[0].effects[1].id
        );
        assert_eq!(app.song.tracks[0].effects[1].params[1], 1200.0);
        assert!(!app.song.tracks[0].effects[1].enabled);
    }
}
