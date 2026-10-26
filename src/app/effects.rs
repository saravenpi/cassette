//! Insert effects on a track's chain: adding, removing, reordering, bypassing
//! and setting parameters.
//!
//! Every edit lands on the track's `effects` first, then the whole chain is
//! pushed to the audio thread as one fixed-size [`EffectChain`].

use super::App;
use crate::core::render::RtCommand;
use crate::models::{Effect, EffectChain, EffectKind, MAX_EFFECTS};

impl App {
    /// Insert a fresh effect at `index`, clamped to the chain, or at the end.
    pub(crate) fn add_effect(&mut self, track: usize, kind: EffectKind, index: Option<usize>) {
        let Some(lane) = self.song.tracks.get_mut(track) else {
            self.status = "no track there to add an effect to".into();
            return;
        };
        if lane.effects.len() >= MAX_EFFECTS {
            self.status = format!("track {} already holds {MAX_EFFECTS} effects", track + 1);
            return;
        }
        if lane.effects.iter().any(|effect| effect.kind == kind) {
            self.status = format!("{} is already on this track", kind.name());
            return;
        }
        let at = index.map_or(lane.effects.len(), |index| index.min(lane.effects.len()));
        lane.effects.insert(at, Effect::new(kind));
        self.select_track(track);
        self.push_effects(track);
        self.status = format!("{} added to track {}", kind.name(), track + 1);
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
        self.send(RtCommand::SetEffects {
            track: track as u8,
            chain,
        });
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
    use crate::app::App;
    use crate::events::Event;
    use crate::models::{EffectKind, MAX_EFFECTS};

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
    fn adding_refuses_a_duplicate_a_full_chain_and_a_missing_track() {
        let mut app = headless_app();
        add(&mut app, EffectKind::Reverb, None);
        add(&mut app, EffectKind::Reverb, None);
        assert_eq!(kinds(&app), vec![EffectKind::Reverb]);
        assert_eq!(app.status, "Reverb is already on this track");

        for kind in EffectKind::ALL {
            add(&mut app, kind, None);
        }
        assert_eq!(kinds(&app).len(), MAX_EFFECTS);
        add(&mut app, EffectKind::Eq, None);
        assert!(app.status.contains("already holds"), "{}", app.status);

        app.submit(Event::AddEffect {
            track: 9,
            kind: EffectKind::Eq,
            index: None,
        });
        assert!(app.status.contains("no track"), "{}", app.status);
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
}
