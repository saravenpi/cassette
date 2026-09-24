//! The ratatui renderer: a session track list plus an inspector.
//!
//! It is a pure function of the state — it never mutates the song, the engine
//! or the cursor.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::ui::layout::Pane;
use crate::ui::view::{UiState, bar_beat, meter_bar};

const NAME_WIDTH: usize = 8;
const KIND_WIDTH: usize = 5;
const METER_WIDTH: usize = 12;

const TRACK_PALETTE: [Color; 8] = [
    Color::LightRed,
    Color::LightYellow,
    Color::LightGreen,
    Color::LightCyan,
    Color::LightBlue,
    Color::LightMagenta,
    Color::Red,
    Color::Cyan,
];

fn track_color(index: u8) -> Color {
    TRACK_PALETTE[index as usize % TRACK_PALETTE.len()]
}

fn pane_block(state: &UiState, pane: Pane) -> Block<'static> {
    let focused = state.focus == pane;
    Block::default()
        .borders(Borders::ALL)
        .title(Line::from(pane.title()))
        .border_style(if focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        })
}

/// Pad or truncate `text` to exactly `width` characters.
fn fit(text: &str, width: usize) -> String {
    let mut out: String = text.chars().take(width).collect();
    while out.chars().count() < width {
        out.push(' ');
    }
    out
}

/// Draw a whole frame.
pub fn draw(frame: &mut Frame, state: &UiState) {
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(6),
        Constraint::Length(2),
    ])
    .split(frame.area());

    draw_transport(frame, rows[0], state);

    let panes = Layout::horizontal([Constraint::Min(40), Constraint::Length(30)]).split(rows[1]);

    draw_tracks(frame, panes[0], state);
    draw_inspector(frame, panes[1], state);
    draw_status(frame, rows[2], state);

    if state.show_help {
        draw_help(frame);
    }
}

