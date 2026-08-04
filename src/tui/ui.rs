use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::canvas::{Canvas, Points};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use super::app::{AUDIO_MENU_ITEMS, App, MENU_ITEMS, Screen};
use crate::stats::{self, Summary};
use crate::theme::Palette;
use crate::timer::{Phase, Timer};

pub fn render(frame: &mut Frame, app: &App) {
    let palette = app.palette();
    match (&app.screen, app.alert) {
        (Screen::Timer(timer), Some(phase)) => render_alert(frame, app, timer, phase, palette),
        (Screen::Timer(timer), None) => render_timer(frame, app, timer, palette),
        (Screen::Menu { selected }, _) => render_menu(frame, app, *selected, palette),
        (Screen::AudioMenu { selected, preview }, _) => {
            render_audio_menu(frame, app, *selected, *preview, palette)
        }
    }
    if let Some(summary) = &app.stats_view {
        render_stats(frame, summary, palette);
    }
}

fn frame_block(frame: &mut Frame, accent: Color, title: &str) -> Rect {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .title(title.to_string())
        .title_alignment(Alignment::Center);
    let inner = block.inner(frame.area());
    frame.render_widget(block, frame.area());
    inner
}

fn render_status(frame: &mut Frame, area: Rect, status: Option<&str>, palette: Palette) {
    if let Some(status) = status {
        frame.render_widget(
            Paragraph::new(status)
                .style(Style::default().fg(palette.warning))
                .alignment(Alignment::Center),
            area,
        );
    }
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

fn render_menu(frame: &mut Frame, app: &App, selected: usize, palette: Palette) {
    let inner = frame_block(frame, palette.fox, " 🦊 Focus Fox ");

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

    render_fox(frame, rows[1], palette);

    let c = &app.config;
    let theme = match c.theme {
        crate::theme::ThemePreference::Auto => format!("auto ({})", app.theme_mode()),
        preference => preference.to_string(),
    };
    let values = [
        humantime::format_duration(c.work).to_string(),
        humantime::format_duration(c.short_break).to_string(),
        humantime::format_duration(c.long_break).to_string(),
        c.sessions_before_long_break.to_string(),
        if c.notify { "on" } else { "off" }.to_string(),
        if c.alert_screen { "on" } else { "off" }.to_string(),
        audio_detail(app),
        theme,
    ];

    let mut lines: Vec<Line> = MENU_ITEMS
        .iter()
        .zip(values)
        .enumerate()
        .map(|(i, (label, value))| {
            let marker = if i == selected { "▸ " } else { "  " };
            let style = if i == selected {
                Style::default()
                    .fg(palette.fox)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Line::styled(format!("{marker}{label:<15} ◂ {value} ▸"), style)
        })
        .collect();
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "press enter to start",
        Style::default().fg(palette.muted),
    ));

    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), rows[3]);

    render_status(frame, rows[5], app.status.as_deref(), palette);

    render_help(
        frame,
        rows[6],
        "↑↓ select · ←→ adjust · enter start/open · a audio · t stats · q quit",
        palette,
    );
}

fn audio_detail(app: &App) -> String {
    let settings = app.config.tone_settings();
    let state = if app.config.binaural_beats {
        "on"
    } else {
        "off"
    };
    format!(
        "{state} · {} · {}/{} Hz · {} Hz · {}%",
        app.config.binaural_preset,
        settings.base_hz,
        settings.base_hz + settings.beat_hz,
        settings.beat_hz,
        settings.volume_percent,
    )
}

fn render_audio_menu(
    frame: &mut Frame,
    app: &App,
    selected: usize,
    preview: bool,
    palette: Palette,
) {
    let inner = frame_block(frame, palette.fox, " 🦊 Audio Settings ");
    let settings = app.config.tone_settings();
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(AUDIO_MENU_ITEMS.len() as u16),
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(format!(
            "{}/{} Hz · {} Hz difference · {}% volume{}",
            settings.base_hz,
            settings.base_hz + settings.beat_hz,
            settings.beat_hz,
            settings.volume_percent,
            if preview { " · previewing" } else { "" },
        ))
        .style(Style::default().fg(if preview { palette.work } else { palette.muted }))
        .alignment(Alignment::Center),
        rows[1],
    );

    let values = [
        if app.config.binaural_beats {
            "on".to_string()
        } else {
            "off".to_string()
        },
        app.config.binaural_preset.to_string(),
        format!("{} Hz", settings.base_hz),
        format!("{} Hz", settings.beat_hz),
        format!("{}%", settings.volume_percent),
    ];
    let lines: Vec<Line> = AUDIO_MENU_ITEMS
        .iter()
        .zip(values)
        .enumerate()
        .map(|(index, (label, value))| {
            let marker = if index == selected { "▸ " } else { "  " };
            let style = if index == selected {
                Style::default()
                    .fg(palette.fox)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Line::styled(format!("{marker}{label:<16} ◂ {value} ▸"), style)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), rows[3]);
    render_status(frame, rows[5], app.status.as_deref(), palette);
    render_help(
        frame,
        rows[6],
        "↑↓ select · ←→ adjust · p preview · enter/esc/m back · use headphones safely",
        palette,
    );
}

