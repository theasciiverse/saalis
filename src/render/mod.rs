//! Drawing a frame: the layout, the space view, the HUD and the cards over
//! them. Everything is centred in the terminal and follows a resize.

pub mod canvas;
pub mod glyphs;
pub mod hud;
pub mod palette;
pub mod space;
pub mod view;

use crate::sim::{Phase, World};
use palette as pal;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Widget};
use ratatui::Frame;

pub const MIN_W: u16 = 80;
pub const MIN_H: u16 = 24;
/// Past this the layout stops growing and sits centred.
const MAX_W: u16 = 240;
const MAX_H: u16 = 72;
const RIGHT_W: u16 = 26;

/// What the frame needs beyond the world.
pub struct Ui<'a> {
    pub world: &'a World,
    pub title: bool,
    pub help: bool,
    pub paused: bool,
    pub confirm_quit: bool,
    pub demo: bool,
    pub note: Option<&'a str>,
    pub aspect: f32,
}

pub struct Layout {
    pub top: Rect,
    pub bottom: Rect,
    pub field: Rect,
    pub radar: Rect,
    pub target: Rect,
}

pub fn layout(area: Rect, aspect: f32) -> Option<Layout> {
    if area.width < MIN_W || area.height < MIN_H {
        return None;
    }
    let w = area.width.min(MAX_W);
    let h = area.height.min(MAX_H);
    let a = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    let top = Rect::new(a.x, a.y, a.width, 4);
    let bottom = Rect::new(a.x, a.bottom() - 3, a.width, 3);
    let mid_h = a.height - 7;
    let right = Rect::new(a.right() - RIGHT_W, a.y + 4, RIGHT_W, mid_h);
    // the scope wants a round circle: about 24 columns across
    let want = ((RIGHT_W - 2) as f32 / aspect).round() as u16 + 2;
    let radar_h = want.min(mid_h.saturating_sub(8)).max(5);
    let radar = Rect::new(right.x, right.y, right.width, radar_h);
    let target = Rect::new(right.x, right.y + radar_h, right.width, mid_h - radar_h);
    let field = Rect::new(a.x, a.y + 4, a.width - RIGHT_W, mid_h);
    Some(Layout { top, bottom, field, radar, target })
}

pub fn draw(f: &mut Frame, ui: &Ui) {
    let area = f.area();
    let buf = f.buffer_mut();
    draw_buf(buf, area, ui);
}

pub fn draw_buf(buf: &mut Buffer, area: Rect, ui: &Ui) {
    let w = ui.world;
    Clear.render(area, buf);
    buf.set_style(area, Style::default().bg(pal::SPACE));
    let Some(l) = layout(area, ui.aspect) else {
        let msg = format!("Saalis needs a terminal of at least {MIN_W}x{MIN_H}.\nEnlarge the window or shrink the font.");
        let lines: Vec<Line> = msg.lines().map(|s| Line::from(Span::styled(s.to_string(), Style::default().fg(pal::TEXT)))).collect();
        let h = lines.len() as u16;
        let y = area.y + area.height.saturating_sub(h) / 2;
        Paragraph::new(lines).centered().render(Rect::new(area.x, y, area.width, h.min(area.height)), buf);
        return;
    };
    // the camera leads a little in the direction of flight
    let centre = w.ships.first().map(|p| p.pos + p.vel * 0.6).unwrap_or_default();
    let v = view::View::new(l.field, centre, ui.aspect);
    space::draw(buf, &v, w);
    hud::top(buf, l.top, w);
    hud::bottom(buf, l.bottom, w, ui.note);
    hud::radar(buf, l.radar, w, ui.aspect);
    hud::target(buf, l.target, w);

    if ui.title {
        title(buf, l.field);
    } else if w.phase == Phase::Lost {
        lost(buf, l.field, w, ui.demo);
    }
    if ui.demo && !ui.title {
        let s = " DEMO · any key ends it ";
        let x = l.field.x + l.field.width.saturating_sub(s.len() as u16 + 1);
        buf.set_string(x, l.field.y, s, Style::default().fg(pal::DIM).bg(pal::CHROME_BG));
    }
    if ui.help {
        help(buf, area);
    } else if ui.confirm_quit {
        card(buf, area, "quit", &[Line::from("Leave the Reach?"), Line::from(""), Line::from(Span::styled("Y quits · any other key flies on", Style::default().fg(pal::DIM)))]);
    } else if ui.paused {
        card(buf, area, "paused", &[Line::from("Holding position."), Line::from(""), Line::from(Span::styled("P flies on · F1 keys", Style::default().fg(pal::DIM)))]);
    }
}

