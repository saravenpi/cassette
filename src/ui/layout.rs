//! The three-pane focus model.

/// Which pane currently owns the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    /// The step grid.
    Pattern,
    /// The track list and meters.
    Mixer,
    /// The selected track's instrument.
    Inspector,
}

impl Pane {
    /// All panes, in tab order.
    pub const ALL: [Pane; 3] = [Pane::Pattern, Pane::Mixer, Pane::Inspector];

    /// Title shown in the pane's border.
    pub const fn title(self) -> &'static str {
        match self {
            Pane::Pattern => "Pattern",
            Pane::Mixer => "Mixer",
            Pane::Inspector => "Instrument",
        }
    }

    /// The next pane in tab order, wrapping around.
    pub fn next(self) -> Self {
        let index = Self::ALL.iter().position(|p| *p == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_cycles_through_every_pane() {
        let mut pane = Pane::Pattern;
        for _ in 0..Pane::ALL.len() {
            pane = pane.next();
        }
        assert_eq!(pane, Pane::Pattern);
    }
}
