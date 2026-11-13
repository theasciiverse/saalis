//! Drawing a frame: the layout, the space view, the HUD and the cards over
//! them. Everything is centred in the terminal and follows a resize.

pub mod canvas;
pub mod deck;
pub mod dock;
pub mod glyphs;
pub mod hud;
pub mod journal;
pub mod navmap;
pub mod palette;
pub mod space;
pub mod view;

use crate::app::{App, Screen};
use crate::sim::Phase;
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
    pub app: &'a App,
    pub help: bool,
    pub paused: bool,
    pub confirm_quit: bool,
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
    let app = ui.app;
    let w = &app.world;
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
    // the field and the right panel together, for the map and the dock
    let wide = Rect::new(l.field.x, l.field.y, l.field.width + l.radar.width, l.field.height);
    hud::top(buf, l.top, w);
    if app.screen == Screen::Base {
        deck::bottom(buf, l.bottom, app);
    } else {
        hud::bottom(buf, l.bottom, w, ui.note);
    }
    if app.screen == Screen::Base {
        deck::draw(buf, wide, app);
    } else if app.screen == Screen::Dock {
        dock::draw(buf, wide, app);
    } else {
        // the camera leads a little in the direction of flight
        let centre = w.ships.first().map(|p| p.pos + p.vel * 0.6).unwrap_or_default();
        let v = view::View::new(l.field, centre, ui.aspect);
        space::draw(buf, &v, w);
        hud::radar(buf, l.radar, w, ui.aspect);
        hud::target(buf, l.target, w);
        if app.screen == Screen::Map {
            navmap::draw(buf, wide, app, ui.aspect);
        }
        if app.screen == Screen::Comms {
            comms(buf, l.field, app);
        }
    }

    if app.screen == Screen::Title {
        title(buf, l.field, app.saved.as_deref());
    } else if w.phase == Phase::Lost {
        lost(buf, l.field, w, app.demo, app.saved.is_some());
    }
    if app.demo && app.screen != Screen::Title {
        let s = " DEMO · any key ends it ";
        let x = l.field.x + l.field.width.saturating_sub(s.len() as u16 + 1);
        buf.set_string(x, l.field.y, s, Style::default().fg(pal::DIM).bg(pal::CHROME_BG));
    }
    if ui.help {
        help(buf, area);
    } else if ui.confirm_quit {
        card(buf, area, "quit", &[Line::from("Leave the Reach?"), Line::from(""), Line::from(Span::styled("Y quits · any other key stays", Style::default().fg(pal::DIM)))]);
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

fn title(buf: &mut Buffer, field: Rect, saved: Option<&str>) {
    let mut lines: Vec<Line> = LOGO.iter().map(|s| Line::from(Span::styled(*s, Style::default().fg(pal::WIRE)))).collect();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("The Northern Reach", Style::default().fg(pal::TEXT).add_modifier(Modifier::BOLD))));
    lines.push(Line::from(Span::styled("high above the belt, nothing is free", Style::default().fg(pal::DIM))));
    lines.push(Line::from(""));
    if let Some(s) = saved {
        lines.push(Line::from(vec![Span::styled("C continue  ", Style::default().fg(pal::TARGET)), Span::styled(s.to_string(), Style::default().fg(pal::DIM))]));
    }
    lines.push(Line::from(Span::styled("Enter new game · F1 keys · Esc quit", Style::default().fg(pal::TARGET))));
    card(buf, field, "", &lines);
}

