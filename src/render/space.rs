//! The space view: Braille stars, shots and sparks; half-block rocks, the
//! station and fireballs; ships as an ASCII glyph with Braille nose line and
//! wake; box-drawing dock and brackets on top (one glyph family per role,
//! David 2026-11-07).

use super::canvas::Canvas;
use super::glyphs;
use super::palette::{self as pal, rgb, scale, Rgb};
use super::view::View;
use crate::sim::v2::{v2, V2};
use crate::sim::ship::Side;
use crate::sim::{Nav, Phase, Rock, SparkKind, Station, World};
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Style};

/// Light falls from the upper left, toward the viewer.
const LIGHT: (f32, f32, f32) = (-0.48, -0.56, 0.67);

pub fn draw(buf: &mut Buffer, view: &View, w: &World) {
    let a = view.area;
    let mut c = Canvas::new(a.width, a.height);
    stars(&mut c, view);
    for r in &w.rocks {
        if view.sees(r.pos, r.radius * 1.3) {
            rock(&mut c, view, r);
        }
    }
    for b in &w.bases {
        if view.sees(b.pos, b.radius) {
            station(&mut c, view, b, w.time);
        }
    }
    for b in &w.blasts {
        if !view.sees(b.pos, b.radius) {
            continue;
        }
        let t = b.age / b.life;
        // swells fast, then shrinks and cools
        let r = b.radius * if t < 0.25 { 0.5 + t * 2.0 } else { 1.0 - (t - 0.25) * 0.9 };
        let hot = [(255, 255, 230), (255, 214, 110), (240, 110, 40), (120, 40, 30)];
        disc(&mut c, view, b.pos, r, |q| {
            let k = (t * 0.8 + q * 0.5).min(1.0);
            Some(pal::ramp(&hot, k))
        });
    }
    for s in &w.shots {
        let col = if s.side == Side::Player { pal::PLAYER_SHOT } else { pal::PIRATE_SHOT };
        // a short streak, dim at the tail; the head is drawn last so its
        // colour wins the cell
        let tail = s.pos - s.vel * 0.02;
        for k in 0..3 {
            let p = tail + (s.pos - tail) * (k as f32 / 2.0);
            let (x, y) = view.dot(p);
            c.dot(x, y, rgb(scale(col, 0.45 + 0.275 * k as f32)));
        }
    }
    for s in &w.sparks {
        let t = s.age / s.life;
        let base: Rgb = match s.kind {
            SparkKind::Shield => pal::SHIELD,
            SparkKind::Armour => (255, 170, 70),
            SparkKind::Hull => (255, 240, 220),
            SparkKind::Debris => pal::lerp((255, 220, 140), (180, 60, 30), t),
            SparkKind::Rock => (170, 150, 130),
        };
        let (x, y) = view.dot(s.pos);
        c.dot(x, y, rgb(scale(base, 1.0 - t * 0.6)));
    }
    // ships' Braille: the player and the target in full, others toned down
    // as the view fills up
    let seen: Vec<&crate::sim::ship::Ship> = w.ships.iter().filter(|s| s.alive && view.sees(s.pos, 200.0)).collect();
    let crowd = glyphs::Detail::crowded(seen.len());
    for s in &seen {
        let full = s.id == crate::sim::PLAYER || w.target == Some(s.id);
        glyphs::trails(&mut c, view, s, if full { glyphs::Detail::FULL } else { crowd }, w.fleeing(s.id), w.time);
    }
    c.blit(buf, a, pal::SPACE);

    for b in &w.bases {
        if view.sees(b.pos, b.radius + 400.0) {
            dock(buf, view, b, w.time);
        }
    }
    for to in w.sector.neighbours(w.region) {
        let m = w.sector.mouth(w.region, to);
        if view.sees(m, 600.0) {
            gate(buf, view, m, w.sector.regions[to].name, w.time);
        }
    }
    for s in &seen {
        glyphs::glyph(buf, view, s, w.fleeing(s.id));
    }
    if w.phase == Phase::Flying {
        nav_arrow(buf, view, w);
        target_marks(buf, view, w);
    }
}

