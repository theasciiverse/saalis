//! Small craft as classic ASCII glyph clusters (David, 2026-11-07: fighters
//! in ASCII, big things in half-blocks). Each hull is a few parts placed in
//! the ship's own frame and turned with it: a fixed body glyph, wing and
//! spine strokes that pick `-` `\` `|` `/` by their angle on the screen, an
//! engine that glows when thrusting, and a bright nose so the heading reads
//! even when the ship sits still.
//!
//! Offsets are in row heights (forward, right); a row is `aspect` columns
//! wide on the screen.

use super::palette::{self as pal, rgb, Rgb};
use super::view::View;
use crate::sim::ship::{Class, Ship, Side, AHMA, KARHU, KARPPA, TIIRA};
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Style};

#[derive(Clone, Copy)]
enum Part {
    Body(char),
    Wing,
    Spine,
    Engine,
    Nose,
}
use Part::*;

type Plan = &'static [(Part, f32, f32)];

const TIIRA_PLAN: Plan = &[(Wing, 0.0, -0.55), (Wing, 0.0, 0.55), (Body('#'), 0.0, 0.0), (Engine, -0.9, 0.0), (Nose, 0.9, 0.0)];
const KARPPA_PLAN: Plan = &[(Wing, -0.35, -0.5), (Wing, -0.35, 0.5), (Body('x'), 0.0, 0.0), (Engine, -0.9, 0.0), (Nose, 0.8, 0.0)];
const AHMA_PLAN: Plan = &[
    (Wing, -0.2, -1.05),
    (Wing, -0.2, 1.05),
    (Wing, 0.0, -0.55),
    (Wing, 0.0, 0.55),
    (Body('H'), 0.0, 0.0),
    (Engine, -0.9, 0.0),
    (Nose, 0.9, 0.0),
];
const KARHU_PLAN: Plan = &[
    (Wing, -0.3, -1.6),
    (Wing, -0.3, 1.6),
    (Wing, -0.3, -1.1),
    (Wing, -0.3, 1.1),
    (Wing, -0.3, -0.55),
    (Wing, -0.3, 0.55),
    (Spine, 0.55, 0.0),
    (Body('#'), 0.0, 0.0),
    (Body('#'), -0.6, 0.0),
    (Engine, -1.3, 0.0),
    (Nose, 1.15, 0.0),
];

fn plan(c: &Class) -> Plan {
    if std::ptr::eq(c, &TIIRA) {
        TIIRA_PLAN
    } else if std::ptr::eq(c, &KARPPA) {
        KARPPA_PLAN
    } else if std::ptr::eq(c, &AHMA) {
        AHMA_PLAN
    } else if std::ptr::eq(c, &KARHU) {
        KARHU_PLAN
    } else {
        TIIRA_PLAN
    }
}

/// The stroke for a line running along screen direction (x, y), y down.
pub fn stroke(x: f32, y: f32) -> char {
    let a = y.atan2(x).rem_euclid(std::f32::consts::PI);
    let s = std::f32::consts::PI / 8.0;
    if !(s..7.0 * s).contains(&a) {
        '-'
    } else if a < 3.0 * s {
        '\\'
    } else if a < 5.0 * s {
        '|'
    } else {
        '/'
    }
}

/// Draws one ship at its place in the view.
pub fn ship(buf: &mut Buffer, view: &View, s: &Ship, fleeing: bool, time: f32) {
    let a = view.area;
    let (cx, cy) = view.cell_f(s.pos);
    let (fx, fy) = (s.heading.sin(), -s.heading.cos());
    let (rx, ry) = (-fy, fx);
    let (hull, wing): (Rgb, Rgb) = match s.side {
        Side::Player => (pal::PLAYER, pal::PLAYER_WING),
        Side::Pirate => (pal::PIRATE, pal::PIRATE_WING),
    };
    let dim = |c: Rgb| if fleeing { pal::scale(c, 0.6) } else { c };
    // a fresh hit flashes the hull
    let flash = s.hit_flash.iter().cloned().fold(9.0f32, f32::min) < 0.1;
    let thrust = s.speed() + 1.0 < s.set_speed || s.burning;
    for &(part, f, r) in plan(s.class) {
        let ux = fx * f + rx * r;
        let uy = fy * f + ry * r;
        // parts sit round the ship's own cell, symmetric either side
        let col = cx.floor() as i32 + (ux * view.aspect).round() as i32;
        let row = cy.floor() as i32 + uy.round() as i32;
        if col < 0 || row < 0 || col >= a.width as i32 || row >= a.height as i32 {
            continue;
        }
        let (ch, color): (char, Color) = match part {
            Body(c) => (c, if flash { Color::Rgb(255, 255, 255) } else { rgb(dim(hull)) }),
            Wing => (stroke(rx, ry), rgb(dim(wing))),
            Spine => (stroke(fx, fy), rgb(dim(wing))),
            Nose => ('o', if fleeing { rgb(dim(hull)) } else { pal::NOSE }),
            Engine => {
                if s.burning {
                    let flick = ((time * 30.0) as i32) % 2 == 0;
                    (if flick { '*' } else { '+' }, rgb(pal::BURN))
                } else if thrust {
                    (':', Color::Rgb(200, 120, 60))
                } else {
                    continue;
                }
            }
        };
        let cell = &mut buf[(a.x + col as u16, a.y + row as u16)];
        cell.set_char(ch).set_style(Style::default().fg(color));
    }
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
    } else {
        &[" ╱╲ ", "─┤├─", " ╰╯ "]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strokes_follow_the_line() {
        assert_eq!(stroke(1.0, 0.0), '-');
        assert_eq!(stroke(0.0, 1.0), '|');
        assert_eq!(stroke(1.0, 1.0), '\\');
        assert_eq!(stroke(1.0, -1.0), '/');
        assert_eq!(stroke(-1.0, 0.05), '-');
    }
}
