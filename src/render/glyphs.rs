//! Ships: one ASCII glyph per hull, with Braille for the fine detail
//! (David, 2026-11-07). A line runs out of the nose in the faction's colour
//! and brightens to white at the tip. Behind the ship trails a wake of
//! engine fire, the same for every ship, amber cooling to ember. White
//! always marks the front and dark red the back. Braille dots are finer than
//! a cell, so the line turns smoothly at any heading.
//!
//! The wake shows thrust: a short ember at cruise, a flame while speeding
//! up, a long hot plume on the afterburner. To keep a crowded screen
//! readable, ships other than the player and the target draw shorter and
//! dimmer as more ships crowd the view, and lose their wakes in a big
//! melee.

use super::canvas::Canvas;
use super::palette::{self as pal, rgb, Rgb};
use super::view::View;
use crate::sim::ship::{Class, Ship, Side, Size, AHMA, HAAHKA, HAUKKA, KARHU, KARPPA, KOTKA, KUIKKA, LOKKI};
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier, Style};

/// How much of a ship's Braille to draw: 1 in full, less in a crowd.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detail {
    pub nose: f32,
    pub wake: f32,
}

impl Detail {
    pub const FULL: Detail = Detail { nose: 1.0, wake: 1.0 };

    /// The detail for a ship that is neither the player nor the target, when
    /// `crowd` ships are in view.
    pub fn crowded(crowd: usize) -> Detail {
        match crowd {
            0..=4 => Detail::FULL,
            5..=7 => Detail { nose: 0.75, wake: 0.5 },
            _ => Detail { nose: 0.6, wake: 0.0 },
        }
    }
}

pub fn glyph_of(c: &Class) -> char {
    if std::ptr::eq(c, &KARPPA) {
        'x'
    } else if std::ptr::eq(c, &AHMA) {
        'H'
    } else if std::ptr::eq(c, &KARHU) {
        'M'
    } else if std::ptr::eq(c, &KOTKA) {
        'A'
    } else if std::ptr::eq(c, &HAAHKA) {
        'O'
    } else if std::ptr::eq(c, &KUIKKA) {
        'Y'
    } else if std::ptr::eq(c, &LOKKI) {
        'U'
    } else if std::ptr::eq(c, &HAUKKA) {
        'W'
    } else {
        '#'
    }
}

/// Nose line length in columns, by hull size.
fn nose_len(c: &Class) -> f32 {
    match c.size {
        Size::Light => 4.0,
        Size::Medium => 5.0,
        Size::Heavy => 6.0,
    }
}

fn colours(s: &Ship) -> (Rgb, Rgb) {
    match s.side {
        Side::Player => (pal::PLAYER, (255, 255, 255)),
        _ => (pal::faction(s.faction), (255, 245, 240)),
    }
}

/// The Braille half of a ship: nose line and wake, onto the canvas, before
/// it is written out. Later dots win their cell's colour, so this goes after
/// the stars.
pub fn trails(c: &mut Canvas, view: &View, s: &Ship, d: Detail, fleeing: bool, time: f32) {
    let (fx, fy) = (s.heading.sin(), -s.heading.cos());
    let (ox, oy) = view.cell_f(s.pos);
    // a point `a` columns along the heading, in dots, round on screen
    let at = |a: f32, side: f32| -> (i32, i32) {
        let x = ox + (fx * a - fy * side);
        let y = oy + (fy * a + fx * side) / view.aspect;
        ((x * 2.0).floor() as i32, (y * 4.0).floor() as i32)
    };
    let (hull, tip) = colours(s);
    let tip = if fleeing { hull } else { tip };
    let dimmed = |c: Rgb| if fleeing { pal::scale(c, 0.6) } else { c };

    // the wake first, so the nose wins any shared cell
    let max = s.class.max_speed.max(1.0);
    let push = if s.burning { 1.8 } else { 0.35 + ((s.set_speed - s.speed()) / max * 3.0).clamp(0.0, 0.65) };
    let n = (push * 7.0 * d.wake).round() as i32;
    let (hot, cold): (Rgb, Rgb) = if s.burning { ((255, 250, 210), (255, 120, 40)) } else { ((255, 190, 90), (150, 40, 25)) };
    for i in 0..n {
        let t = i as f32 / (n - 1).max(1) as f32;
        let a = -(1.5 + t * 0.55 * n as f32);
        let jit = 0.3 * (i as f32 * 2.3 + time * 23.0).sin() * t;
        let (x, y) = at(a, jit);
        c.dot(x, y, pal::ramp(&[hot, cold, pal::lerp(cold, (5, 7, 14), 0.7)], t));
    }

    let len = nose_len(s.class) * d.nose;
    let steps = (len * 3.0) as i32;
    for i in 0..=steps {
        let t = i as f32 / steps.max(1) as f32;
        let (x, y) = at(1.5 + (len - 1.5) * t, 0.0);
        c.dot(x, y, pal::ramp(&[pal::lerp(dimmed(hull), (5, 7, 14), 0.4), dimmed(hull), tip], t));
    }
}

/// The glyph half of a ship, as text over the written-out canvas.
pub fn glyph(buf: &mut Buffer, view: &View, s: &Ship, fleeing: bool) {
    let Some((x, y)) = view.cell(s.pos) else { return };
    let (hull, _) = colours(s);
    let flash = s.hit_flash.iter().cloned().fold(9.0f32, f32::min) < 0.1;
    let col = if flash { Color::Rgb(255, 255, 255) } else { rgb(if fleeing { pal::scale(hull, 0.6) } else { hull }) };
    let a = view.area;
    buf[(a.x + x, a.y + y)].set_char(glyph_of(s.class)).set_style(Style::default().fg(col).add_modifier(Modifier::BOLD));
}

/// A hull's top-down wireframe in box-drawing lines, nose up, for the target
/// box (pipes for clean lines and silhouettes, David 2026-11-07).
pub fn schematic(c: &Class) -> &'static [&'static str] {
    if std::ptr::eq(c, &KARPPA) {
        &["╲ ╱", " ┼ ", "╱ ╲"]
    } else if std::ptr::eq(c, &AHMA) {
        &["  ╱╲  ", "┌─┤├─┐", "╵ ╰╯ ╵"]
    } else if std::ptr::eq(c, &KARHU) {
        &["   ╱╲   ", "╶──┤├──╴", "╶──┤├──╴", "   ╰╯   "]
    } else if std::ptr::eq(c, &KOTKA) {
        &["  ╱╲  ", "╱─┤├─╲", "  ╰╯  "]
    } else if std::ptr::eq(c, &HAAHKA) {
        &[" ╭──╮ ", " │▫▫│ ", " │▫▫│ ", " ╰┬┬╯ "]
    } else {
        &[" ╱╲ ", "─┤├─", " ╰╯ "]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crowd_tones_the_others_down() {
        assert_eq!(Detail::crowded(3), Detail::FULL);
        let busy = Detail::crowded(6);
        assert!(busy.nose < 1.0 && busy.wake < 1.0);
        assert_eq!(Detail::crowded(12).wake, 0.0, "a melee drops the wakes");
    }

    #[test]
    fn every_hull_has_its_own_glyph() {
        let g: Vec<char> = [&crate::sim::ship::TIIRA, &KARPPA, &AHMA, &KARHU, &KOTKA, &HAAHKA, &KUIKKA, &LOKKI, &HAUKKA].iter().map(|c| glyph_of(c)).collect();
        let mut u = g.clone();
        u.dedup();
        assert_eq!(g.len(), u.len());
    }
}