fn draw_transport(frame: &mut Frame, area: Rect, state: &UiState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Line::from("Transport"));

    let (bar, beat) = bar_beat(state.transport_position_beats);
    let status = if state.playing {
        "▶ playing"
    } else {
        "■ stopped"
    };
    let line = Line::from(vec![
        Span::styled(
            status,
            Style::default().fg(if state.playing {
                Color::Green
            } else {
                Color::Gray
            }),
        ),
        Span::raw("   "),
        Span::styled(
            format!("bpm {:.0}", state.bpm),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw("   "),
        Span::raw(format!("bar {bar}.{beat}")),
        Span::raw("   "),
        Span::raw(format!("loop {}", if state.looping { "on" } else { "off" })),
        Span::raw("   "),
        Span::styled(
            format!("{} @ {} Hz", state.backend, state.sample_rate),
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    frame.render_widget(Paragraph::new(line).block(block), area);
}

fn draw_tracks(frame: &mut Frame, area: Rect, state: &UiState) {
    let block = pane_block(state, Pane::Tracks);
    let inner_height = area.height.saturating_sub(2) as usize;

    let mut lines: Vec<Line> = vec![Line::from(Span::styled(
        format!(
            "   {}  {} {} {} {}",
            fit("track", NAME_WIDTH),
            fit("kind", KIND_WIDTH),
            fit("blocks", 12),
            fit("+", 1),
            "meter",
        ),
        Style::default().fg(Color::DarkGray),
    ))];

    for (index, track) in state.tracks.iter().enumerate() {
        let selected = index == state.cursor.track;
        let color = track_color(track.color_index);
        let mut spans = vec![
            Span::styled(" ●", Style::default().fg(color)),
            Span::raw(format!("{:>2} ", index + 1)),
            Span::styled(
                fit(&track.name, NAME_WIDTH),
                Style::default().fg(color).add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
            ),
            Span::styled(
                format!(" {} ", fit(track.kind.name(), KIND_WIDTH)),
                Style::default().fg(Color::DarkGray),
            ),
        ];

        for (block_index, name) in track.blocks.iter().enumerate() {
            let on_cursor = selected && block_index == state.cursor.block;
            let mut style = Style::default().fg(color);
            if on_cursor {
                style = style.add_modifier(Modifier::REVERSED | Modifier::BOLD);
            }
            spans.push(Span::styled(format!("{name} "), style));
        }

        let mut add_style = Style::default().fg(Color::DarkGray);
        if track.add_slot {
            add_style = add_style.add_modifier(Modifier::REVERSED | Modifier::BOLD);
        }
        spans.push(Span::styled("+ ", add_style));

        spans.push(Span::styled(
            meter_bar(track.level, METER_WIDTH),
            Style::default().fg(Color::Green),
        ));
        spans.push(Span::raw(format!(" {:>4.2}", track.volume)));
        lines.push(Line::from(spans));
    }

    let cursor_line = 1 + state.cursor.track;
    let scroll_y = if inner_height > 0 && cursor_line >= inner_height {
        (cursor_line + 1 - inner_height) as u16
    } else {
        0
    };

    let paragraph = Paragraph::new(lines).block(block).scroll((scroll_y, 0));
    frame.render_widget(paragraph, area);
}

fn draw_inspector(frame: &mut Frame, area: Rect, state: &UiState) {
    let block = pane_block(state, Pane::Inspector);
    let mut lines: Vec<Line> = Vec::new();

    if let Some(track) = state.selected() {
        lines.push(Line::from(Span::styled(
            track.name.clone(),
            Style::default()
                .fg(track_color(track.color_index))
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(format!("kind     {}", track.kind.name())));
        lines.push(Line::from(format!("blocks   {}", track.block_count())));
        lines.push(Line::from(""));
        lines.push(Line::from(format!("preset   {}", track.preset)));
        lines.push(Line::from(format!("waveform {}", track.waveform)));
        lines.push(Line::from(format!("octave   {:+}", track.octave)));
        lines.push(Line::from(format!("attack   {:.3}s", track.attack)));
        lines.push(Line::from(format!("decay    {:.3}s", track.decay)));
        lines.push(Line::from(format!("sustain  {:.2}", track.sustain)));
        lines.push(Line::from(format!("release  {:.3}s", track.release)));
        lines.push(Line::from(""));
        lines.push(Line::from(format!("volume   {:.2}", track.volume)));
        lines.push(Line::from(format!("pan      {:+.2}", track.pan)));
        lines.push(Line::from(format!(
            "mute     {}",
            if track.muted { "on" } else { "off" }
        )));
        lines.push(Line::from(format!(
            "solo     {}",
            if track.solo { "on" } else { "off" }
        )));
    } else {
        lines.push(Line::from("no track selected"));
    }

    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_status(frame: &mut Frame, area: Rect, state: &UiState) {
    let lines = vec![
        Line::from(Span::styled(
            state.status.clone(),
            Style::default().fg(Color::Gray),
        )),
        Line::from(Span::styled(
            state.backend_note.clone(),
            Style::default().fg(Color::DarkGray),
        )),
    ];
    frame.render_widget(Paragraph::new(lines), area);
}

fn draw_help(frame: &mut Frame) {
    let area = centered_rect(64, 72, frame.area());
    frame.render_widget(Clear, area);

    let lines = vec![
        Line::from("Transport"),
        Line::from("  space      play / stop"),
        Line::from("  L          toggle looping"),
        Line::from("  < >        tempo"),
        Line::from("  Q · ctrl-c quit"),
        Line::from(""),
        Line::from("Session"),
        Line::from("  up / down  move between tracks"),
        Line::from("  left/right move between blocks"),
        Line::from("  A          add a block to the track"),
        Line::from("  D          delete the block under the cursor"),
        Line::from("  K          MIDI / audio track"),
        Line::from("  enter      rename the block under the cursor"),
        Line::from("  esc        cancel the rename"),
        Line::from(""),
        Line::from("Mixer"),
        Line::from("  , .        track volume -/+"),
        Line::from("  P N        pan left / right"),
        Line::from("  M S        mute / solo the track"),
        Line::from(""),
        Line::from("Playing"),
        Line::from("  z s x d c… play the lower octave"),
        Line::from("  q 2 w 3 e… play the upper octave"),
        Line::from("  pgup/pgdn  shift the keyboard octave"),
        Line::from("  ?          close this help"),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Line::from("Help"))
        .border_style(Style::default().fg(Color::Yellow));
    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TrackKind;
    use crate::ui::view::{Cursor, TrackView, UiState};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn state() -> UiState {
        UiState {
            bpm: 120.0,
            playing: true,
            looping: true,
            transport_position_beats: 5.0,
            cursor: Cursor { track: 0, block: 0 },
            focus: Pane::Tracks,
            octave: 0,
            tracks: vec![
                TrackView {
                    name: "Midi 1".into(),
                    kind: TrackKind::Midi,
                    preset: "Lead".into(),
                    waveform: "Square".into(),
                    octave: 0,
                    attack: 0.01,
                    decay: 0.12,
                    sustain: 0.6,
                    release: 0.15,
                    volume: 0.8,
                    pan: 0.0,
                    muted: false,
                    solo: false,
                    level: 0.5,
                    blocks: vec!["Verse".into()],
                    add_slot: false,
                    color_index: 0,
                },
                TrackView {
                    name: "Audio 1".into(),
                    kind: TrackKind::Audio,
                    preset: "Pluck".into(),
                    waveform: "Triangle".into(),
                    octave: 0,
                    attack: 0.0,
                    decay: 0.2,
                    sustain: 0.0,
                    release: 0.1,
                    volume: 0.8,
                    pan: 0.0,
                    muted: false,
                    solo: false,
                    level: 0.0,
                    blocks: Vec::new(),
                    add_slot: true,
                    color_index: 1,
                },
            ],
            status: "ready".into(),
            backend: "silent".into(),
            backend_note: "silent: no device".into(),
            sample_rate: 44_100,
            show_help: false,
        }
    }

    fn frame_text(state: &UiState) -> String {
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("test terminal");
        terminal
            .draw(|frame| draw(frame, state))
            .expect("draw a frame");
        let buffer = terminal.backend().buffer();
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn a_frame_draws_the_track_list_and_the_inspector() {
        let text = frame_text(&state());
        assert!(text.contains("Tracks"));
        assert!(text.contains("Inspector"));
        assert!(text.contains("Midi 1"));
        assert!(text.contains("Verse"));
    }

    #[test]
    fn the_help_overlay_draws_too() {
        let mut state = state();
        state.show_help = true;
        assert!(frame_text(&state).contains("add a block"));
    }
}
