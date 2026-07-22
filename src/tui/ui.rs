use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::canvas::{Canvas, Points};
use ratatui::widgets::{Block, Borders, Paragraph};

use super::app::{App, MENU_ITEMS, Screen};
use crate::timer::{Phase, Timer};

const FOX: Color = Color::LightYellow;

pub fn render(frame: &mut Frame, app: &App) {
    match (&app.screen, app.alert) {
        (Screen::Timer(timer), Some(phase)) => render_alert(frame, timer, phase),
        (Screen::Timer(timer), None) => render_timer(frame, timer),
        (Screen::Menu { selected }, _) => render_menu(frame, app, *selected),
    }
}

fn frame_block(frame: &mut Frame, accent: Color) -> Rect {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .title(" 🦊 Focus Fox ")
        .title_alignment(Alignment::Center);
    let inner = block.inner(frame.area());
    frame.render_widget(block, frame.area());
    inner
}

// --- Menu screen ---

const FOX_ART: &str = "\
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⣀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣀⡀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣾⠙⠻⢶⣄⡀⠀⠀⠀⢀⣤⠶⠛⠛⡇⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢹⣇⠀⠀⣙⣿⣦⣤⣴⣿⣁⠀⠀⣸⠇⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠙⣡⣾⣿⣿⣿⣿⣿⣿⣿⣷⣌⠋⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣴⣿⣷⣄⡈⢻⣿⡟⢁⣠⣾⣿⣦⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢹⣿⣿⣿⣿⠘⣿⠃⣿⣿⣿⣿⡏⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣀⠀⠈⠛⣰⠿⣆⠛⠁⠀⡀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⣼⣿⣦⠀⠘⠛⠋⠀⣴⣿⠁⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⣀⣤⣶⣾⣿⣿⣿⣿⡇⠀⠀⠀⢸⣿⣏⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⣠⣶⣿⣿⣿⣿⣿⣿⣿⣿⠿⠿⠀⠀⠀⠾⢿⣿⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⣠⣿⣿⣿⣿⣿⣿⡿⠟⠋⣁⣠⣤⣤⡶⠶⠶⣤⣄⠈⠀⠀⠀⠀⠀⠀
⠀⠀⠀⢰⣿⣿⣮⣉⣉⣉⣤⣴⣶⣿⣿⣋⡥⠄⠀⠀⠀⠀⠉⢻⣄⠀⠀⠀⠀⠀
⠀⠀⠀⠸⣿⣿⣿⣿⣿⣿⣿⣿⣿⣟⣋⣁⣤⣀⣀⣤⣤⣤⣤⣄⣿⡄⠀⠀⠀⠀
⠀⠀⠀⠀⠙⠿⣿⣿⣿⣿⣿⣿⣿⡿⠿⠛⠋⠉⠁⠀⠀⠀⠀⠈⠛⠃⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠉⠉⠉⠉⠉⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀";

fn render_menu(frame: &mut Frame, app: &App, selected: usize) {
    let inner = frame_block(frame, FOX);

    let fox_height = FOX_ART.lines().count() as u16;
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(fox_height), // fox
            Constraint::Length(1),
            Constraint::Length(MENU_ITEMS.len() as u16 + 2), // items + start row
            Constraint::Fill(1),
            Constraint::Length(1), // status
            Constraint::Length(1), // key help
        ])
        .split(inner);

    render_fox(frame, rows[1]);

    let c = &app.config;
    let values = [
        humantime::format_duration(c.work).to_string(),
        humantime::format_duration(c.short_break).to_string(),
        humantime::format_duration(c.long_break).to_string(),
        c.sessions_before_long_break.to_string(),
        if c.notify { "on" } else { "off" }.to_string(),
        if c.alert_screen { "on" } else { "off" }.to_string(),
    ];

    let mut lines: Vec<Line> = MENU_ITEMS
        .iter()
        .zip(values)
        .enumerate()
        .map(|(i, (label, value))| {
            let marker = if i == selected { "▸ " } else { "  " };
            let style = if i == selected {
                Style::default().fg(FOX).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Line::styled(format!("{marker}{label:<15} ◂ {value:>7} ▸"), style)
        })
        .collect();
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "press enter to start",
        Style::default().fg(Color::DarkGray),
    ));

    frame.render_widget(
        Paragraph::new(lines).alignment(Alignment::Center),
        rows[3],
    );

    if let Some(status) = &app.status {
        frame.render_widget(
            Paragraph::new(status.as_str())
                .style(Style::default().fg(Color::Yellow))
                .alignment(Alignment::Center),
            rows[5],
        );
    }

    render_help(frame, rows[6], "↑↓ select · ←→ adjust · enter start · q quit");
}

/// Center the fox as a block: left-aligned inside a width-fitted rect, so
/// the art's internal indentation survives (per-line centering would skew it).
fn render_fox(frame: &mut Frame, area: Rect) {
    let width = FOX_ART.lines().map(|l| l.chars().count()).max().unwrap_or(0) as u16;
    frame.render_widget(
        Paragraph::new(FOX_ART).style(Style::default().fg(FOX)),
        centered(area, width),
    );
}