/// A centred card with a rounded frame.
fn card(buf: &mut Buffer, area: Rect, name: &str, lines: &[Line]) {
    let w = lines.iter().map(|l| l.width() as u16).max().unwrap_or(10).max(name.len() as u16 + 4) + 6;
    let h = lines.len() as u16 + 2;
    let r = Rect::new(area.x + area.width.saturating_sub(w) / 2, area.y + area.height.saturating_sub(h) / 2, w.min(area.width), h.min(area.height));
    Clear.render(r, buf);
    let block = hud::frame(name);
    let inner = block.inner(r);
    block.render(r, buf);
    Paragraph::new(lines.to_vec()).centered().style(Style::default().fg(pal::TEXT)).render(inner, buf);
}

/// The name in box-drawing letters: clean lines (David's brief).
const LOGO: [&str; 3] = ["╭─╴╭─╮╭─╮╷  ╶┬╴╭─╴", "╰─╮├─┤├─┤│   │ ╰─╮", "╶─╯╵ ╵╵ ╵╰─╴╶┴╴╶─╯"];

fn title(buf: &mut Buffer, field: Rect) {
    let mut lines: Vec<Line> = LOGO.iter().map(|s| Line::from(Span::styled(*s, Style::default().fg(pal::WIRE)))).collect();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("The Northern Reach", Style::default().fg(pal::TEXT).add_modifier(Modifier::BOLD))));
    lines.push(Line::from(Span::styled("high above the belt, nothing is free", Style::default().fg(pal::DIM))));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("Enter launch · F1 keys · Esc quit", Style::default().fg(pal::TARGET))));
    card(buf, field, "", &lines);
}

fn lost(buf: &mut Buffer, field: Rect, w: &World, demo: bool) {
    let mut lines = vec![
        Line::from(Span::styled("SHIP LOST", Style::default().fg(pal::TARGET).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(format!("wave {} · {} kills · {} cr", w.wave, w.kills, w.score)),
    ];
    if w.phase_t > 2.0 && !demo {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("Enter: a spare hull from Routa", Style::default().fg(pal::DIM))));
    }
    card(buf, field, "", &lines);
}

const KEYS: [(&str, &str); 14] = [
    ("← →  A D", "turn"),
    ("↑ ↓  W S", "throttle up, down"),
    ("Home", "full speed"),
    ("End  Backspace", "stop"),
    ("M", "match the target's speed"),
    ("Space", "fire guns (hold)"),
    ("Tab", "afterburner (hold)"),
    ("T", "next target"),
    ("R", "nearest target"),
    ("P", "pause"),
    ("F8", "sound on, off"),
    ("F1", "these keys"),
    ("Esc", "quit"),
    ("pad", "stick flies · A fires · B burns · RB target"),
];

fn help(buf: &mut Buffer, area: Rect) {
    let lines: Vec<Line> = KEYS
        .iter()
        .map(|(k, what)| Line::from(vec![Span::styled(format!("{k:>15}  "), Style::default().fg(pal::TARGET)), Span::styled(format!("{what:<44}"), Style::default().fg(pal::TEXT))]))
        .collect();
    card(buf, area, "keys", &lines);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::ship::Intent;

    fn ui(w: &World) -> Ui<'_> {
        Ui { world: w, title: false, help: false, paused: false, confirm_quit: false, demo: false, note: None, aspect: 2.0 }
    }

    fn text(buf: &Buffer) -> String {
        let a = buf.area;
        (0..a.height).map(|y| (0..a.width).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
    }

    #[test]
    fn every_screen_draws_at_every_size() {
        let mut w = World::new(5);
        for _ in 0..(60 * 8) {
            w.update(&Intent::default(), 1.0 / 60.0);
        }
        for (cw, ch) in [(20, 6), (79, 23), (80, 24), (100, 30), (160, 50), (300, 90)] {
            for flags in 0..5 {
                let mut u = ui(&w);
                u.title = flags == 1;
                u.help = flags == 2;
                u.paused = flags == 3;
                u.confirm_quit = flags == 4;
                let area = Rect::new(0, 0, cw, ch);
                let mut buf = Buffer::empty(area);
                draw_buf(&mut buf, area, &u);
            }
        }
    }

    #[test]
    fn a_frame_shows_the_hud_and_the_player() {
        let w = World::new(5);
        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);
        draw_buf(&mut buf, area, &ui(&w));
        let t = text(&buf);
        if std::env::var("SAALIS_PRINT").is_ok() {
            println!("{t}");
        }
        assert!(t.contains("SHIELD") && t.contains("ARMOUR") && t.contains("radar"));
        assert!(t.contains('#'), "the player's hull");
        assert!(t.contains('▄') && t.contains('│'), "the station and its dock");
    }

    #[test]
    fn too_small_says_so() {
        let w = World::new(5);
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        draw_buf(&mut buf, area, &ui(&w));
        assert!(text(&buf).contains("80x24"));
    }
}
