//! The arrangement clipboard: copying, cutting, clearing, pasting and
//! duplicating a range of tracks and beats.

use super::App;
use crate::events::BeatRange;

impl App {
    pub(crate) fn copy_range(&mut self, _range: BeatRange) {}

    pub(crate) fn cut_range(&mut self, _range: BeatRange) {}

    pub(crate) fn delete_range(&mut self, _range: BeatRange) {}

    pub(crate) fn paste_range(&mut self, _track: usize, _start_beats: f32) {}

    pub(crate) fn duplicate_range(&mut self, _range: BeatRange) {}
}
