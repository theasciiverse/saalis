//! On foot: the base's deck in classic ASCII (David, 2026-11-08), with
//! the walls in box-drawing pipes, the same line language as the stations
//! outside. You are `@`; everyone else is a letter in their faction's
//! colour.

use super::dock::{side, SIDE_W};
use super::hud::frame;
use super::palette as pal;
use crate::app::App;
use crate::sim::deck::{Deck, Tile, H};
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
    side(buf, Rect::new(area.x, area.y, SIDE_W, area.height), app);
    let map = Rect::new(area.x + SIDE_W, area.y, area.width - SIDE_W, area.height);
    buf.set_style(map, Style::default().bg(pal::CHROME_BG));
    // centred when it fits; otherwise the view follows you
    let fit = |len: i32, room: i32, at: i32| if len <= room { (room - len) / 2 } else { -(at - room / 2).clamp(0, len - room) };
    let ox = map.x as i32 + fit(deck.w, map.width as i32, deck.you.0);
    let oy = map.y as i32 + fit(H, map.height as i32, deck.you.1);
    let mut put = |x: i32, y: i32, c: char, st: Style| {
        let (sx, sy) = (ox + x, oy + y);
        if sx >= map.x as i32 && sy >= map.y as i32 && sx < map.right() as i32 && sy < map.bottom() as i32 {
            buf[(sx as u16, sy as u16)].set_char(c).set_style(st);
        }
    };
    let fg = |c: Color| Style::default().fg(c).bg(pal::CHROME_BG);
    for y in 0..H {
        for x in 0..deck.w {
            let (c, col) = match deck.tile(x, y) {
                Tile::Void => continue,
                Tile::Wall => (wall(deck, x, y), WALL),
                Tile::Door => ('+', DOOR),
                Tile::Floor => ('·', FLOOR),
                Tile::Counter => ('=', Color::Rgb(196, 176, 140)),
                Tile::Terminal => ('&', pal::WIRE),
                Tile::Crate => ('%', pal::rgb(pal::ROCK[2])),
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
        let label = format!(" {} ", r.name);
        let y = if r.y0 == 0 { r.y0 } else { r.y1 };
        let x = r.x0 + 1;
        if label.chars().count() as i32 <= r.x1 - r.x0 - 1 {
            for (i, c) in label.chars().enumerate() {
                put(x + i as i32, y, c, fg(pal::DIM));
            }
        }
    }
    if let Some(p) = app.world.ships.first() {
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
    for n in &deck.npcs {
        put(n.x, n.y, n.who.glyph(), fg(pal::rgb(pal::faction(n.faction))).add_modifier(Modifier::BOLD));
    }
    put(deck.you.0, deck.you.1, '@', fg(Color::White).add_modifier(Modifier::BOLD));
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
    let line = match &app.dock_note {
        Some(n) => Line::from(Span::styled(n.clone(), Style::default().fg(pal::TARGET))),
        None => Line::from(Span::styled("arrows, WASD or hjkl walk · walk into people to talk · F save · Esc quit", Style::default().fg(pal::DIM))),
    };
    Paragraph::new(line).render(inner, buf);
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