// --- Timer screen ---

fn phase_color(phase: Phase) -> Color {
    match phase {
        Phase::Work => Color::LightRed,
        Phase::ShortBreak => Color::LightGreen,
        Phase::LongBreak => Color::LightBlue,
    }
}

fn render_timer(frame: &mut Frame, timer: &Timer) {
    let accent = phase_color(timer.phase);
    let inner = frame_block(frame, accent);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1), // phase + session dots
            Constraint::Length(1),
            Constraint::Length(5), // big clock
            Constraint::Length(1),
            Constraint::Fill(1),   // progress pie takes the rest
            Constraint::Length(1), // key help
        ])
        .split(inner);

    render_phase_line(frame, rows[1], timer, accent);
    render_clock(frame, rows[3], timer, accent);
    render_pie(frame, rows[5], timer, accent);
    render_help(
        frame,
        rows[6],
        "space pause · s skip · r reset · m menu · q quit",
    );
}

// --- Alert screen ---

/// Full-screen banner shown between phases; the timer is frozen behind it
/// until the user presses Enter.
fn render_alert(frame: &mut Frame, timer: &Timer, phase: Phase) {
    let accent = phase_color(phase);
    let inner = frame_block(frame, accent);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(3), // banner
            Constraint::Length(1),
            Constraint::Length(1), // tagline
            Constraint::Length(1), // upcoming phase length
            Constraint::Fill(1),
            Constraint::Length(1), // key help
        ])
        .split(inner);

    let (title, tagline) = match phase {
        Phase::Work => ("BACK TO WORK", "Break's over — time to focus."),
        Phase::ShortBreak => ("BREAK TIME", "Stretch your legs for a bit."),
        Phase::LongBreak => ("LONG BREAK", "You earned it. Step away."),
    };

    let banner = Paragraph::new(vec![
        Line::raw(""),
        Line::from(Span::styled(
            format!("  🦊  {title}  🦊  "),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::raw(""),
    ])
    .style(Style::default().fg(accent).add_modifier(Modifier::REVERSED))
    .alignment(Alignment::Center);
    frame.render_widget(banner, centered(rows[1], 40));

    frame.render_widget(
        Paragraph::new(tagline)
            .style(Style::default().fg(accent))
            .alignment(Alignment::Center),
        rows[3],
    );
    frame.render_widget(
        Paragraph::new(format!(
            "{} · {}",
            timer.phase.label(),
            humantime::format_duration(timer.total)
        ))
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center),
        rows[4],
    );

    render_help(frame, rows[6], "enter continue · s skip · q quit");
}

fn render_phase_line(frame: &mut Frame, area: Rect, timer: &Timer, accent: Color) {
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

fn render_clock(frame: &mut Frame, area: Rect, timer: &Timer, accent: Color) {
    let secs = timer.remaining.as_secs();
    let text = format!("{:02}:{:02}", secs / 60, secs % 60);
    let lines = big_text(&text);
    let style = if timer.paused {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    };
    let paragraph = Paragraph::new(lines.into_iter().map(Line::from).collect::<Vec<_>>())
        .style(style)
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

/// Filled progress pie: the elapsed slice sweeps clockwise from 12 o'clock
/// in the accent color, the rest stays dim. Each braille dot is painted
/// exactly once, colored by its own angle — overlapping strokes would smear
/// colors, since a terminal cell can only hold one. Everything works in dot
/// units (2x4 braille dots per cell): dot pitch is square in a typical 1:2
/// terminal cell, so the circle stays round at any size. The circle fills
/// 80% of the limiting dimension.
fn render_pie(frame: &mut Frame, area: Rect, timer: &Timer, accent: Color) {
    use std::f64::consts::{FRAC_PI_2, TAU};

    let progress = timer.progress().clamp(0.0, 1.0);

    let (w, h) = (area.width as f64 * 2.0, area.height as f64 * 4.0);
    let radius = 0.8 * w.min(h) / 2.0;
    let mut elapsed = Vec::new();
    let mut remaining = Vec::new();
    for iy in 0..h as usize {
        for ix in 0..w as usize {
            let x = ix as f64 + 0.5 - w / 2.0;
            let y = iy as f64 + 0.5 - h / 2.0;
            if x * x + y * y > radius * radius {
                continue;
            }
            let t = (FRAC_PI_2 - y.atan2(x)).rem_euclid(TAU) / TAU;
            let dot = (ix as f64 + 0.5, iy as f64 + 0.5);
            if t < progress {
                elapsed.push(dot);
            } else {
                remaining.push(dot);
            }
        }
    }

    let canvas = Canvas::default()
        .marker(Marker::Braille)
        .x_bounds([0.0, w])
        .y_bounds([0.0, h])
        .paint(move |ctx| {
            ctx.draw(&Points {
                coords: &remaining,
                color: Color::DarkGray,
            });
            ctx.draw(&Points {
                coords: &elapsed,
                color: accent,
            });
        });
    frame.render_widget(canvas, area);
}

fn render_help(frame: &mut Frame, area: Rect, text: &str) {
    frame.render_widget(
        Paragraph::new(text)
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
