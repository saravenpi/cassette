//! Insert effects on a track's chain: adding, removing, reordering, bypassing
//! and setting parameters.

use super::App;
use crate::models::EffectKind;

impl App {
    pub(crate) fn add_effect(&mut self, _track: usize, _kind: EffectKind, _index: Option<usize>) {}

    pub(crate) fn remove_effect(&mut self, _track: usize, _index: usize) {}

    pub(crate) fn move_effect(&mut self, _track: usize, _from: usize, _to: usize) {}

    pub(crate) fn set_effect_enabled(&mut self, _track: usize, _index: usize, _enabled: bool) {}

    pub(crate) fn set_effect_param(
        &mut self,
        _track: usize,
        _index: usize,
        _param: usize,
        _value: f32,
    ) {
    }
}