/// Center the fox as a block: left-aligned inside a width-fitted rect, so
/// the art's internal indentation survives (per-line centering would skew it).
fn render_fox(frame: &mut Frame, area: Rect, palette: Palette) {
    let width = FOX_ART
        .lines()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0) as u16;
    frame.render_widget(
        Paragraph::new(FOX_ART).style(Style::default().fg(palette.fox)),
        centered(area, width),
    );
}

// --- Timer screen ---

fn phase_color(phase: Phase, palette: Palette) -> Color {
    match phase {
        Phase::Work => palette.work,
        Phase::ShortBreak => palette.short_break,
        Phase::LongBreak => palette.long_break,
    }
}

fn render_timer(frame: &mut Frame, app: &App, timer: &Timer, palette: Palette) {
    let accent = phase_color(timer.phase, palette);
    let inner = frame_block(frame, accent, " 🦊 Focus Fox ");

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1), // phase + session dots
            Constraint::Length(1),
            Constraint::Length(5), // big clock
            Constraint::Length(1),
            Constraint::Fill(1),   // progress pie takes the rest
            Constraint::Length(1), // status
            Constraint::Length(1), // key help
        ])
        .split(inner);

    render_phase_line(frame, rows[1], timer, accent, palette);
    render_clock(frame, rows[3], timer, accent, palette);
    render_pie(frame, rows[5], timer, accent, palette);
    render_status(frame, rows[6], app.status.as_deref(), palette);
    render_help(
        frame,
        rows[7],
        "space pause · s skip · r reset · h/l ±1m · t stats · m menu · q quit",
        palette,
    );
}

// --- Alert screen ---

/// Full-screen banner shown between phases; the timer is frozen behind it
/// until the user presses Enter.
fn render_alert(frame: &mut Frame, app: &App, timer: &Timer, phase: Phase, palette: Palette) {
    let accent = phase_color(phase, palette);
    let inner = frame_block(frame, accent, " 🦊 Focus Fox ");

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(3), // banner
            Constraint::Length(1),
            Constraint::Length(1), // tagline
            Constraint::Length(1), // upcoming phase length
            Constraint::Fill(1),
            Constraint::Length(1), // status
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
        .style(Style::default().fg(palette.muted))
        .alignment(Alignment::Center),
        rows[4],
    );

    render_status(frame, rows[6], app.status.as_deref(), palette);
    render_help(frame, rows[7], "enter continue · s skip · q quit", palette);
}

// --- Stats overlay ---

/// Full-frame stats panel drawn over the current screen; the timer keeps
/// ticking underneath.
fn render_stats(frame: &mut Frame, s: &Summary, palette: Palette) {
    frame.render_widget(Clear, frame.area());
    let inner = frame_block(frame, palette.fox, " 🦊 Stats ");

    let mut lines: Vec<Line> = vec![
        stat_line("Today", s.today_sessions, s.today_focus),
        stat_line("This week", s.week_sessions, s.week_focus),
        Line::from(format!("{:<10} {} days", "Streak", s.streak_days)),
        stat_line("Lifetime", s.lifetime_sessions, s.lifetime_focus),
    ];
    if !s.recent.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "Recent",
            Style::default()
                .fg(palette.fox)
                .add_modifier(Modifier::BOLD),
        ));
        lines.extend(s.recent.iter().map(|r| {
            Line::styled(
                stats::recent_line(r),
                Style::default().fg(palette.secondary),
            )
        }));
    } else {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "No sessions yet — finish one. 🦊",
            Style::default().fg(palette.muted),
        ));
    }

    let width = lines.iter().map(|l| l.width()).max().unwrap_or(0) as u16;

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(lines.len() as u16),
            Constraint::Fill(1),
            Constraint::Length(1), // key help
        ])
        .split(inner);

    frame.render_widget(Paragraph::new(lines), centered(rows[1], width));
    render_help(frame, rows[3], "t/esc close", palette);
}

fn stat_line(label: &str, sessions: u32, focus: std::time::Duration) -> Line<'static> {
    Line::from(format!(
        "{label:<10} {sessions} sessions · {}",
        stats::fmt_focus(focus)
    ))
}