/// Three layers of stars, further ones moving less (parallax), on a tile
/// that repeats so they never run out.
fn stars(c: &mut Canvas, view: &View) {
    let (w, h) = (view.area.width as i32 * 2, view.area.height as i32 * 4);
    for (layer, &(depth, count)) in [(0.06f32, 90u32), (0.15, 50), (0.32, 22)].iter().enumerate() {
        const TILE: f32 = 640.0;
        // camera offset in dots, scaled by depth
        let ox = view.centre.x / view.scale * 2.0 * depth;
        let oy = view.centre.y / view.row_m() * 4.0 * depth;
        let mut s = 0x9E37_79B9u32.wrapping_mul(layer as u32 + 1);
        for _ in 0..count {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            let sx = (s % 10_000) as f32 / 10_000.0 * TILE;
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            let sy = (s % 10_000) as f32 / 10_000.0 * TILE;
            let x = (sx - ox).rem_euclid(TILE);
            let y = (sy - oy).rem_euclid(TILE);
            let mut px = x as i32;
            while px < w {
                let mut py = y as i32;
                while py < h {
                    c.dot(px, py, pal::STAR[layer]);
                    py += TILE as i32;
                }
                px += TILE as i32;
            }
        }
    }
}

/// Fills a world-space disc of radius `r` with half-block pixels; `paint`
/// gets the distance from the centre as a fraction (0 centre .. 1 edge).
fn disc(c: &mut Canvas, view: &View, centre: V2, r: f32, paint: impl Fn(f32) -> Option<Color>) {
    let (pw, ph) = view.pixel_m();
    let (cx, cy) = view.pixel(centre);
    let (rx, ry) = ((r / pw).ceil() as i32 + 1, (r / ph).ceil() as i32 + 1);
    for y in (cy - ry)..=(cy + ry) {
        for x in (cx - rx)..=(cx + rx) {
            let d = (view.pixel_world(x, y) - centre).len() / r;
            if d <= 1.0 {
                if let Some(col) = paint(d) {
                    c.pixel(x, y, col);
                }
            }
        }
    }
}

fn hash(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x2C1B_3C6D) ^ 0x297A_2D39;
    x ^= x >> 15;
    x = x.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 13;
    (x & 0xFFFF) as f32 / 65_535.0
}

/// A lumpy, lit rock with a few craters.
fn rock(c: &mut Canvas, view: &View, r: &Rock) {
    let s = r.seed;
    let ph: [f32; 3] = [hash(s) * 6.3, hash(s ^ 1) * 6.3, hash(s ^ 2) * 6.3];
    let base = pal::ROCK[(s % 3) as usize];
    let craters: Vec<(V2, f32)> = (0..3)
        .map(|k| {
            let a = hash(s ^ (10 + k)) * 6.3;
            let d = hash(s ^ (20 + k)) * 0.6;
            (r.pos + V2::from_heading(a) * (r.radius * d), r.radius * (0.12 + hash(s ^ (30 + k)) * 0.15))
        })
        .collect();
    let outer = r.radius * 1.25;
    let (pw, phm) = view.pixel_m();
    let (cx, cy) = view.pixel(r.pos);
    let (rx, ry) = ((outer / pw).ceil() as i32 + 1, (outer / phm).ceil() as i32 + 1);
    for y in (cy - ry)..=(cy + ry) {
        for x in (cx - rx)..=(cx + rx) {
            let p = view.pixel_world(x, y);
            let d = p - r.pos;
            let ang = d.heading();
            let edge = r.radius * (1.0 + 0.13 * (3.0 * ang + ph[0]).sin() + 0.07 * (5.0 * ang + ph[1]).sin() + 0.04 * (9.0 * ang + ph[2]).sin());
            let q = d.len() / edge;
            if q > 1.0 {
                continue;
            }
            let nz = (1.0 - q * q).max(0.0).sqrt();
            let (nx, ny) = (d.x / edge, d.y / edge);
            let lit = (nx * LIGHT.0 + ny * LIGHT.1 + nz * LIGHT.2).max(0.0);
            let mut k = 0.18 + lit * 0.95;
            if craters.iter().any(|&(cp, cr)| (p - cp).len() < cr) {
                k *= 0.62;
            }
            c.pixel(x, y, rgb(scale(base, k)));
        }
    }
}