fn comms(buf: &mut Buffer, field: Rect, app: &App) {
    let w = &app.world;
    let Some(s) = app.hailing.and_then(|id| w.ship(id)) else { return };
    let col = pal::rgb(pal::faction(s.faction));
    let mut lines = vec![
        Line::from(Span::styled(s.callsign.clone(), Style::default().fg(col).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled(format!("{} · {}", s.faction.name(), s.class.role), Style::default().fg(pal::DIM))),
        Line::from(""),
    ];
    for (i, l) in w.hail_lines(s.id).iter().enumerate() {
        lines.push(Line::from(vec![Span::styled(format!("{} ", i + 1), Style::default().fg(pal::TARGET)), Span::styled(l.clone(), Style::default().fg(pal::TEXT))]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("a number says it · Esc hangs up", Style::default().fg(pal::DIM))));
    card(buf, field, "comms", &lines);
}

fn lost(buf: &mut Buffer, field: Rect, w: &crate::sim::World, demo: bool, saved: bool) {
    let head = match &w.lost_aboard {
        Some(name) => format!("KILLED ABOARD THE {}", name.to_uppercase()),
        None => "SHIP LOST".into(),
    };
    let mut lines = vec![
        Line::from(Span::styled(head, Style::default().fg(pal::TARGET).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(format!("{} · {} kills · {} cr", w.date(), w.kills, w.credits)),
    ];
    if w.phase_t > 2.0 && !demo {
        lines.push(Line::from(""));
        let what = if saved { "Enter: back to your last save" } else { "Enter: start again" };
        lines.push(Line::from(Span::styled(what, Style::default().fg(pal::DIM))));
    }
    card(buf, field, "", &lines);
}

const KEYS: [(&str, &str); 22] = [
    ("← →  A D", "turn"),
    ("↑ ↓  W S", "throttle up, down"),
    ("Home", "full speed"),
    ("End  Backspace", "stop"),
    ("M", "match the target's speed"),
    ("Space", "fire guns (hold)"),
    ("Tab", "afterburner (hold)"),
    ("F", "missile (seeks when LOCK shows)"),
    ("T", "next target"),
    ("R", "nearest target"),
    ("N", "nav map (Tab in it: the Reach)"),
    ("[  ]", "previous, next nav point"),
    ("G", "autopilot to the nav point"),
    ("J", "jump through a lane mouth"),
    ("L", "land at a base (close and slow)"),
    ("C", "hail the target or the nearest ship"),
    ("P", "pause"),
    ("F8", "sound on, off"),
    ("F1", "these keys"),
    ("Esc", "quit"),
    ("on foot", "walk into things to use them · Enter talks · aboard: F fire, M medkit"),
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
    use crate::sim::World;

    fn app(w: World, screen: Screen) -> App {
        App {
            world: w,
            screen,
            map_sector: false,
            map_sel: 0,
            dock_row: 0,
            dock_tab: 0,
            yard_row: 0,
            job_row: 0,
            dock_note: None,
            saved: None,
            demo: false,
            ask_quit: false,
            hailing: None,
            deck: None,
            quests: crate::sim::quest::Quests::new(Vec::new()),
            quest_wait: 0,
        }
    }

    fn ui(a: &App) -> Ui<'_> {
        Ui { app: a, help: false, paused: false, confirm_quit: false, note: None, aspect: 2.0 }
    }

    fn text(buf: &Buffer) -> String {
        let a = buf.area;
        (0..a.height).map(|y| (0..a.width).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
    }

    fn frame_of(a: &App, w: u16, h: u16) -> String {
        let area = Rect::new(0, 0, w, h);
        let mut buf = Buffer::empty(area);
        draw_buf(&mut buf, area, &ui(a));
        let t = text(&buf);
        if std::env::var("SAALIS_PRINT").is_ok() {
            println!("{t}");
        }
        t
    }

    #[test]
    fn every_screen_draws_at_every_size() {
        let mut w = World::demo(5);
        for _ in 0..(60 * 8) {
            w.update(&Intent::default(), 1.0 / 60.0);
        }
        let mut a = app(w, Screen::Flight);
        for (cw, ch) in [(20, 6), (79, 23), (80, 24), (100, 30), (160, 50), (300, 90)] {
            for screen in [Screen::Title, Screen::Flight, Screen::Map] {
                a.screen = screen;
                for sector in [false, true] {
                    a.map_sector = sector;
                    for flags in 0..4 {
                        let area = Rect::new(0, 0, cw, ch);
                        let mut buf = Buffer::empty(area);
                        let mut u = ui(&a);
                        u.help = flags == 1;
                        u.paused = flags == 2;
                        u.confirm_quit = flags == 3;
                        draw_buf(&mut buf, area, &u);
                    }
                }
            }
        }
        let mut docked = app(World::new(5), Screen::Dock);
        for tab in 0..4 {
            docked.dock_tab = tab;
            for (cw, ch) in [(20, 6), (80, 24), (100, 30), (300, 90)] {
                frame_of(&docked, cw, ch);
            }
        }
        docked.go_aboard();
        for (cw, ch) in [(20, 6), (79, 23), (80, 24), (100, 30), (160, 50), (300, 90)] {
            frame_of(&docked, cw, ch);
        }
        let mut w = World::new(5);
        w.practice_boarding();
        let mut boarded = app(w, Screen::Flight);
        boarded.follow();
        assert!(boarded.deck.as_ref().is_some_and(|d| d.boarding));
        for (cw, ch) in [(20, 6), (79, 23), (80, 24), (100, 30), (160, 50), (300, 90)] {
            frame_of(&boarded, cw, ch);
        }
    }

    #[test]
    fn a_flight_frame_shows_the_hud_and_the_player() {
        let a = app(World::demo(5), Screen::Flight);
        let t = frame_of(&a, 100, 30);
        assert!(t.contains("SHIELD") && t.contains("ARMOUR") && t.contains("radar"));
        assert!(t.contains('#'), "the player's glyph");
        assert!(t.contains("Pallas Reach"));
    }

    #[test]
    fn the_dock_shows_the_exchange() {
        let a = app(World::new(5), Screen::Dock);
        let t = frame_of(&a, 100, 30);
        assert!(t.contains("Routa Station") && t.contains("Water ice") && t.contains("index"));
    }

    #[test]
    fn on_foot_the_deck_shows_you_and_your_ship() {
        let mut a = app(World::new(5), Screen::Dock);
        a.go_aboard();
        let t = frame_of(&a, 80, 24);
        assert!(t.contains('@') && t.contains("hangar") && t.contains("exchange") && t.contains("bar"));
        assert!(t.contains("─┤├─"), "the Tiira's schematic in the hangar");
    }

    #[test]
    fn a_quest_talks_in_a_box_and_keeps_a_journal() {
        use crate::sim::quest::{Quests, Tick};
        let mut a = app(World::new(5), Screen::Base);
        a.quests = Quests::built_in();
        a.quests.tick(
            &mut a.world,
            &Tick {
                on_foot: true,
                talked: None,
            },
        );
        a.go_aboard();
        let t = frame_of(&a, 100, 30);
        assert!(
            t.contains("QUESTS") && t.contains("The Jumped Claim"),
            "the journal beside the deck"
        );
        a.quests.tick(
            &mut a.world,
            &Tick {
                on_foot: true,
                talked: Some("ilta".into()),
            },
        );
        let t = frame_of(&a, 100, 30);
        assert!(
            t.contains("Ilta Rask") && t.contains("You fly?"),
            "her line in the talk box"
        );
        a.quests.dismiss();
        let t = frame_of(&a, 100, 30);
        assert!(
            t.contains("1 I'll get it back.") && t.contains("1 to 3 answer"),
            "her question and its options"
        );
        for (cw, ch) in [(20, 6), (79, 23), (80, 24), (100, 30), (160, 50), (300, 90)] {
            frame_of(&a, cw, ch);
        }
        a.dock_tab = 1;
        a.screen = Screen::Dock;
        let t = frame_of(&a, 100, 30);
        assert!(t.contains("The Jumped Claim"), "on the mission boards");
    }

    #[test]
    fn the_map_shows_lanes_and_the_reach() {
        let mut w = World::new(5);
        w.launch();
        let mut a = app(w, Screen::Map);
        let t = frame_of(&a, 100, 30);
        assert!(t.contains("Kide Drift"), "a lane out of Pallas Reach");
        a.map_sector = true;
        let t = frame_of(&a, 100, 30);
        assert!(t.contains("Crown Gate") && t.contains("Kalmisto"));
    }

    #[test]
    fn too_small_says_so() {
        let a = app(World::new(5), Screen::Flight);
        assert!(frame_of(&a, 60, 20).contains("80x24"));
    }
}