fn render_phase_line(
    frame: &mut Frame,
    area: Rect,
    timer: &Timer,
    accent: Color,
    palette: Palette,
) {
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
            Style::default().fg(palette.warning),
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).alignment(Alignment::Center),
        area,
    );
}

fn render_clock(frame: &mut Frame, area: Rect, timer: &Timer, accent: Color, palette: Palette) {
    let secs = timer.remaining.as_secs();
    let text = format!("{:02}:{:02}", secs / 60, secs % 60);
    let lines = big_text(&text);
    let style = if timer.paused {
        Style::default().fg(palette.muted)
    } else {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    };
    let paragraph = Paragraph::new(lines.into_iter().map(Line::from).collect::<Vec<_>>())
        .style(style)
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

/// Progress dial: the elapsed slice sweeps clockwise from 12 o'clock in the
/// accent color, the rest stays dim. Each braille dot is painted exactly
/// once, colored by its own angle — overlapping strokes would smear colors,
/// since a terminal cell can only hold one. Everything works in dot units
/// (2x4 braille dots per cell): dot pitch is square in a typical 1:2
/// terminal cell, so the circle stays round at any size. The circle fills
/// 80% of the limiting dimension; when it's big enough to hold the fox art
/// it hollows into a ring with the fox denned inside.
fn render_pie(frame: &mut Frame, area: Rect, timer: &Timer, accent: Color, palette: Palette) {
    use std::f64::consts::{FRAC_PI_2, TAU};

    let progress = timer.progress().clamp(0.0, 1.0);

    let (w, h) = (area.width as f64 * 2.0, area.height as f64 * 4.0);
    let radius = 0.8 * w.min(h) / 2.0;
    // Hollow out a den for the fox when a ring of decent thickness still
    // fits around it; otherwise stay a filled pie.
    let hole = {
        let need = fox_reach() + 2.0;
        (radius - need >= 6.0).then(|| need.max(0.6 * radius))
    };
    let mut elapsed = Vec::new();
    let mut remaining = Vec::new();
    for iy in 0..h as usize {
        for ix in 0..w as usize {
            let x = ix as f64 + 0.5 - w / 2.0;
            let y = iy as f64 + 0.5 - h / 2.0;
            let d2 = x * x + y * y;
            if d2 > radius * radius {
                continue;
            }
            if let Some(inner) = hole
                && d2 < inner * inner
            {
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
                color: palette.muted,
            });
            ctx.draw(&Points {
                coords: &elapsed,
                color: accent,
            });
        });
    frame.render_widget(canvas, area);

    if hole.is_some() {
        draw_fox_in_den(frame, area, palette);
    }
}

fn inked(line: &str) -> impl Iterator<Item = (usize, char)> + '_ {
    line.chars().enumerate().filter(|(_, ch)| *ch != '\u{2800}')
}

/// Distance in braille dots from the fox art's center to the farthest
/// corner of any inked cell — the smallest circle the fox fits inside.
fn fox_reach() -> f64 {
    let lines: Vec<&str> = FOX_ART.lines().collect();
    let half_h = lines.len() as f64 * 2.0;
    let half_w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as f64;
    lines
        .iter()
        .enumerate()
        .flat_map(|(r, line)| {
            inked(line).map(move |(c, _)| {
                let x = (c as f64 * 2.0 - half_w)
                    .abs()
                    .max(c as f64 * 2.0 + 2.0 - half_w);
                let y = (r as f64 * 4.0 - half_h)
                    .abs()
                    .max(r as f64 * 4.0 + 4.0 - half_h);
                (x * x + y * y).sqrt()
            })
        })
        .fold(0.0, f64::max)
}

/// The menu fox, centered in the ring's hollow. Cells are replaced whole
/// and blank art cells are skipped — fox and ring never share a cell, so
/// their colors can't smear.
fn draw_fox_in_den(frame: &mut Frame, area: Rect, palette: Palette) {
    let lines: Vec<&str> = FOX_ART.lines().collect();
    let art_h = lines.len() as u16;
    let art_w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as u16;
    if area.width < art_w || area.height < art_h {
        return;
    }

    let origin_x = area.x + (area.width - art_w) / 2;
    let origin_y = area.y + (area.height - art_h) / 2;
    let buf = frame.buffer_mut();
    for (r, line) in lines.iter().enumerate() {
        for (c, ch) in inked(line) {
            if let Some(cell) = buf.cell_mut((origin_x + c as u16, origin_y + r as u16)) {
                cell.set_char(ch);
                cell.set_fg(palette.fox);
            }
        }
    }
}

fn render_help(frame: &mut Frame, area: Rect, text: &str, palette: Palette) {
    frame.render_widget(
        Paragraph::new(text)
            .style(Style::default().fg(palette.muted))
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