/// A base: a slowly turning ring habitat seen from above, with lit windows,
/// four spokes and a hub.
fn station(c: &mut Canvas, view: &View, st: &Station, time: f32) {
    let spin = time * 0.05;
    let (pw, phm) = view.pixel_m();
    let (cx, cy) = view.pixel(st.pos);
    let (rx, ry) = ((st.radius / pw).ceil() as i32 + 1, (st.radius / phm).ceil() as i32 + 1);
    for y in (cy - ry)..=(cy + ry) {
        for x in (cx - rx)..=(cx + rx) {
            let p = view.pixel_world(x, y);
            let d = p - st.pos;
            let q = d.len() / st.radius;
            if q > 1.0 {
                continue;
            }
            let ang = d.heading() - spin;
            let (nx, ny) = (d.x / st.radius, d.y / st.radius);
            let lit = 0.45 + 0.55 * (-(nx * LIGHT.0 + ny * LIGHT.1)).clamp(-1.0, 1.0).max(0.0);
            let col = if q > 0.72 {
                let window = q > 0.8 && q < 0.9 && (ang * 24.0 / std::f32::consts::TAU).rem_euclid(1.0) < 0.35;
                if window { rgb(pal::WINDOW) } else { rgb(scale(pal::STATION, lit * if q > 0.95 { 0.7 } else { 1.0 })) }
            } else if q < 0.26 {
                rgb(scale(pal::STATION, lit * 1.1))
            } else {
                let spoke = (ang / std::f32::consts::FRAC_PI_2).rem_euclid(1.0);
                if !(0.04..=0.96).contains(&spoke) {
                    rgb(scale(pal::STATION, lit * 0.8))
                } else {
                    continue;
                }
            };
            c.pixel(x, y, col);
        }
    }
}

/// The docking arm north of the ring, in box-drawing lines, with blinking
/// lamps at its mouth; and the station's name.
fn dock(buf: &mut Buffer, view: &View, st: &Station, time: f32) {
    let a = view.area;
    let top = st.pos + v2(0.0, -(st.radius + 220.0));
    let bottom = st.pos + v2(0.0, -st.radius * 0.95);
    let half = 50.0;
    let wire = Style::default().fg(pal::WIRE);
    let put = |buf: &mut Buffer, p: V2, ch: &str, style: Style| {
        if let Some((x, y)) = view.cell(p) {
            buf[(a.x + x, a.y + y)].set_symbol(ch).set_style(style);
        }
    };
    let (_, y0) = view.cell_f(top);
    let (_, y1) = view.cell_f(bottom);
    let rows = (y1 - y0).ceil().max(1.0) as i32;
    for k in 0..=rows {
        let y = top.y + k as f32 * view.row_m();
        for &sx in &[-half, half] {
            let ch = if k == 0 { "╷" } else { "│" };
            put(buf, v2(st.pos.x + sx, y), ch, wire);
        }
    }
    let blink = (time * 2.0) as i32 % 2 == 0;
    let lamp = Style::default().fg(if blink { Color::Rgb(120, 255, 160) } else { Color::Rgb(30, 90, 50) });
    put(buf, top + v2(-half - view.scale, 0.0), "•", lamp);
    put(buf, top + v2(half + view.scale, 0.0), "•", lamp);
    let label = st.name.to_uppercase();
    let below = st.pos + v2(0.0, st.radius + view.row_m());
    if let Some((x, y)) = view.cell(below) {
        let x0 = (x as i32 - label.chars().count() as i32 / 2).max(0) as u16;
        buf.set_string(a.x + x0, a.y + y, &label, Style::default().fg(pal::DIM));
    }
}

