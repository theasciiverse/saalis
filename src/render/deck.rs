//! On foot: the base's deck in classic ASCII (David, 2026-11-08), with
//! the walls in box-drawing pipes, the same line language as the stations
//! outside. You are `@`; everyone else is a letter in their faction's
//! colour.

use super::dock::{side, SIDE_W};
use super::hud::frame;
use super::palette as pal;
use crate::app::App;
use crate::sim::deck::{Deck, Tile};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Widget};

const FLOOR: Color = Color::Rgb(40, 54, 68);
const WALL: Color = Color::Rgb(84, 150, 170);
const DOOR: Color = Color::Rgb(220, 170, 90);

/// The deck beside the side panel (`area` is the field and right panel).
pub fn draw(buf: &mut Buffer, area: Rect, app: &App) {
    Clear.render(area, buf);
    let Some(deck) = &app.deck else { return };
    if area.width < SIDE_W + 30 {
        return;
    }
    if deck.boarding {
        board_side(buf, Rect::new(area.x, area.y, SIDE_W, area.height), app, deck);
    } else {
        side(buf, Rect::new(area.x, area.y, SIDE_W, area.height), app);
    }
    let map = Rect::new(area.x + SIDE_W, area.y, area.width - SIDE_W, area.height);
    buf.set_style(map, Style::default().bg(pal::CHROME_BG));
    // centred when it fits; otherwise the view follows you
    let fit = |len: i32, room: i32, at: i32| if len <= room { (room - len) / 2 } else { -(at - room / 2).clamp(0, len - room) };
    let ox = map.x as i32 + fit(deck.w, map.width as i32, deck.you.0);
    let oy = map.y as i32 + fit(deck.h, map.height as i32, deck.you.1);
    let mut put = |x: i32, y: i32, c: char, st: Style| {
        let (sx, sy) = (ox + x, oy + y);
        if sx >= map.x as i32 && sy >= map.y as i32 && sx < map.right() as i32 && sy < map.bottom() as i32 {
            buf[(sx as u16, sy as u16)].set_char(c).set_style(st);
        }
    };
    let fg = |c: Color| Style::default().fg(c).bg(pal::CHROME_BG);
    for y in 0..deck.h {
        for x in 0..deck.w {
            let (c, col) = match deck.tile(x, y) {
                Tile::Void => continue,
                Tile::Wall => (wall(deck, x, y), WALL),
                Tile::Door => ('+', DOOR),
                Tile::Floor => ('·', FLOOR),
                Tile::Counter => ('=', Color::Rgb(196, 176, 140)),
                Tile::Terminal => ('&', pal::WIRE),
                // a crate with something in it stands out
                Tile::Crate if deck.loot_at(x, y).is_some() => ('%', Color::Rgb(230, 200, 120)),
                Tile::Crate => ('%', pal::rgb(pal::ROCK[2])),
                Tile::Hatch => ('<', pal::rgb(pal::PLAYER)),
                Tile::Console if deck.loot_at(x, y).is_some() => ('&', Color::Rgb(230, 200, 120)),
                Tile::Console => ('&', pal::WIRE),
                Tile::Table => ('o', pal::DIM),
                Tile::Plant => ('"', Color::Rgb(110, 200, 110)),
                Tile::Bed => ('_', pal::TEXT),
                Tile::Vat => ('~', pal::rgb(pal::BURN)),
                // drawn below, as the ship's schematic
                Tile::Ship => (' ', pal::TEXT),
            };
            put(x, y, c, fg(col));
        }
    }
    // each room's name on its outer wall
    for r in &deck.rooms {
        // a narrow room keeps the first word of its name ("guild")
        let fits = |l: &str| l.chars().count() as i32 + 2 <= r.x1 - r.x0 - 1;
        let name = if fits(r.name) { r.name } else { r.name.split(' ').next().unwrap_or(r.name) };
        let label = format!(" {name} ");
        let y = if r.y0 == 0 { r.y0 } else { r.y1 };
        let y = if deck.boarding { r.y1 } else { y };
        let x = r.x0 + 1;
        if label.chars().count() as i32 <= r.x1 - r.x0 - 1 {
            for (i, c) in label.chars().enumerate() {
                put(x + i as i32, y, c, fg(pal::DIM));
            }
        }
    }
    if let Some(p) = app.world.ships.first().filter(|_| !deck.boarding) {
        let art = super::glyphs::schematic(p.class);
        let (sx, sy) = deck.ship_at;
        for (dy, row) in art.iter().enumerate() {
            for (dx, c) in row.chars().enumerate() {
                if c != ' ' {
                    put(sx + dx as i32, sy + dy as i32, c, fg(pal::rgb(pal::PLAYER)));
                }
            }
        }
    }
    for (i, n) in deck.npcs.iter().enumerate() {
        let st = if n.fight.as_ref().is_some_and(|f| f.surrendered) {
            fg(pal::DIM)
        } else {
            fg(pal::rgb(pal::faction(n.faction))).add_modifier(Modifier::BOLD)
        };
        // a quest's person stands out: underlined
        let st = if deck.named_id(i).is_some() {
            st.add_modifier(Modifier::UNDERLINED)
        } else {
            st
        };
        put(n.x, n.y, deck.glyph_of(i), st);
    }
    // the one you would shoot at
    if let Some(i) = deck.in_sights(&app.world.body) {
        let n = &deck.npcs[i];
        put(
            n.x,
            n.y,
            deck.glyph_of(i),
            fg(pal::TARGET).add_modifier(Modifier::BOLD | Modifier::REVERSED),
        );
    }
    put(
        deck.you.0,
        deck.you.1,
        '@',
        fg(Color::White).add_modifier(Modifier::BOLD),
    );
    // the talk box keeps clear of you: below you, or above if you're low
    talk_box(
        buf,
        map,
        app,
        oy + deck.you.1 > map.y as i32 + map.height as i32 / 2,
    );
}

