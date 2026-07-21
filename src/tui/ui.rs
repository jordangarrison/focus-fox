use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};

use super::app::App;
use crate::timer::Phase;

pub fn render(frame: &mut Frame, app: &App) {
    let timer = &app.timer;
    let accent = match timer.phase {
        Phase::Work => Color::LightRed,
        Phase::ShortBreak => Color::LightGreen,
        Phase::LongBreak => Color::LightBlue,
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .title(" 🦊 Focus Fox ")
        .title_alignment(Alignment::Center);
    let inner = block.inner(frame.area());
    frame.render_widget(block, frame.area());

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(1), // phase + session dots
            Constraint::Length(1),
            Constraint::Length(5), // big clock
            Constraint::Length(1),
            Constraint::Length(1), // gauge
            Constraint::Fill(1),
            Constraint::Length(1), // key help
        ])
        .split(inner);

    render_phase_line(frame, rows[1], app, accent);
    render_clock(frame, rows[3], app, accent);
    render_gauge(frame, centered(rows[5], 60), app, accent);
    render_help(frame, rows[7]);
}

fn render_phase_line(frame: &mut Frame, area: Rect, app: &App, accent: Color) {
    let timer = &app.timer;
    let (done, total) = timer.cycle_position();
    let dots: String = (0..total)
        .map(|i| if i < done { '●' } else { '○' })
        .collect();

    let mut spans = vec![
        Span::styled(
            timer.phase.label(),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(dots, Style::default().fg(accent)),
    ];
    if timer.paused {
        spans.push(Span::styled(
            "  ⏸ paused",
            Style::default().fg(Color::Yellow),
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).alignment(Alignment::Center),
        area,
    );
}

fn render_clock(frame: &mut Frame, area: Rect, app: &App, accent: Color) {
    let secs = app.timer.remaining.as_secs();
    let text = format!("{:02}:{:02}", secs / 60, secs % 60);
    let lines = big_text(&text);
    let style = if app.timer.paused {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    };
    let paragraph = Paragraph::new(lines.into_iter().map(Line::from).collect::<Vec<_>>())
        .style(style)
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn render_gauge(frame: &mut Frame, area: Rect, app: &App, accent: Color) {
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(accent))
        .ratio(app.timer.progress().clamp(0.0, 1.0))
        .label("");
    frame.render_widget(gauge, area);
}

fn render_help(frame: &mut Frame, area: Rect) {
    frame.render_widget(
        Paragraph::new("space pause · s skip · r reset · q quit")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        area,
    );
}

/// Center `area` horizontally to at most `width` columns.
fn centered(area: Rect, width: u16) -> Rect {
    let width = width.min(area.width);
    Rect {
        x: area.x + (area.width - width) / 2,
        width,
        ..area
    }
}

/// 3x5 block-character font for digits and colon.
fn big_text(text: &str) -> Vec<String> {
    const GLYPHS: [[&str; 5]; 11] = [
        ["███", "█ █", "█ █", "█ █", "███"], // 0
        ["  █", "  █", "  █", "  █", "  █"], // 1
        ["███", "  █", "███", "█  ", "███"], // 2
        ["███", "  █", "███", "  █", "███"], // 3
        ["█ █", "█ █", "███", "  █", "  █"], // 4
        ["███", "█  ", "███", "  █", "███"], // 5
        ["███", "█  ", "███", "█ █", "███"], // 6
        ["███", "  █", "  █", "  █", "  █"], // 7
        ["███", "█ █", "███", "█ █", "███"], // 8
        ["███", "█ █", "███", "  █", "███"], // 9
        [" ", "█", " ", "█", " "],           // :
    ];

    let mut rows = vec![String::new(); 5];
    for (i, ch) in text.chars().enumerate() {
        let glyph = match ch {
            '0'..='9' => &GLYPHS[ch as usize - '0' as usize],
            ':' => &GLYPHS[10],
            _ => continue,
        };
        for (row, line) in rows.iter_mut().zip(glyph.iter()) {
            if i > 0 {
                row.push_str("  ");
            }
            row.push_str(line);
        }
    }
    rows
}