/// Brackets round the selected target, or an arrow at the edge pointing to
/// it when it is off the screen.
fn target_marks(buf: &mut Buffer, view: &View, w: &World) {
    let Some(t) = w.target.and_then(|id| w.ship(id)) else { return };
    let a = view.area;
    let style = Style::default().fg(pal::TARGET);
    let (fx, fy) = view.cell_f(t.pos);
    let (hw, hh) = match t.class.size {
        crate::sim::ship::Size::Light => (3, 2),
        crate::sim::ship::Size::Medium => (4, 2),
        crate::sim::ship::Size::Heavy => (5, 3),
    };
    let inside = fx >= 0.0 && fy >= 0.0 && fx < a.width as f32 && fy < a.height as f32;
    if inside {
        let (x, y) = (fx as i32, fy as i32);
        for (dx, dy, ch) in [(-hw, -hh, "┌"), (hw, -hh, "┐"), (-hw, hh, "└"), (hw, hh, "┘")] {
            let (cx, cy) = (x + dx, y + dy);
            if cx >= 0 && cy >= 0 && cx < a.width as i32 && cy < a.height as i32 {
                buf[(a.x + cx as u16, a.y + cy as u16)].set_symbol(ch).set_style(style);
            }
        }
        return;
    }
    edge_arrow(buf, view, t.pos, style);
}

/// A drive lane's mouth: a box-drawing gate with the destination's name.
fn gate(buf: &mut Buffer, view: &View, at: V2, name: &str, time: f32) {
    let a = view.area;
    let Some((x, y)) = view.cell(at) else { return };
    let pulse = 0.7 + 0.3 * (time * 2.5).sin();
    let col = rgb(scale(pal::LANE, pulse));
    let style = Style::default().fg(col);
    let rows = ["╭─────╮", "│     │", "╰─────╯"];
    for (k, row) in rows.iter().enumerate() {
        let ry = y as i32 - 1 + k as i32;
        for (j, ch) in row.chars().enumerate() {
            let rx = x as i32 - 3 + j as i32;
            if ch != ' ' && rx >= 0 && ry >= 0 && rx < a.width as i32 && ry < a.height as i32 {
                buf[(a.x + rx as u16, a.y + ry as u16)].set_char(ch).set_style(style);
            }
        }
    }
    let label = format!("lane ▸ {name}");
    let ly = y as i32 + 2;
    if ly < a.height as i32 {
        let lx = (x as i32 - label.chars().count() as i32 / 2).max(0) as u16;
        let room = a.width.saturating_sub(lx) as usize;
        let text: String = label.chars().take(room).collect();
        buf.set_string(a.x + lx, a.y + ly as u16, text, Style::default().fg(pal::DIM));
    }
}

/// An arrow at the field's edge toward the nav point when it is off screen.
fn nav_arrow(buf: &mut Buffer, view: &View, w: &World) {
    let Some(n) = w.nav else { return };
    let col = match n {
        Nav::Base(_) => pal::TRAFFIC,
        Nav::Mouth(_) => rgb(pal::LANE),
    };
    edge_arrow(buf, view, w.nav_pos(n), Style::default().fg(col));
}

/// An arrow on the field's border pointing toward `p`, if it is off screen.
fn edge_arrow(buf: &mut Buffer, view: &View, p: V2, style: Style) {
    let a = view.area;
    let (fx, fy) = view.cell_f(p);
    if fx >= 0.0 && fy >= 0.0 && fx < a.width as f32 && fy < a.height as f32 {
        return;
    }
    let (mx, my) = (a.width as f32 / 2.0, a.height as f32 / 2.0);
    let (dx, dy) = (fx - mx, fy - my);
    let k = ((mx - 1.0) / dx.abs().max(1e-3)).min((my - 1.0) / dy.abs().max(1e-3));
    let (ex, ey) = ((mx + dx * k).clamp(0.0, a.width as f32 - 1.0), (my + dy * k).clamp(0.0, a.height as f32 - 1.0));
    let ang = (p - view.centre).heading();
    const ARROWS: [&str; 8] = ["↑", "↗", "→", "↘", "↓", "↙", "←", "↖"];
    let i = ((ang / std::f32::consts::FRAC_PI_4).round() as i32).rem_euclid(8) as usize;
    buf[(a.x + ex as u16, a.y + ey as u16)].set_symbol(ARROWS[i]).set_style(style);
}