/// Someone talking to you, over the bottom of the deck: their line, or
/// their question and its numbered answers.
fn talk_box(buf: &mut Buffer, map: Rect, app: &App, top: bool) {
    let Some(t) = app.quests.talk.front() else {
        return;
    };
    let w = map.width.saturating_sub(4).min(72);
    if w < 20 {
        return;
    }
    let inner_w = (w - 4) as usize;
    let mut lines: Vec<Line> = wrap(&t.text, inner_w)
        .into_iter()
        .map(|l| Line::from(Span::styled(l, Style::default().fg(pal::TEXT))))
        .collect();
    lines.push(Line::from(""));
    for (k, c) in t.choices.iter().enumerate() {
        for (j, l) in wrap(c, inner_w - 3).into_iter().enumerate() {
            let head = if j == 0 {
                format!("{} ", k + 1)
            } else {
                "  ".into()
            };
            lines.push(Line::from(vec![
                Span::styled(
                    head,
                    Style::default()
                        .fg(pal::TARGET)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(l, Style::default().fg(pal::TEXT)),
            ]));
        }
    }
    let more = app.quests.talk.len() > 1;
    let hint = match (t.choices.len(), more) {
        (0, true) => "Enter: more".to_string(),
        (0, false) => "Enter".to_string(),
        (n, _) => format!("1 to {n} answer"),
    };
    lines.push(Line::from(Span::styled(
        hint,
        Style::default().fg(pal::DIM),
    )));
    let h = (lines.len() as u16 + 2).min(map.height);
    let y = if top {
        map.y
    } else {
        map.bottom().saturating_sub(h).max(map.y)
    };
    let area = Rect::new(map.x + (map.width - w) / 2, y, w, h);
    Clear.render(area, buf);
    let block = frame(&t.who);
    let inner = block.inner(area);
    block.render(area, buf);
    buf.set_style(area, Style::default().bg(pal::CHROME_BG));
    let inner = Rect::new(
        inner.x + 1,
        inner.y,
        inner.width.saturating_sub(2),
        inner.height,
    );
    Paragraph::new(lines).render(inner, buf);
}

/// Words, wrapped to `width` columns.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut cur = String::new();
        for word in para.split(' ') {
            if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > width {
                out.push(std::mem::take(&mut cur));
            }
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(word);
        }
        out.push(cur);
    }
    out
}

