//! The ratatui renderer: a three-pane layout driven by [`UiState`].
//!
//! It is a pure function of the state — it never mutates the song, the engine
//! or the cursor.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::ui::layout::Pane;
use crate::ui::view::{UiState, meter_bar};
use crate::utils::frequency::note_name;

const NAME_WIDTH: usize = 9;

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

    let panes = Layout::horizontal([
        Constraint::Length(30),
        Constraint::Min(24),
        Constraint::Length(28),
    ])
    .split(rows[1]);

    draw_mixer(frame, panes[0], state);
    draw_pattern(frame, panes[1], state);
    draw_inspector(frame, panes[2], state);
    draw_status(frame, rows[2], state);

    if state.show_help {
        draw_help(frame);
    }
}

fn draw_transport(frame: &mut Frame, area: Rect, state: &UiState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Line::from("Transport"));

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
        Span::styled(format!("bpm {:.0}", state.bpm), Style::default().fg(Color::Cyan)),
        Span::raw("   "),
        Span::raw(format!(
            "step {}/{}",
            state.position_step + 1,
            state.length_steps
        )),
        Span::raw("   "),
        Span::raw(format!("loop {}", if state.looping { "on" } else { "off" })),
        Span::raw("   "),
        Span::styled(
            format!("{} @ {} Hz", state.backend, state.sample_rate),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            if state.recording { "   ● REC" } else { "" },
            Style::default().fg(Color::Red),
        ),
    ]);

    frame.render_widget(Paragraph::new(line).block(block), area);
}

fn draw_mixer(frame: &mut Frame, area: Rect, state: &UiState) {
    let block = pane_block(state, Pane::Mixer);
    let inner_width = area.width.saturating_sub(4) as usize;
    let mut lines: Vec<Line> = Vec::new();

    for (index, track) in state.tracks.iter().enumerate() {
        let selected = index == state.cursor.track;
        let color = track_color(track.color_index);
        let mut label = format!("{:>2} {}", index + 1, fit(&track.name, 7));
        label.push(if track.muted { 'M' } else { '-' });
        label.push(if track.solo { 'S' } else { '-' });

        let bar_width = inner_width.saturating_sub(label.chars().count() + 7).max(4);
        let bar = meter_bar(track.level, bar_width);
        let peak_pos = ((track.peak.clamp(0.0, 1.0) * bar_width as f32).round() as usize)
            .min(bar_width.saturating_sub(1));

        let mut spans = vec![Span::styled(
            label,
            if selected {
                Style::default().fg(color).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(color)
            },
        )];
        spans.push(Span::raw(" "));
        for (position, glyph) in bar.chars().enumerate() {
            let style = if position == peak_pos && track.peak > 0.01 {
                Style::default().fg(Color::Red)
            } else if glyph == '#' {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            spans.push(Span::styled(glyph.to_string(), style));
        }
        spans.push(Span::raw(format!(" {:>4.2}", track.volume)));
        lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_pattern(frame: &mut Frame, area: Rect, state: &UiState) {
    let block = pane_block(state, Pane::Pattern);
    let inner_height = area.height.saturating_sub(2) as usize;
    let inner_width = area.width.saturating_sub(2) as usize;

    let mut lines: Vec<Line> = Vec::new();

    let mut ruler = vec![Span::styled(
        " ".repeat(NAME_WIDTH + 1),
        Style::default().fg(Color::DarkGray),
    )];
    for step in 0..state.length_steps {
        let label = if step % 4 == 0 {
            format!("{:>2}", step / 4 + 1)
        } else {
            " ·".to_string()
        };
        ruler.push(Span::styled(label, Style::default().fg(Color::DarkGray)));
    }
    lines.push(Line::from(ruler));

    for (track_index, track) in state.tracks.iter().enumerate() {
        let mut spans = vec![
            Span::styled(
                fit(&track.name, NAME_WIDTH),
                Style::default()
                    .fg(track_color(track.color_index))
                    .add_modifier(if track_index == state.cursor.track {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
            Span::raw(" "),
        ];

        for (step_index, step) in track.steps.iter().enumerate().take(state.length_steps) {
            let glyph = if step.active { "● " } else { "· " };
            let mut style = Style::default().fg(if step.active {
                track_color(track.color_index)
            } else {
                Color::DarkGray
            });
            if state.playing && step_index == state.position_step {
                style = style.bg(Color::Rgb(40, 40, 64));
            }
            if track_index == state.cursor.track && step_index == state.cursor.step {
                style = style.add_modifier(Modifier::REVERSED | Modifier::BOLD);
            }
            spans.push(Span::styled(glyph.to_string(), style));
        }
        lines.push(Line::from(spans));
    }

    // Keep the cursor visible without owning any scroll state.
    let cursor_line = 1 + state.cursor.track;
    let scroll_y = if inner_height > 0 && cursor_line >= inner_height {
        (cursor_line + 1 - inner_height) as u16
    } else {
        0
    };
    let cursor_column = NAME_WIDTH + 1 + state.cursor.step * 2;
    let scroll_x = if inner_width > 0 && cursor_column + 2 > inner_width {
        (cursor_column + 2 - inner_width) as u16
    } else {
        0
    };

    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((scroll_y, scroll_x));
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
        lines.push(Line::from(format!("preset   {}", track.preset)));
        lines.push(Line::from(format!("waveform {}", track.waveform)));
        lines.push(Line::from(format!("octave   {:+}", track.octave)));
        lines.push(Line::from(format!("attack   {:.3}s", track.attack)));
        lines.push(Line::from(format!("decay    {:.3}s", track.decay)));
        lines.push(Line::from(format!("sustain  {:.2}", track.sustain)));
        lines.push(Line::from(format!("release  {:.3}s", track.release)));
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
        lines.push(Line::from(""));
        if let Some(pitch) = track.pitch_at(state.cursor.step) {
            lines.push(Line::from(format!(
                "step {}  {}",
                state.cursor.step + 1,
                note_name(pitch)
            )));
        }
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
            "space play · enter step · arrows move · [ ] pitch · - = velocity · , . volume · M mute · S solo · W wave · I preset · R rec · L loop · E export · + - length · ? help · Q quit",
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
        Line::from("  Q · ctrl-c quit"),
        Line::from(""),
        Line::from("Editing"),
        Line::from("  arrows     move the pattern cursor"),
        Line::from("  enter      toggle the step"),
        Line::from("  [ ]        transpose the step"),
        Line::from("  - =        velocity -/+"),
        Line::from("  + -        pattern length +/-"),
        Line::from("  , .        track volume -/+"),
        Line::from("  P N        pan left / right"),
        Line::from("  M S        mute / solo the track"),
        Line::from("  W I        cycle waveform / preset"),
        Line::from(""),
        Line::from("Playing & recording"),
        Line::from("  z s x d c… play the lower octave"),
        Line::from("  q 2 w 3 e… play the upper octave"),
        Line::from("  pgup/pgdn  shift the keyboard octave"),
        Line::from("  R          arm performance recording"),
        Line::from("  L          toggle looping"),
        Line::from("  E          bounce to zboub69-mix.wav"),
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