/// The bar under the deck: what is beside you, and the last thing said.
pub fn bottom(buf: &mut Buffer, area: Rect, app: &App) {
    let Some(deck) = &app.deck else { return };
    let here = deck.here().map_or("corridor", |r| r.name);
    let beside = deck.nearby().map(|u| format!("beside you: {} · Enter", u.label(deck)));
    let title = match beside {
        Some(b) => format!("{here} · {b}"),
        None => here.to_string(),
    };
    let block = frame(&title);
    let inner = block.inner(area);
    block.render(area, buf);
    let talking = app.quests.talk.front().map(|t| {
        if t.choices.is_empty() {
            "Enter to read on"
        } else {
            "press the number of your answer"
        }
    });
    let line = match (&app.dock_note, deck.boarding) {
        _ if talking.is_some() => Line::from(Span::styled(
            talking.unwrap_or_default(),
            Style::default().fg(pal::DIM),
        )),
        (Some(n), _) => Line::from(Span::styled(n.clone(), Style::default().fg(pal::TARGET))),
        (None, true) => Line::from(Span::styled("F fire · walk into crew to fight, crates to loot · M medkit · < leave", Style::default().fg(pal::DIM))),
        (None, false) => Line::from(Span::styled("arrows, WASD or hjkl walk · Enter talks or uses · F save · Esc quit", Style::default().fg(pal::DIM))),
    };
    Paragraph::new(line).render(inner, buf);
}

/// Aboard: the ship, you (health, arm, vest, medkits), the crew left, and
/// what has happened.
fn board_side(buf: &mut Buffer, area: Rect, app: &App, deck: &Deck) {
    let w = &app.world;
    let block = frame("aboard");
    let inner = block.inner(area);
    block.render(area, buf);
    let dim = Style::default().fg(pal::DIM);
    let text = Style::default().fg(pal::TEXT);
    let b = &w.body;
    let (name, col) = match w.phase {
        crate::sim::Phase::Boarding(id) => w.ship(id).map_or(("a wreck".to_string(), pal::TEXT), |s| (s.callsign.clone(), pal::rgb(pal::faction(s.faction)))),
        _ => ("a wreck".into(), pal::TEXT),
    };
    let hp = b.hp.max(0) as usize;
    let bar_w = 20;
    let filled = hp * bar_w / crate::sim::body::MAX_HP as usize;
    let hp_col = pal::ramp(&pal::GAUGE, b.hp.max(0) as f32 / crate::sim::body::MAX_HP as f32);
    let mut lines = vec![
        Line::from(Span::styled(name, Style::default().fg(col).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![Span::styled("HEALTH ", dim), Span::styled("█".repeat(filled), Style::default().fg(hp_col)), Span::styled("░".repeat(bar_w - filled), Style::default().fg(pal::TRACE))]),
        Line::from(vec![Span::styled(format!("{} ", b.arm().name), text), Span::styled(format!("· {} · {} medkit{}", b.vest_name(), b.medkits, if b.medkits == 1 { "" } else { "s" }), dim)]),
        Line::from(vec![Span::styled("crew fighting  ", dim), Span::styled(deck.crew_left().to_string(), text)]),
        Line::from(""),
    ];
    let room = inner.height.saturating_sub(lines.len() as u16) as usize;
    let width = inner.width as usize;
    // the log, newest at the bottom, wrapped to the panel
    let wrapped: Vec<String> = deck.log.iter().flat_map(|l| wrap(l, width)).collect();
    let skip = wrapped.len().saturating_sub(room);
    let n = wrapped.len();
    for (k, l) in wrapped.into_iter().enumerate().skip(skip) {
        let st = if k + 3 >= n { text } else { dim };
        lines.push(Line::from(Span::styled(l, st)));
    }
    Paragraph::new(lines).render(inner, buf);
}

/// A wall's pipe, joined to the walls and doors around it.
fn wall(d: &Deck, x: i32, y: i32) -> char {
    let j = |x, y| matches!(d.tile(x, y), Tile::Wall | Tile::Door);
    match (j(x, y - 1), j(x, y + 1), j(x - 1, y), j(x + 1, y)) {
        (true, true, true, true) => '┼',
        (true, true, true, false) => '┤',
        (true, true, false, true) => '├',
        (true, false, true, true) => '┴',
        (false, true, true, true) => '┬',
        (true, true, false, false) | (true, false, false, false) | (false, true, false, false) => '│',
        (false, false, true, true) | (false, false, true, false) | (false, false, false, true) => '─',
        (false, true, false, true) => '┌',
        (false, true, true, false) => '┐',
        (true, false, false, true) => '└',
        (true, false, true, false) => '┘',
        (false, false, false, false) => '■',
    }
}
